use crate::pool::OrderPool;

/// Client-supplied identifier for an order.
///
/// It is not authoritative exchange identity. Uniqueness is enforced per
/// `(instrument_id, account_id, client_order_id)` while the order is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ClientOrderId(pub u64);

/// Exchange-assigned identifier for an accepted order.
///
/// Exchange IDs are monotonically allocated by the single matching engine and
/// are independent of client identifiers. Zero is reserved for non-resting
/// command records such as cancellation requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ExchangeOrderId(pub u64);

/// Active-order lookup key. The instrument is deliberately part of the key so
/// the identity contract remains compatible with instrument-scoped matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderKey {
    pub instrument_id: u16,
    pub account_id: u32,
    pub client_order_id: ClientOrderId,
}

/// Binary order packet (32 bytes total).
///
/// The existing `order_id` wire field is now explicitly a `client_order_id`.
/// The exchange order ID is assigned after validation at ingress and is never
/// supplied by the client.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct OrderPacket {
    pub order_id: u64,       // 8 bytes: client_order_id
    pub account_id: u32,     // 4 bytes: account ID
    pub instrument_id: u16,  // 2 bytes: instrument ID
    pub side: u8,            // 1 byte: 0 = Buy, 1 = Sell, 2 = Cancel
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

    #[inline(always)]
    pub fn client_order_id(&self) -> ClientOrderId {
        ClientOrderId(self.order_id)
    }
}

impl OrderPool {
    /// Allocate an accepted order with an exchange-assigned identity.
    #[inline(always)]
    pub fn allocate_from_packet(
        &mut self,
        packet: &OrderPacket,
        exchange_order_id: ExchangeOrderId,
    ) -> u32 {
        let idx = self.allocate();
        let order = &mut self.data[idx as usize];
        order.exchange_order_id = exchange_order_id.0;
        order.client_order_id = packet.order_id;
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
