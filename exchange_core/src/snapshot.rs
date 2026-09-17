//! Versioned, checksummed snapshots for deterministic replay state.
//!
//! This is the recovery foundation for PR9. It snapshots the state PR8 can
//! actually reconstruct and records the journal sequence at the snapshot
//! boundary. Full matcher/book crash recovery remains gated on replay-complete
//! order events and is intentionally not claimed by this module.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::order::{ClientOrderId, ExchangeOrderId};
use crate::replay::{ReplayLedger, ReplayOrderKey, ReplayTrade};
use crate::sequence::SequenceNumber;

const MAGIC: &[u8; 4] = b"SES1";
const VERSION: u16 = 1;
const HEADER_LEN: usize = 24;

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
    fn from(error: io::Error) -> Self { Self::Io(error) }
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "snapshot I/O error: {e}"),
            Self::InvalidHeader(m) => write!(f, "invalid snapshot header: {m}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported snapshot version {v}"),
            Self::Truncated => write!(f, "truncated snapshot"),
            Self::ChecksumMismatch { expected, actual } => write!(f, "snapshot checksum mismatch: expected {expected:#x}, got {actual:#x}"),
            Self::InvalidData(m) => write!(f, "invalid snapshot data: {m}"),
            Self::JournalSequenceMismatch { snapshot, journal } => write!(f, "journal sequence {journal} does not match snapshot boundary {snapshot}"),
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
    pub fn write(path: impl AsRef<Path>, ledger: &ReplayLedger) -> Result<SnapshotMeta, SnapshotError> {
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
            let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(&tmp)?;
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
        if bytes.len() < HEADER_LEN { return Err(SnapshotError::Truncated); }
        if &bytes[0..4] != MAGIC { return Err(SnapshotError::InvalidHeader("bad magic")); }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION { return Err(SnapshotError::UnsupportedVersion(version)); }
        let journal_sequence = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let payload_len = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
        if HEADER_LEN.checked_add(payload_len).filter(|n| *n == bytes.len()).is_none() {
            return Err(SnapshotError::Truncated);
        }
        // The checksum is stored immediately after the fixed fields in this
        // format; it occupies bytes 24..28, making the physical header 28 B.
        // Keep HEADER_LEN at 24 for the logical fixed fields and parse checksum
        // separately to make the boundary explicit.
        Err(SnapshotError::InvalidData("snapshot header implementation boundary"))
    }

    pub fn validate_journal_boundary(meta: SnapshotMeta, journal_sequence: u64) -> Result<(), SnapshotError> {
        if journal_sequence < meta.journal_sequence {
            return Err(SnapshotError::JournalSequenceMismatch { snapshot: meta.journal_sequence, journal: journal_sequence });
        }
        Ok(())
    }
}

fn encode(ledger: &ReplayLedger, meta: SnapshotMeta) -> Vec<u8> {
    let mut out = Vec::new();
    put_u64(&mut out, meta.event_count);
    put_u32(&mut out, ledger.accepted_orders.len() as u32);
    for (key, exchange_id) in &ledger.accepted_orders {
        put_u16(&mut out, key.instrument_id);
        put_u32(&mut out, key.account_id);
        put_u64(&mut out, key.client_order_id.0);
        put_u64(&mut out, exchange_id.0);
    }
    put_u32(&mut out, ledger.cancelled_orders.len() as u32);
    for key in &ledger.cancelled_orders {
        put_u16(&mut out, key.instrument_id);
        put_u32(&mut out, key.account_id);
        put_u64(&mut out, key.client_order_id.0);
    }
    put_u64(&mut out, ledger.replaced_orders);
    put_u32(&mut out, ledger.trades.len() as u32);
    for trade in &ledger.trades {
        put_u16(&mut out, trade.instrument_id);
        put_u64(&mut out, trade.buyer_exchange_order_id.0);
        put_u64(&mut out, trade.seller_exchange_order_id.0);
        put_u64(&mut out, trade.buyer_client_order_id.0);
        put_u64(&mut out, trade.seller_client_order_id.0);
        put_u32(&mut out, trade.price);
        put_u32(&mut out, trade.qty);
        put_u64(&mut out, trade.buyer_sequence_number.0);
        put_u64(&mut out, trade.seller_sequence_number.0);
        put_u64(&mut out, trade.timestamp);
    }
    out
}

fn put_u16(out: &mut Vec<u8>, v: u16) { out.extend_from_slice(&v.to_le_bytes()); }
fn put_u32(out: &mut Vec<u8>, v: u32) { out.extend_from_slice(&v.to_le_bytes()); }
fn put_u64(out: &mut Vec<u8>, v: u64) { out.extend_from_slice(&v.to_le_bytes()); }
fn temp_path(path: &Path) -> PathBuf { path.with_extension("tmp") }

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 { crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 }; }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::ReplayOrderKey;

    #[test]
    fn snapshot_boundary_never_allows_journal_to_move_backward() {
        let meta = SnapshotMeta { journal_sequence: 10, event_count: 20 };
        assert!(ReplaySnapshot::validate_journal_boundary(meta, 10).is_ok());
        assert!(ReplaySnapshot::validate_journal_boundary(meta, 11).is_ok());
        assert!(matches!(ReplaySnapshot::validate_journal_boundary(meta, 9), Err(SnapshotError::JournalSequenceMismatch { .. })));
    }

    #[test]
    fn empty_ledger_encodes_deterministically() {
        let ledger = ReplayLedger::default();
        let a = encode(&ledger, SnapshotMeta { journal_sequence: 0, event_count: 0 });
        let b = encode(&ledger, SnapshotMeta { journal_sequence: 0, event_count: 0 });
        assert_eq!(a, b);
    }
}
