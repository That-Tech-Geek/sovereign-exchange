use crate::pool::OrderPool;

/// Binary protocol for orders (32 bytes total).
/// Designed for zero-copy parsing from UDP network packets.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct OrderPacket {
    pub order_id: u64,   // 8 bytes: Client order sequence
    pub account_id: u32, // 4 bytes: Bot / account ID
    pub ticker_id: u16,  // 2 bytes: Country index (0=USA, 1=GERMANY, etc.)
    pub side: u8,        // 1 byte: 0 = Buy, 1 = Sell
    pub price: u32,      // 4 bytes: Scaled integer (e.g. 10050 = $100.50)
    pub quantity: u32,   // 4 bytes: Total quantity
    pub timestamp: u64,  // 8 bytes: Client timestamp in unix nanos
    pub _pad: u8,        // 1 byte: Network alignment padding (32 bytes total)
}

impl OrderPacket {
    /// Parse from raw bytes. Assumes little-endian.
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) }
    }

    /// Serialize into 32 raw bytes (for testing & network egress).
    #[inline(always)]
    pub fn to_bytes(self) -> [u8; 32] {
        unsafe { std::mem::transmute_copy(&self) }
    }
}

impl OrderPool {
    /// Allocate and initialize an order from a packet.
    #[inline(always)]
    pub fn allocate_from_packet(&mut self, packet: &OrderPacket) -> u32 {
        let idx = self.allocate();
        let order = &mut self.data[idx as usize];
        order.order_id = packet.order_id;
        order.account_id = packet.account_id;
        order.ticker_id = packet.ticker_id;
        order.side = packet.side;
        order.price = packet.price;
        order.quantity = packet.quantity;
        order.remaining = packet.quantity;
        order.timestamp = packet.timestamp;
        order.next = 0;
        order.prev = 0;
        idx
    }
}
