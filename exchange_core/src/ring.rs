use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use crate::constants::RING_BUFFER_SIZE;

/// Bounded non-blocking order ingress queue.
///
/// This is intentionally not called an LMAX Disruptor: it is a Crossbeam
/// bounded channel carrying already-allocated order-pool indices.
#[derive(Clone)]
pub struct OrderQueue {
    pub tx: Sender<u32>,
    pub rx: Receiver<u32>,
}

impl OrderQueue {
    pub fn new() -> Self {
        let (tx, rx) = bounded(RING_BUFFER_SIZE);
        Self { tx, rx }
    }

    /// Non-blocking enqueue. A Full/Disconnected error returns ownership of
    /// the order index to the caller so the pool slot can be released.
    #[inline(always)]
    pub fn try_send(&self, idx: u32) -> Result<(), TrySendError<u32>> {
        self.tx.try_send(idx)
    }

    #[inline(always)]
    pub fn try_recv(&self) -> Option<u32> {
        self.rx.try_recv().ok()
    }

    pub fn recv_blocking(&self) -> u32 {
        self.rx.recv().expect("OrderQueue sender closed")
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.rx.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.rx.is_empty()
    }
}

impl Default for OrderQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Compatibility alias for existing health/test code. New code should use
/// `OrderQueue` because this is a bounded channel, not a literal ring buffer.
pub type RingBuffer = OrderQueue;
