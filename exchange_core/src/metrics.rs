//! Performance counters and microsecond latency telemetry.

use std::sync::atomic::{AtomicU64, Ordering};

pub struct PerformanceMetrics {
    pub orders_processed: AtomicU64,
    pub trades_generated: AtomicU64,
    pub cumulative_service_nanos: AtomicU64,
    pub min_latency_nanos: AtomicU64,
    pub max_latency_nanos: AtomicU64,
}

impl PerformanceMetrics {
    pub const fn new() -> Self {
        Self {
            orders_processed: AtomicU64::new(0),
            trades_generated: AtomicU64::new(0),
            cumulative_service_nanos: AtomicU64::new(0),
            min_latency_nanos: AtomicU64::new(u64::MAX),
            max_latency_nanos: AtomicU64::new(0),
        }
    }

    #[inline(always)]
    pub fn record_order_latency(&self, duration_nanos: u64, trades: usize) {
        self.orders_processed.fetch_add(1, Ordering::Relaxed);
        self.trades_generated.fetch_add(trades as u64, Ordering::Relaxed);
        self.cumulative_service_nanos.fetch_add(duration_nanos, Ordering::Relaxed);

        // Update min
        let mut current_min = self.min_latency_nanos.load(Ordering::Relaxed);
        while duration_nanos < current_min {
            match self.min_latency_nanos.compare_exchange_weak(
                current_min,
                duration_nanos,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_min = actual,
            }
        }

        // Update max
        let mut current_max = self.max_latency_nanos.load(Ordering::Relaxed);
        while duration_nanos > current_max {
            match self.max_latency_nanos.compare_exchange_weak(
                current_max,
                duration_nanos,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }
    }

    pub fn average_latency_micros(&self) -> f64 {
        let count = self.orders_processed.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        let total_nanos = self.cumulative_service_nanos.load(Ordering::Relaxed);
        (total_nanos as f64 / count as f64) / 1000.0
    }

    pub fn min_latency_nanos(&self) -> u64 {
        let val = self.min_latency_nanos.load(Ordering::Relaxed);
        if val == u64::MAX { 0 } else { val }
    }

    pub fn max_latency_nanos(&self) -> u64 {
        self.max_latency_nanos.load(Ordering::Relaxed)
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

