use crate::command::{CancelOrder, NewOrder, OrderCommand, OrderSide, ReplaceOrder};
use crate::order::ClientOrderId;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"SEC1";
const VERSION: u16 = 1;
const HEADER: usize = 16;
const MAX_RECORD: usize = 128;

#[derive(Debug)]
pub enum CommandJournalError {
    Io(io::Error),
    Invalid(&'static str),
    Checksum,
    UnsupportedVersion(u16),
}

impl From<io::Error> for CommandJournalError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for CommandJournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "command journal I/O error: {e}"),
            Self::Invalid(m) => write!(f, "invalid command journal: {m}"),
            Self::Checksum => write!(f, "command journal checksum mismatch"),
            Self::UnsupportedVersion(v) => {
                write!(f, "unsupported command journal version {v}")
            }
        }
    }
}

impl std::error::Error for CommandJournalError {}

pub struct CommandJournal {
    file: File,
    path: PathBuf,
}

impl CommandJournal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CommandJournalError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;
        let mut journal = Self { file, path };
        journal.validate()?;
        Ok(journal)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, command: &OrderCommand) -> Result<(), CommandJournalError> {
        let payload = encode(command);
        if payload.len() > MAX_RECORD {
            return Err(CommandJournalError::Invalid("record too large"));
        }
        let checksum = crc32(&payload);
        let mut header = [0u8; HEADER];
        header[0..4].copy_from_slice(MAGIC);
        header[4..6].copy_from_slice(&VERSION.to_le_bytes());
        header[6] = payload[0];
        header[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        header[12..16].copy_from_slice(&checksum.to_le_bytes());
        self.file.write_all(&header)?;
        self.file.write_all(&payload)?;
        self.file.sync_data()?;
        Ok(())
    }

    pub fn read_all(&mut self) -> Result<Vec<OrderCommand>, CommandJournalError> {
        self.file.rewind()?;
        let mut bytes = Vec::new();
        self.file.read_to_end(&mut bytes)?;
        decode_records(&bytes)
    }

    fn validate(&mut self) -> Result<(), CommandJournalError> {
        let _ = self.read_all()?;
        self.file.seek(std::io::SeekFrom::End(0))?;
        Ok(())
    }
}

fn encode(command: &OrderCommand) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    match *command {
        OrderCommand::New(x) => {
            out.push(1);
            put_u64(&mut out, x.client_order_id.0);
            put_u32(&mut out, x.account_id);
            put_u16(&mut out, x.instrument_id);
            out.push(x.side.wire_value());
            put_u32(&mut out, x.price);
            put_u32(&mut out, x.quantity);
            put_u64(&mut out, x.client_timestamp);
        }
        OrderCommand::Cancel(x) => {
            out.push(2);
            put_u32(&mut out, x.account_id);
            put_u16(&mut out, x.instrument_id);
            put_u64(&mut out, x.client_order_id.0);
        }
        OrderCommand::Replace(x) => {
            out.push(3);
            put_u32(&mut out, x.account_id);
            put_u16(&mut out, x.instrument_id);
            put_u64(&mut out, x.target_client_order_id.0);
            put_u64(&mut out, x.new_client_order_id.0);
            out.push(x.side.wire_value());
            put_u32(&mut out, x.price);
            put_u32(&mut out, x.quantity);
            put_u64(&mut out, x.client_timestamp);
        }
    }
    out
}

fn decode_records(bytes: &[u8]) -> Result<Vec<OrderCommand>, CommandJournalError> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes.len() - pos < HEADER {
            return Err(CommandJournalError::Invalid("truncated header"));
        }
        if &bytes[pos..pos + 4] != MAGIC {
            return Err(CommandJournalError::Invalid("bad magic"));
        }
        let version = u16::from_le_bytes([bytes[pos + 4], bytes[pos + 5]]);
        if version != VERSION {
            return Err(CommandJournalError::UnsupportedVersion(version));
        }
        let len = u32::from_le_bytes(bytes[pos + 8..pos + 12].try_into().unwrap()) as usize;
        if len == 0 || len > MAX_RECORD {
            return Err(CommandJournalError::Invalid("invalid record length"));
        }
        let end = pos
            .checked_add(HEADER + len)
            .filter(|end| *end <= bytes.len())
            .ok_or(CommandJournalError::Invalid("invalid record length"))?;
        let payload = &bytes[pos + HEADER..end];
        let expected = u32::from_le_bytes(bytes[pos + 12..pos + 16].try_into().unwrap());
        if crc32(payload) != expected {
            return Err(CommandJournalError::Checksum);
        }
        out.push(decode(payload)?);
        pos = end;
    }
    Ok(out)
}

fn decode(bytes: &[u8]) -> Result<OrderCommand, CommandJournalError> {
    let mut reader = Reader { bytes, pos: 0 };
    let side = |value| match value {
        0 => Ok(OrderSide::Buy),
        1 => Ok(OrderSide::Sell),
        _ => Err(CommandJournalError::Invalid("invalid side")),
    };
    match reader.u8()? {
        1 => Ok(OrderCommand::New(NewOrder {
            client_order_id: ClientOrderId(reader.u64()?),
            account_id: reader.u32()?,
            instrument_id: reader.u16()?,
            side: side(reader.u8()?)?,
            price: reader.u32()?,
            quantity: reader.u32()?,
            client_timestamp: reader.u64()?,
        })),
        2 => Ok(OrderCommand::Cancel(CancelOrder {
            account_id: reader.u32()?,
            instrument_id: reader.u16()?,
            client_order_id: ClientOrderId(reader.u64()?),
        })),
        3 => Ok(OrderCommand::Replace(ReplaceOrder {
            account_id: reader.u32()?,
            instrument_id: reader.u16()?,
            target_client_order_id: ClientOrderId(reader.u64()?),
            new_client_order_id: ClientOrderId(reader.u64()?),
            side: side(reader.u8()?)?,
            price: reader.u32()?,
            quantity: reader.u32()?,
            client_timestamp: reader.u64()?,
        })),
        _ => Err(CommandJournalError::Invalid("unknown command type")),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], CommandJournalError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(CommandJournalError::Invalid("offset overflow"))?;
        if end > self.bytes.len() {
            return Err(CommandJournalError::Invalid("truncated payload"));
        }
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, CommandJournalError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, CommandJournalError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, CommandJournalError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, CommandJournalError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
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
