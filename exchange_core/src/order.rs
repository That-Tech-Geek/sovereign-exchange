use crate::command::{NewOrder, OrderCommand, OrderSide, ReplaceOrder};
use crate::pool::{CommandKind, OrderPool};
use crate::sequence::SequenceNumber;

/// Client-supplied identifier for an order.
///
/// Uniqueness is enforced per `(instrument_id, account_id, client_order_id)`
/// while an order is active. The instrument scope is provided by the owning
/// `OrderBook`; it is not duplicated in the key stored by that book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ClientOrderId(pub u64);

/// Exchange-assigned identifier for an accepted order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ExchangeOrderId(pub u64);

/// Active-order lookup key within one instrument book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderKey {
    pub account_id: u32,
    pub client_order_id: ClientOrderId,
}

/// Binary order packet (32 bytes total).
///
/// The packet is a transport representation only. Its legacy numeric `side`
/// field is decoded once by `OrderCommand::from_packet`; matching logic uses
/// typed commands and never interprets `side == 2` as cancellation.
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
    pub fn to_bytes(&self) -> [u8; 32] {
        unsafe { std::mem::transmute_copy(self) }
    }

    #[inline(always)]
    pub fn client_order_id(&self) -> ClientOrderId {
        ClientOrderId(self.client_order_id)
    }
}

impl OrderPool {
    #[inline(always)]
    pub fn allocate_from_packet(
        &mut self,
        packet: &OrderPacket,
        exchange_order_id: ExchangeOrderId,
    ) -> u32 {
        self.allocate_from_packet_with_sequence(packet, exchange_order_id, SequenceNumber(0))
    }

    #[inline(always)]
    pub fn allocate_from_packet_with_sequence(
        &mut self,
        packet: &OrderPacket,
        exchange_order_id: ExchangeOrderId,
        sequence_number: SequenceNumber,
    ) -> u32 {
        let command = OrderCommand::from_packet(packet).expect("legacy packet must decode");
        self.allocate_from_command(&command, exchange_order_id, sequence_number)
    }

    /// Allocate a command after admission. The pool representation is an
    /// internal command envelope; protocol-specific numeric side values do not
    /// leak into matching semantics.
    #[inline(always)]
    pub fn allocate_from_command(
        &mut self,
        command: &OrderCommand,
        exchange_order_id: ExchangeOrderId,
        sequence_number: SequenceNumber,
    ) -> u32 {
        let idx = self.allocate();
        let order = &mut self.data[idx as usize];
        *order = crate::pool::Order::default();
        order.exchange_order_id = exchange_order_id.0;
        order.sequence_number = sequence_number.0;

        match *command {
            OrderCommand::New(NewOrder {
                client_order_id,
                account_id,
                instrument_id,
                side,
                price,
                quantity,
                client_timestamp,
            }) => {
                order.command_kind = CommandKind::New as u8;
                order.client_order_id = client_order_id.0;
                order.account_id = account_id;
                order.instrument_id = instrument_id;
                order.side = side.wire_value();
                order.price = price;
                order.quantity = quantity;
                order.remaining = quantity;
                order.client_timestamp = client_timestamp;
            }
            OrderCommand::Cancel(cancel) => {
                order.command_kind = CommandKind::Cancel as u8;
                order.client_order_id = cancel.client_order_id.0;
                order.account_id = cancel.account_id;
                order.instrument_id = cancel.instrument_id;
            }
            OrderCommand::Replace(ReplaceOrder {
                account_id,
                instrument_id,
                target_client_order_id,
                new_client_order_id,
                side,
                price,
                quantity,
                client_timestamp,
            }) => {
                order.command_kind = CommandKind::Replace as u8;
                order.client_order_id = new_client_order_id.0;
                order.account_id = account_id;
                order.instrument_id = instrument_id;
                order.side = side.wire_value();
                order.price = price;
                order.quantity = quantity;
                order.remaining = quantity;
                order.client_timestamp = client_timestamp;
                order.replace_target_client_order_id = target_client_order_id.0;
            }
        }

        order.next = 0;
        order.prev = 0;
        idx
    }
}
