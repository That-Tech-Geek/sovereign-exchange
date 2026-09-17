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

#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    pub exchange_order_id: u64,
    pub client_order_id: u64,
    pub account_id: u32,
    pub instrument_id: u16,
    pub side: u8,
    pub command_kind: u8,
    pub price: u32,
    pub quantity: u32,
    pub remaining: u32,
    pub next: u32,
    pub prev: u32,
    pub sequence_number: u64,
    pub client_timestamp: u64,
    pub replace_target_client_order_id: u64,
}

pub struct OrderPool {
    pub data: Vec<Order>,
    pub free_head: u32,
    pub allocated_count: u32,
}

impl OrderPool {
    pub fn new() -> Self {
        let mut data = Vec::with_capacity(MAX_ORDERS);
        data.resize(MAX_ORDERS, Order::default());
        for i in 1..MAX_ORDERS - 1 {
            data[i].next = (i + 1) as u32;
        }
        data[MAX_ORDERS - 1].next = u32::MAX;
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
                order.instrument_id = order_cmd.instrument_id;
                order.side = order_cmd.side.wire_value();
                order.command_kind = CommandKind::New as u8;
                order.price = order_cmd.price;
                order.quantity = order_cmd.quantity;
                order.remaining = order_cmd.quantity;
                order.sequence_number = sequence_number.0;
                order.client_timestamp = order_cmd.client_timestamp;
            }
            OrderCommand::Cancel(cancel_cmd) => {
                order.client_order_id = cancel_cmd.client_order_id.0;
                order.account_id = cancel_cmd.account_id;
                order.instrument_id = cancel_cmd.instrument_id;
                order.command_kind = CommandKind::Cancel as u8;
                order.sequence_number = sequence_number.0;
            }
            OrderCommand::Replace(replace_cmd) => {
                order.client_order_id = replace_cmd.new_client_order_id.0;
                order.account_id = replace_cmd.account_id;
                order.instrument_id = replace_cmd.instrument_id;
                order.side = replace_cmd.side.wire_value();
                order.command_kind = CommandKind::Replace as u8;
                order.price = replace_cmd.price;
                order.quantity = replace_cmd.quantity;
                order.remaining = replace_cmd.quantity;
                order.sequence_number = sequence_number.0;
                order.client_timestamp = replace_cmd.client_timestamp;
                order.replace_target_client_order_id = replace_cmd.target_client_order_id.0;
            }
        }
        order.exchange_order_id = exchange_order_id.0;
        Ok(idx)
    }

    /// Legacy packet adapter. Production ingress should decode to OrderCommand
    /// before reaching the pool; this helper exists for compatibility tests.
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
