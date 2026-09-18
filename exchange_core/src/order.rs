use crate::command::OrderCommand;
use crate::pool::OrderPool;
use crate::sequence::SequenceNumber;

/// Client-supplied identifier for an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ClientOrderId(pub u64);

/// Exchange-assigned identifier for an accepted order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ExchangeOrderId(pub u64);

/// Active-order lookup key within one instrument book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderKey {
    pub account_id: u32,
    pub client_order_id: ClientOrderId,
}

/// Binary order packet (32 bytes total).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct OrderPacket {
    pub client_order_id: u64,
    pub account_id: u32,
    pub instrument_id: u16,
    pub side: u8,
    pub price: u32,
    pub quantity: u32,
    pub timestamp: u64,
    pub _pad: u8,
}

impl OrderPacket {
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) }
    }

    #[inline(always)]
    pub fn to_bytes(self) -> [u8; 32] {
        unsafe { std::mem::transmute_copy(&self) }
    }

    #[inline(always)]
    pub fn client_order_id(&self) -> ClientOrderId {
        ClientOrderId(self.client_order_id)
    }
}

#[allow(dead_code)]
fn _order_pool_api_anchor(_: &mut OrderPool, _: &OrderCommand, _: SequenceNumber) {}
