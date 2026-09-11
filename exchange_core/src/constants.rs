//! Single source of truth for all exchange core tunable parameters.

pub const MAX_ORDERS: usize = 5_000_000;        // 5M orders ~ 320MB RAM (safety margin)
pub const MAX_PRICE_LEVELS: usize = 100_000;    // Depth per ticker
pub const RING_BUFFER_SIZE: usize = 1_048_576;  // 2^20 slots
pub const MAX_TRADES_PER_MATCH: usize = 64;     // Stack buffer for trades
pub const MAX_TICKERS: usize = 100;              // Country count
pub const UDP_PORT: u16 = 8888;                 // Bot ingress
pub const HTTP_PORT: u16 = 8080;                // Health checks
pub const WAL_BUFFER_SIZE: usize = 1024 * 1024; // 1MB compression buffer
pub const FIRESTORE_SYNC_MS: u64 = 500;          // 500ms batch window
pub const SNAPSHOT_INTERVAL_SECS: u64 = 60;      // Full snapshot every 60s
pub const GITHUB_CHUNK_SIZE: u64 = 1_000_000_000; // 1GB per git commit
pub const CPU_CORE_MATCHING: usize = 0;          // Core for matching
pub const CPU_CORE_SCAVENGER: usize = 1;         // Core for async tasks
pub const NULL_ORDER: u32 = 0;                  // Sentinel null order index
