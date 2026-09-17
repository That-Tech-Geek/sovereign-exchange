use crate::order::{ClientOrderId, ExchangeOrderId};
use crate::replay::{ReplayLedger, ReplayOrderKey, ReplayTrade};
use crate::sequence::SequenceNumber;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
const MAGIC: &[u8; 4] = b"SES1";
const VERSION: u16 = 1;
const HEADER_LEN: usize = 28;
#[derive(Debug)]
pub enum SnapshotError {
    Io(io::Error),
    InvalidHeader(&'static str),
    UnsupportedVersion(u16),
    Truncated,
    ChecksumMismatch { expected: u32, actual: u32 },
    InvalidData(&'static str),
    JournalSequenceMismatch { snapshot: u64, journal: u64 },
}
impl From<io::Error> for SnapshotError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "snapshot I/O error: {e}"),
            Self::InvalidHeader(m) => write!(f, "invalid snapshot header: {m}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported snapshot version {v}"),
            Self::Truncated => write!(f, "truncated snapshot"),
            Self::ChecksumMismatch { expected, actual } => write!(
                f,
                "snapshot checksum mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::InvalidData(m) => write!(f, "invalid snapshot data: {m}"),
            Self::JournalSequenceMismatch { snapshot, journal } => write!(
                f,
                "journal sequence {journal} is behind snapshot boundary {snapshot}"
            ),
        }
    }
}
impl std::error::Error for SnapshotError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotMeta {
    pub journal_sequence: u64,
    pub event_count: u64,
}
pub struct ReplaySnapshot;
impl ReplaySnapshot {
    pub fn write(
        path: impl AsRef<Path>,
        ledger: &ReplayLedger,
    ) -> Result<SnapshotMeta, SnapshotError> {
        let path = path.as_ref();
        let meta = SnapshotMeta {
            journal_sequence: ledger.last_sequence.unwrap_or(0),
            event_count: ledger.event_count,
        };
        let payload = encode(ledger, meta);
        let checksum = crc32(&payload);
        let mut bytes = Vec::with_capacity(HEADER_LEN + payload.len());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&meta.journal_sequence.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&checksum.to_le_bytes());
        bytes.extend_from_slice(&payload);
        let tmp = temp_path(path);
        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                File::open(parent)?.sync_all()?;
            }
        }
        Ok(meta)
    }
    pub fn read(path: impl AsRef<Path>) -> Result<(SnapshotMeta, ReplayLedger), SnapshotError> {
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.len() < HEADER_LEN {
            return Err(SnapshotError::Truncated);
        }
        if &bytes[0..4] != MAGIC {
            return Err(SnapshotError::InvalidHeader("bad magic"));
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION {
            return Err(SnapshotError::UnsupportedVersion(version));
        }
        let journal_sequence = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let payload_len = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
        if HEADER_LEN
            .checked_add(payload_len)
            .filter(|n| *n == bytes.len())
            .is_none()
        {
            return Err(SnapshotError::Truncated);
        }
        let expected = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let payload = &bytes[HEADER_LEN..];
        let actual = crc32(payload);
        if expected != actual {
            return Err(SnapshotError::ChecksumMismatch { expected, actual });
        }
        let (event_count, ledger) = decode(payload, journal_sequence)?;
        Ok((
            SnapshotMeta {
                journal_sequence,
                event_count,
            },
            ledger,
        ))
    }
    pub fn validate_journal_boundary(
        meta: SnapshotMeta,
        journal_sequence: u64,
    ) -> Result<(), SnapshotError> {
        if journal_sequence < meta.journal_sequence {
            return Err(SnapshotError::JournalSequenceMismatch {
                snapshot: meta.journal_sequence,
                journal: journal_sequence,
            });
        }
        Ok(())
    }
}
fn encode(ledger: &ReplayLedger, meta: SnapshotMeta) -> Vec<u8> {
    let mut out = Vec::new();
    put_u64(&mut out, meta.event_count);
    put_u32(&mut out, ledger.accepted_orders.len() as u32);
    for (k, e) in &ledger.accepted_orders {
        put_u16(&mut out, k.instrument_id);
        put_u32(&mut out, k.account_id);
        put_u64(&mut out, k.client_order_id.0);
        put_u64(&mut out, e.0)
    }
    let mut cancelled: Vec<_> = ledger.cancelled_orders.iter().copied().collect();
    cancelled.sort_by_key(|k| (k.instrument_id, k.account_id, k.client_order_id.0));
    put_u32(&mut out, cancelled.len() as u32);
    for k in cancelled {
        put_u16(&mut out, k.instrument_id);
        put_u32(&mut out, k.account_id);
        put_u64(&mut out, k.client_order_id.0)
    }
    put_u64(&mut out, ledger.replaced_orders);
    put_u32(&mut out, ledger.trades.len() as u32);
    for t in &ledger.trades {
        put_u16(&mut out, t.instrument_id);
        put_u64(&mut out, t.buyer_exchange_order_id.0);
        put_u64(&mut out, t.seller_exchange_order_id.0);
        put_u64(&mut out, t.buyer_client_order_id.0);
        put_u64(&mut out, t.seller_client_order_id.0);
        put_u32(&mut out, t.price);
        put_u32(&mut out, t.qty);
        put_u64(&mut out, t.buyer_sequence_number.0);
        put_u64(&mut out, t.seller_sequence_number.0);
        put_u64(&mut out, t.timestamp)
    }
    out
}
fn decode(bytes: &[u8], journal_sequence: u64) -> Result<(u64, ReplayLedger), SnapshotError> {
    let mut r = Reader { bytes, pos: 0 };
    let event_count = r.u64()?;
    let n = r.u32()? as usize;
    let mut accepted_orders = BTreeMap::new();
    for _ in 0..n {
        let k = ReplayOrderKey {
            instrument_id: r.u16()?,
            account_id: r.u32()?,
            client_order_id: ClientOrderId(r.u64()?),
        };
        accepted_orders.insert(k, ExchangeOrderId(r.u64()?));
    }
    let n = r.u32()? as usize;
    let mut cancelled_orders = HashSet::new();
    for _ in 0..n {
        cancelled_orders.insert(ReplayOrderKey {
            instrument_id: r.u16()?,
            account_id: r.u32()?,
            client_order_id: ClientOrderId(r.u64()?),
        });
    }
    let replaced_orders = r.u64()?;
    let n = r.u32()? as usize;
    let mut trades = Vec::with_capacity(n);
    for _ in 0..n {
        trades.push(ReplayTrade {
            instrument_id: r.u16()?,
            buyer_exchange_order_id: ExchangeOrderId(r.u64()?),
            seller_exchange_order_id: ExchangeOrderId(r.u64()?),
            buyer_client_order_id: ClientOrderId(r.u64()?),
            seller_client_order_id: ClientOrderId(r.u64()?),
            price: r.u32()?,
            qty: r.u32()?,
            buyer_sequence_number: SequenceNumber(r.u64()?),
            seller_sequence_number: SequenceNumber(r.u64()?),
            timestamp: r.u64()?,
        });
    }
    if r.pos != bytes.len() {
        return Err(SnapshotError::InvalidData("trailing bytes"));
    }
    Ok((
        event_count,
        ReplayLedger {
            event_count,
            last_sequence: if journal_sequence == 0 {
                None
            } else {
                Some(journal_sequence)
            },
            accepted_orders,
            cancelled_orders,
            replaced_orders,
            trades,
        },
    ))
}
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], SnapshotError> {
        let e = self.pos.checked_add(n).ok_or(SnapshotError::Truncated)?;
        if e > self.bytes.len() {
            return Err(SnapshotError::Truncated);
        }
        let o = &self.bytes[self.pos..e];
        self.pos = e;
        Ok(o)
    }
    fn u16(&mut self) -> Result<u16, SnapshotError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, SnapshotError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, SnapshotError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}
fn put_u16(o: &mut Vec<u8>, v: u16) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn put_u32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn put_u64(o: &mut Vec<u8>, v: u64) {
    o.extend_from_slice(&v.to_le_bytes())
}
fn temp_path(p: &Path) -> PathBuf {
    p.with_extension("tmp")
}
fn crc32(b: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &x in b {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xedb8_8320
            } else {
                c >> 1
            }
        }
    }
    !c
}
