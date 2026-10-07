use crate::command::{OrderCommand, OrderSide};
use crate::constants::{MAX_ORDERS, NULL_ORDER};
use crate::order::{ExchangeOrderId, OrderPacket};
use crate::sequence::SequenceNumber;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandKind {
    New = 0,
    Cancel = 1,
    Replace = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError {
    Exhausted,
}

/// Compact hot-path order state.
///
/// Five million slots at 44 bytes consume 220,000,044 bytes including the
/// reserved index 0. Instrument, side and command kind are bit-packed into
/// one u32. For a pending Replace command, the prev field temporarily carries
/// the target client-order ID; it is cleared before the replacement enters the
/// book and resumes its normal FIFO-link role.
#[repr(C, packed(4))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    pub exchange_order_id: u64,
    pub client_order_id: u64,
    pub sequence_number: u64,
    pub account_id: u32,
    pub price: u32,
    pub remaining: u32,
    pub next: u32,
    pub prev: u32,
    meta: u32,
}

const _: () = assert!(std::mem::size_of::<Order>() == 44);

const INSTRUMENT_MASK: u32 = 0x0000_FFFF;
const SIDE_SHIFT: u32 = 16;
const KIND_SHIFT: u32 = 18;
const SIDE_MASK: u32 = 0x3;
const KIND_MASK: u32 = 0x3;

impl Order {
    #[inline(always)]
    pub fn instrument_id(&self) -> u16 {
        (self.meta & INSTRUMENT_MASK) as u16
    }

    #[inline(always)]
    pub fn side(&self) -> u8 {
        ((self.meta >> SIDE_SHIFT) & SIDE_MASK) as u8
    }

    #[inline(always)]
    pub fn command_kind(&self) -> u8 {
        ((self.meta >> KIND_SHIFT) & KIND_MASK) as u8
    }

    #[inline(always)]
    fn set_meta(&mut self, instrument_id: u16, side: u8, command_kind: CommandKind) {
        self.meta = instrument_id as u32
            | ((side as u32 & SIDE_MASK) << SIDE_SHIFT)
            | (((command_kind as u32) & KIND_MASK) << KIND_SHIFT);
    }

    #[inline(always)]
    fn set_command_kind(&mut self, command_kind: CommandKind) {
        self.meta = (self.meta & !(KIND_MASK << KIND_SHIFT))
            | (((command_kind as u32) & KIND_MASK) << KIND_SHIFT);
    }

    #[inline(always)]
    pub fn set_command_kind_new(&mut self) {
        self.set_command_kind(CommandKind::New);
    }
}

pub struct OrderPool {
    pub data: Vec<Order>,
    pub free_head: u32,
    pub allocated_count: u32,
}

impl Default for OrderPool {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderPool {
    pub fn new() -> Self {
        let mut data = Vec::with_capacity(MAX_ORDERS + 1);
        data.resize(MAX_ORDERS + 1, Order::default());
        for (i, order) in data.iter_mut().enumerate().take(MAX_ORDERS).skip(1) {
            order.next = (i + 1) as u32;
        }
        data[MAX_ORDERS].next = u32::MAX;
        Self {
            data,
            free_head: 1,
            allocated_count: 0,
        }
    }

    #[inline(always)]
    pub fn allocate(&mut self) -> Result<u32, PoolError> {
        let idx = self.free_head;
        if idx == u32::MAX {
            return Err(PoolError::Exhausted);
        }
        self.free_head = self.data[idx as usize].next;
        self.data[idx as usize] = Order::default();
        self.allocated_count += 1;
        Ok(idx)
    }

    #[inline(always)]
    pub fn allocate_from_command(
        &mut self,
        command: &OrderCommand,
        exchange_order_id: ExchangeOrderId,
        sequence_number: SequenceNumber,
    ) -> Result<u32, PoolError> {
        let idx = self.allocate()?;
        let order = &mut self.data[idx as usize];
        match *command {
            OrderCommand::New(order_cmd) => {
                order.client_order_id = order_cmd.client_order_id.0;
                order.account_id = order_cmd.account_id;
                order.set_meta(
                    order_cmd.instrument_id,
                    order_cmd.side.wire_value(),
                    CommandKind::New,
                );
                order.price = order_cmd.price;
                order.remaining = order_cmd.quantity;
                order.sequence_number = sequence_number.0;
            }
            OrderCommand::Cancel(cancel_cmd) => {
                order.client_order_id = cancel_cmd.client_order_id.0;
                order.account_id = cancel_cmd.account_id;
                order.set_meta(cancel_cmd.instrument_id, 0, CommandKind::Cancel);
                order.sequence_number = sequence_number.0;
            }
            OrderCommand::Replace(replace_cmd) => {
                if replace_cmd.target_client_order_id.0 > u32::MAX as u64 {
                    self.deallocate(idx);
                    return Err(PoolError::Exhausted);
                }
                order.client_order_id = replace_cmd.new_client_order_id.0;
                order.account_id = replace_cmd.account_id;
                order.set_meta(
                    replace_cmd.instrument_id,
                    replace_cmd.side.wire_value(),
                    CommandKind::Replace,
                );
                order.price = replace_cmd.price;
                order.remaining = replace_cmd.quantity;
                order.sequence_number = sequence_number.0;
                order.prev = replace_cmd.target_client_order_id.0 as u32;
            }
        }
        order.exchange_order_id = exchange_order_id.0;
        Ok(idx)
    }

    #[inline(always)]
    pub fn allocate_from_packet(
        &mut self,
        packet: &OrderPacket,
        exchange_order_id: ExchangeOrderId,
    ) -> Result<u32, PoolError> {
        let side = match packet.side {
            0 => OrderSide::Buy,
            1 => OrderSide::Sell,
            _ => return Err(PoolError::Exhausted),
        };
        let command = OrderCommand::New(crate::command::NewOrder {
            client_order_id: packet.client_order_id(),
            account_id: packet.account_id,
            instrument_id: packet.instrument_id,
            side,
            price: packet.price,
            quantity: packet.quantity,
            client_timestamp: packet.timestamp,
        });
        self.allocate_from_command(&command, exchange_order_id, SequenceNumber::FIRST)
    }

    #[inline(always)]
    pub fn deallocate(&mut self, idx: u32) {
        if idx == NULL_ORDER {
            return;
        }
        self.data[idx as usize].next = self.free_head;
        self.free_head = idx;
        self.allocated_count -= 1;
    }

    #[inline(always)]
    pub fn get_mut(&mut self, idx: u32) -> &mut Order {
        &mut self.data[idx as usize]
    }

    #[inline(always)]
    pub fn get(&self, idx: u32) -> &Order {
        &self.data[idx as usize]
    }
}
