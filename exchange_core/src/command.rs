use crate::order::{ClientOrderId, ExchangeOrderId, OrderPacket};
use crate::sequence::SequenceNumber;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderCommand {
    New(NewOrder),
    Cancel(CancelOrder),
    Replace(ReplaceOrder),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewOrder {
    pub client_order_id: ClientOrderId,
    pub account_id: u32,
    pub instrument_id: u16,
    pub side: OrderSide,
    pub price: u32,
    pub quantity: u32,
    pub client_timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelOrder {
    pub account_id: u32,
    pub instrument_id: u16,
    pub client_order_id: ClientOrderId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplaceOrder {
    pub account_id: u32,
    pub instrument_id: u16,
    pub target_client_order_id: ClientOrderId,
    pub new_client_order_id: ClientOrderId,
    pub side: OrderSide,
    pub price: u32,
    pub quantity: u32,
    pub client_timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    #[inline(always)]
    pub const fn wire_value(self) -> u8 {
        match self {
            Self::Buy => 0,
            Self::Sell => 1,
        }
    }

    #[inline(always)]
    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Buy),
            1 => Some(Self::Sell),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRejectReason {
    InvalidInstrument,
    InvalidSide,
    DuplicateClientOrderId,
    UnknownOrder,
    ExchangeOrderIdExhausted,
    SequenceNumberExhausted,
    OrderPoolExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeEvent {
    /// Contains the complete immutable order attributes required to rebuild
    /// resting state without re-running the matcher.
    OrderAccepted {
        instrument_id: u16,
        account_id: u32,
        client_order_id: ClientOrderId,
        exchange_order_id: ExchangeOrderId,
        sequence_number: SequenceNumber,
        side: OrderSide,
        price: u32,
        quantity: u32,
        client_timestamp: u64,
    },
    OrderRejected {
        account_id: u32,
        client_order_id: ClientOrderId,
        sequence_number: Option<SequenceNumber>,
        reason: CommandRejectReason,
    },
    OrderCancelled {
        instrument_id: u16,
        account_id: u32,
        client_order_id: ClientOrderId,
        sequence_number: SequenceNumber,
    },
    OrderReplaced {
        instrument_id: u16,
        account_id: u32,
        old_client_order_id: ClientOrderId,
        new_client_order_id: ClientOrderId,
        exchange_order_id: ExchangeOrderId,
        sequence_number: SequenceNumber,
    },
    Trade {
        buyer: u32,
        seller: u32,
        instrument_id: u16,
        buyer_exchange_order_id: ExchangeOrderId,
        seller_exchange_order_id: ExchangeOrderId,
        buyer_client_order_id: ClientOrderId,
        seller_client_order_id: ClientOrderId,
        buyer_sequence_number: SequenceNumber,
        seller_sequence_number: SequenceNumber,
        price: u32,
        qty: u32,
        timestamp: u64,
    },
}

impl OrderCommand {
    pub fn from_packet(packet: &OrderPacket) -> Result<Self, CommandRejectReason> {
        match packet.side {
            0 => Ok(Self::New(NewOrder {
                client_order_id: packet.client_order_id(),
                account_id: packet.account_id,
                instrument_id: packet.instrument_id,
                side: OrderSide::Buy,
                price: packet.price,
                quantity: packet.quantity,
                client_timestamp: packet.timestamp,
            })),
            1 => Ok(Self::New(NewOrder {
                client_order_id: packet.client_order_id(),
                account_id: packet.account_id,
                instrument_id: packet.instrument_id,
                side: OrderSide::Sell,
                price: packet.price,
                quantity: packet.quantity,
                client_timestamp: packet.timestamp,
            })),
            2 => Ok(Self::Cancel(CancelOrder {
                account_id: packet.account_id,
                instrument_id: packet.instrument_id,
                client_order_id: packet.client_order_id(),
            })),
            _ => Err(CommandRejectReason::InvalidSide),
        }
    }
}
