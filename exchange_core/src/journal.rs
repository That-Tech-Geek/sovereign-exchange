use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::command::ExchangeEvent;

const MAGIC: &[u8; 4] = b"SEJ1";
const VERSION: u16 = 1;
const HEADER_LEN: u32 = 32;

#[derive(Debug)]
pub enum JournalError {
    Io(io::Error),
    InvalidHeader(&'static str),
    InvalidRecord(&'static str),
    ChecksumMismatch { expected: u32, actual: u32 },
    SequenceRegression { previous: u64, next: u64 },
    UnsupportedVersion(u16),
    PayloadTooLarge(usize),
}
impl From<io::Error> for JournalError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl std::fmt::Display for JournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "journal I/O error: {e}"),
            Self::InvalidHeader(m) => write!(f, "invalid journal header: {m}"),
            Self::InvalidRecord(m) => write!(f, "invalid journal record: {m}"),
            Self::ChecksumMismatch { expected, actual } => write!(
                f,
                "journal checksum mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::SequenceRegression { previous, next } => write!(
                f,
                "journal sequence regression: previous={previous}, next={next}"
            ),
            Self::UnsupportedVersion(v) => write!(f, "unsupported journal version {v}"),
            Self::PayloadTooLarge(n) => write!(f, "journal payload too large: {n} bytes"),
        }
    }
}
impl std::error::Error for JournalError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    SyncData,
    Buffered,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalPosition {
    pub offset: u64,
    pub sequence: u64,
}
pub struct EventJournal {
    file: File,
    path: PathBuf,
    next_sequence: Option<u64>,
    durability: Durability,
}
impl EventJournal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, JournalError> {
        Self::open_with_durability(path, Durability::SyncData)
    }
    pub fn open_with_durability(
        path: impl AsRef<Path>,
        durability: Durability,
    ) -> Result<Self, JournalError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;
        let mut journal = Self {
            file,
            path,
            next_sequence: None,
            durability,
        };
        journal.validate_existing()?;
        Ok(journal)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn append(&mut self, event: &ExchangeEvent) -> Result<JournalPosition, JournalError> {
        let p = self.append_internal(event)?;
        self.sync_if_required()?;
        Ok(p)
    }
    pub fn append_batch(
        &mut self,
        events: &[ExchangeEvent],
    ) -> Result<Option<JournalPosition>, JournalError> {
        let mut last = None;
        for event in events {
            last = Some(self.append_internal(event)?);
        }
        if !events.is_empty() {
            self.sync_if_required()?;
        }
        Ok(last)
    }
    pub fn sync(&mut self) -> Result<(), JournalError> {
        self.file.sync_data().map_err(JournalError::Io)
    }
    pub fn next_sequence(&self) -> Option<u64> {
        self.next_sequence
    }
    fn append_internal(&mut self, event: &ExchangeEvent) -> Result<JournalPosition, JournalError> {
        let sequence = event_sequence(event);
        if let Some(previous) = self.next_sequence {
            if sequence < previous {
                return Err(JournalError::SequenceRegression {
                    previous,
                    next: sequence,
                });
            }
        }
        let payload = encode_event(event);
        if payload.len() > u32::MAX as usize {
            return Err(JournalError::PayloadTooLarge(payload.len()));
        }
        let timestamp = unix_nanos();
        let checksum = crc32(&payload);
        let mut header = [0u8; HEADER_LEN as usize];
        header[0..4].copy_from_slice(MAGIC);
        header[4..6].copy_from_slice(&VERSION.to_le_bytes());
        header[6] = event_type(event);
        header[8..16].copy_from_slice(&sequence.to_le_bytes());
        header[16..24].copy_from_slice(&timestamp.to_le_bytes());
        header[24..28].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        header[28..32].copy_from_slice(&checksum.to_le_bytes());
        let offset = self.file.metadata()?.len();
        self.file.write_all(&header)?;
        self.file.write_all(&payload)?;
        self.next_sequence = Some(sequence);
        Ok(JournalPosition { offset, sequence })
    }
    fn sync_if_required(&mut self) -> Result<(), JournalError> {
        if self.durability == Durability::SyncData {
            self.sync()?;
        }
        Ok(())
    }
    fn validate_existing(&mut self) -> Result<(), JournalError> {
        let mut bytes = Vec::new();
        self.file.read_to_end(&mut bytes)?;
        let mut offset = 0usize;
        let mut previous = None;
        while offset < bytes.len() {
            if bytes.len() - offset < HEADER_LEN as usize {
                return Err(JournalError::InvalidRecord("truncated header"));
            }
            let header = &bytes[offset..offset + HEADER_LEN as usize];
            if &header[0..4] != MAGIC {
                return Err(JournalError::InvalidHeader("bad magic"));
            }
            let version = u16::from_le_bytes([header[4], header[5]]);
            if version != VERSION {
                return Err(JournalError::UnsupportedVersion(version));
            }
            let payload_len = u32::from_le_bytes(header[24..28].try_into().unwrap()) as usize;
            let end = offset
                .checked_add(HEADER_LEN as usize)
                .and_then(|x| x.checked_add(payload_len))
                .ok_or(JournalError::InvalidRecord("record length overflow"))?;
            if end > bytes.len() {
                return Err(JournalError::InvalidRecord("truncated payload"));
            }
            let sequence = u64::from_le_bytes(header[8..16].try_into().unwrap());
            if let Some(p) = previous {
                if sequence < p {
                    return Err(JournalError::SequenceRegression {
                        previous: p,
                        next: sequence,
                    });
                }
            }
            let expected = u32::from_le_bytes(header[28..32].try_into().unwrap());
            let actual = crc32(&bytes[offset + HEADER_LEN as usize..end]);
            if expected != actual {
                return Err(JournalError::ChecksumMismatch { expected, actual });
            }
            previous = Some(sequence);
            offset = end;
        }
        self.next_sequence = previous;
        self.file.seek(std::io::SeekFrom::End(0))?;
        Ok(())
    }
}
fn event_type(event: &ExchangeEvent) -> u8 {
    match event {
        ExchangeEvent::OrderAccepted { .. } => 1,
        ExchangeEvent::OrderRejected { .. } => 2,
        ExchangeEvent::OrderCancelled { .. } => 3,
        ExchangeEvent::OrderReplaced { .. } => 4,
        ExchangeEvent::Trade { .. } => 5,
    }
}
fn event_sequence(event: &ExchangeEvent) -> u64 {
    match event {
        ExchangeEvent::OrderAccepted {
            sequence_number, ..
        }
        | ExchangeEvent::OrderCancelled {
            sequence_number, ..
        }
        | ExchangeEvent::OrderReplaced {
            sequence_number, ..
        } => sequence_number.0,
        ExchangeEvent::OrderRejected {
            sequence_number, ..
        } => sequence_number.map_or(0, |s| s.0),
        ExchangeEvent::Trade {
            buyer_sequence_number,
            seller_sequence_number,
            ..
        } => buyer_sequence_number.0.max(seller_sequence_number.0),
    }
}
fn encode_event(event: &ExchangeEvent) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    match event {
        ExchangeEvent::OrderAccepted {
            instrument_id,
            account_id,
            client_order_id,
            exchange_order_id,
            sequence_number,
        } => {
            out.push(1);
            put_u16(&mut out, *instrument_id);
            put_u32(&mut out, *account_id);
            put_u64(&mut out, client_order_id.0);
            put_u64(&mut out, exchange_order_id.0);
            put_u64(&mut out, sequence_number.0)
        }
        ExchangeEvent::OrderRejected {
            account_id,
            client_order_id,
            sequence_number,
            reason,
        } => {
            out.push(2);
            put_u32(&mut out, *account_id);
            put_u64(&mut out, client_order_id.0);
            put_u64(&mut out, sequence_number.map_or(0, |s| s.0));
            out.push(*reason as u8)
        }
        ExchangeEvent::OrderCancelled {
            instrument_id,
            account_id,
            client_order_id,
            sequence_number,
        } => {
            out.push(3);
            put_u16(&mut out, *instrument_id);
            put_u32(&mut out, *account_id);
            put_u64(&mut out, client_order_id.0);
            put_u64(&mut out, sequence_number.0)
        }
        ExchangeEvent::OrderReplaced {
            instrument_id,
            account_id,
            old_client_order_id,
            new_client_order_id,
            exchange_order_id,
            sequence_number,
        } => {
            out.push(4);
            put_u16(&mut out, *instrument_id);
            put_u32(&mut out, *account_id);
            put_u64(&mut out, old_client_order_id.0);
            put_u64(&mut out, new_client_order_id.0);
            put_u64(&mut out, exchange_order_id.0);
            put_u64(&mut out, sequence_number.0)
        }
        ExchangeEvent::Trade {
            buyer,
            seller,
            instrument_id,
            buyer_exchange_order_id,
            seller_exchange_order_id,
            buyer_client_order_id,
            seller_client_order_id,
            buyer_sequence_number,
            seller_sequence_number,
            price,
            qty,
            timestamp,
        } => {
            out.push(5);
            put_u32(&mut out, *buyer);
            put_u32(&mut out, *seller);
            put_u16(&mut out, *instrument_id);
            put_u64(&mut out, buyer_exchange_order_id.0);
            put_u64(&mut out, seller_exchange_order_id.0);
            put_u64(&mut out, buyer_client_order_id.0);
            put_u64(&mut out, seller_client_order_id.0);
            put_u64(&mut out, buyer_sequence_number.0);
            put_u64(&mut out, seller_sequence_number.0);
            put_u32(&mut out, *price);
            put_u32(&mut out, *qty);
            put_u64(&mut out, *timestamp)
        }
    }
    out
}
fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes())
}
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes())
}
fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes())
}
fn unix_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(u64::MAX as u128) as u64
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
