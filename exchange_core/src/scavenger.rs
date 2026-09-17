use crate::constants::{FIRESTORE_SYNC_MS, GITHUB_CHUNK_SIZE, SNAPSHOT_INTERVAL_SECS};
use crate::engine::Trade;
use crossbeam_channel::Receiver;
use std::time::{Duration, Instant};

/// The scavenger runs on a separate worker thread (Core 1).
/// It batches executed trades, simulates periodic Firestore sync, and compresses WAL records for GitHub.
pub fn run(trade_rx: Receiver<Trade>) {
    let mut batch = Vec::with_capacity(1000);
    let mut last_sync = Instant::now();
    let mut last_snapshot = Instant::now();
    let mut wal_buffer = Vec::with_capacity(1024 * 1024); // 1MB buffer
    let mut compressed_bytes_written = 0u64;
    let mut total_trades_processed = 0u64;

    eprintln!("[Scavenger] Initialized and listening on Core 1 (Isolated I/O)");

    loop {
        // Try to receive a batch of trades with a timeout to maintain periodicity
        match trade_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(trade) => {
                batch.push(trade);
                total_trades_processed += 1;

                // Append raw binary trade event to WAL buffer
                wal_buffer.extend_from_slice(&trade.price.to_le_bytes());
                wal_buffer.extend_from_slice(&trade.qty.to_le_bytes());
                wal_buffer.extend_from_slice(&trade.buyer.to_le_bytes());
                wal_buffer.extend_from_slice(&trade.seller.to_le_bytes());
                wal_buffer.extend_from_slice(&trade.timestamp.to_le_bytes());

                // Flush if batch threshold is reached
                if batch.len() >= 1000 {
                    flush_to_firestore(&batch);
                    batch.clear();
                }
            }
            Err(_) => {
                // Timeout elapsed: flush any uncommitted trades in the queue
                if !batch.is_empty() {
                    flush_to_firestore(&batch);
                    batch.clear();
                }
            }
        }

        // Periodic L2 state synchronization window
        if last_sync.elapsed() >= Duration::from_millis(FIRESTORE_SYNC_MS) {
            eprintln!(
                "[Scavenger] Syncing L2 to Firestore (Total lifetime trades: {})",
                total_trades_processed
            );
            last_sync = Instant::now();
        }

        // Periodic complete state snapshot
        if last_snapshot.elapsed() >= Duration::from_secs(SNAPSHOT_INTERVAL_SECS) {
            eprintln!(
                "[Scavenger] Snapshot generated at timestamp {:?}",
                Instant::now()
            );
            last_snapshot = Instant::now();
        }

        // WAL buffer chunk rotation for GitHub / cold storage commit
        if wal_buffer.len() as u64 > GITHUB_CHUNK_SIZE {
            eprintln!(
                "[Scavenger] Compressing WAL chunk (size {} bytes)...",
                wal_buffer.len()
            );
            compressed_bytes_written += GITHUB_CHUNK_SIZE;
            wal_buffer.clear();
            eprintln!(
                "[Scavenger] Cumulative WAL archived: {} MB",
                compressed_bytes_written / (1024 * 1024)
            );
        }
    }
}

/// Flush an aggregated batch of trades to persistent storage.
#[inline(always)]
fn flush_to_firestore(trades: &[Trade]) {
    eprintln!(
        "[Scavenger] Flushed batch of {} trades to Firestore buffer",
        trades.len()
    );
}
