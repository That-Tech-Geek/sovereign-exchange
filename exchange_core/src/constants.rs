//! Single source of truth for all exchange core tunable parameters.

pub const MAX_ORDERS: usize = 5_000_000;
pub const MAX_PRICE_LEVELS: usize = 100_000;
pub const RING_BUFFER_SIZE: usize = 1_048_576;
pub const MAX_TRADES_PER_MATCH: usize = 64;
pub const MAX_SOVEREIGNS: usize = 196;
pub const MAX_INSTRUMENTS: usize = MAX_SOVEREIGNS * 2;
pub const UDP_PORT: u16 = 8888;
pub const HTTP_PORT: u16 = 8080;
pub const WAL_BUFFER_SIZE: usize = 1024 * 1024;
pub const FIRESTORE_SYNC_MS: u64 = 500;
pub const SNAPSHOT_INTERVAL_SECS: u64 = 60;
pub const GITHUB_CHUNK_SIZE: u64 = 1_000_000_000;
pub const CPU_CORE_MATCHING: usize = 0;
pub const CPU_CORE_SCAVENGER: usize = 1;
pub const NULL_ORDER: u32 = 0;
