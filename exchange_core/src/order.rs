use crate::pool::OrderPool;

/// Binary order packet. The wire layout remains 32 bytes for this PR.
///
/// `instrument_id` is the authoritative market/book selector. It replaces the
/// old `ticker_id` terminology without changing the packet width.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct OrderPacket {
    pub order_id: u64,       // 8 bytes: existing client/exchange order ID field
    pub account_id: u32,     // 4 bytes: account ID
    pub instrument_id: u16,  // 2 bytes: instrument ID [0, MAX_INSTRUMENTS)
    pub side: u8,            // 1 byte: 0 = Buy, 1 = Sell, 2 = Cancel (legacy command encoding)
    pub price: u32,          // 4 bytes: scaled integer price
    pub quantity: u32,       // 4 bytes: total quantity
    pub timestamp: u64,      // 8 bytes: client timestamp metadata
    pub _pad: u8,            // 1 byte: protocol padding
}

impl OrderPacket {
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) }
    }

    #[inline(always)]
    pub fn to_bytes(&self) -> [u8; 32] {
        unsafe { std::mem::transmute_copy(self) }
    }
}

impl OrderPool {
    #[inline(always)]
    pub fn allocate_from_packet(&mut self, packet: &OrderPacket) -> u32 {
        let idx = self.allocate();
        let order = &mut self.data[idx as usize];
        order.order_id = packet.order_id;
        order.account_id = packet.account_id;
        order.instrument_id = packet.instrument_id;
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
