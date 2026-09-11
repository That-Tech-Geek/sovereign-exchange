use crossbeam_channel::{bounded, Sender, Receiver};
use crate::constants::RING_BUFFER_SIZE;

/// LMAX Disruptor-style ring buffer using crossbeam's lock-free bounded channel.
/// Serves as the communication highway between network ingress threads and the single matching core.
#[derive(Clone)]
pub struct RingBuffer {
    pub tx: Sender<u32>,  // Order indices (already allocated in the OrderPool)
    pub rx: Receiver<u32>,
}

impl RingBuffer {
    pub fn new() -> Self {
        let (tx, rx) = bounded(RING_BUFFER_SIZE);
        Self { tx, rx }
    }

    /// Send an order index into the ring buffer (non-blocking).
    /// Returns Ok(()) if sent, Err if the ring buffer is at capacity.
    #[inline(always)]
    pub fn send(&self, idx: u32) -> Result<(), crossbeam_channel::TrySendError<u32>> {
        self.tx.try_send(idx)
    }

    /// Try to pop an order index without blocking the matching thread.
    /// Returns Some(idx) if available, None if empty.
    #[inline(always)]
    pub fn try_recv(&self) -> Option<u32> {
        self.rx.try_recv().ok()
    }

    /// Blocking receive (used by utility/test consumers).
    pub fn recv_blocking(&self) -> u32 {
        self.rx.recv().expect("RingBuffer sender closed")
    }

    /// Returns the approximate number of pending orders in the ring buffer.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.rx.len()
    }

    /// Returns true if empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.rx.is_empty()
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new()
    }
}
