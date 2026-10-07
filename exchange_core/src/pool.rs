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
#[repr(C, packed(1))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    exchange_order_id: [u8; 5],
    pub client_order_id: u64,
    sequence_number: [u8; 7],
    pub account_id: u32,
    pub price: u32,
    pub remaining: u32,
    links: [u8; 6],
    meta: [u8; 2],
}

const _: () = assert!(std::mem::size_of::<Order>() == 40);
pub const MAX_PACKED_EXCHANGE_ORDER_ID: u64 = (1u64 << 40) - 1;
pub const MAX_PACKED_SEQUENCE_NUMBER: u64 = (1u64 << 56) - 1;
const EXCHANGE_ID_MASK: u64 = MAX_PACKED_EXCHANGE_ORDER_ID;
const SEQUENCE_MASK: u64 = MAX_PACKED_SEQUENCE_NUMBER;

const LINK_MASK: u32 = (1 << 23) - 1;
const FREE_SENTINEL: u32 = LINK_MASK;
const INSTRUMENT_MASK: u32 = 0x01FF;
const SIDE_SHIFT: u32 = 9;
const KIND_SHIFT: u32 = 11;
const SIDE_MASK: u32 = 0x3;
const KIND_MASK: u32 = 0x3;

impl Order {
    #[inline(always)]
    pub fn exchange_order_id(&self) -> u64 {
        let mut b = [0u8; 8];
        b[..5].copy_from_slice(&self.exchange_order_id);
        u64::from_le_bytes(b)
    }

    #[inline(always)]
    fn set_exchange_order_id(&mut self, value: u64) {
        assert!(value <= EXCHANGE_ID_MASK);
        self.exchange_order_id
            .copy_from_slice(&value.to_le_bytes()[..5]);
    }

    #[inline(always)]
    pub fn sequence_number(&self) -> u64 {
        let mut b = [0u8; 8];
        b[..7].copy_from_slice(&self.sequence_number);
        u64::from_le_bytes(b)
    }

    #[inline(always)]
    fn set_sequence_number(&mut self, value: u64) {
        assert!(value <= SEQUENCE_MASK);
        self.sequence_number.copy_from_slice(&value.to_le_bytes()[..7]);
    }

    #[inline(always)]
    pub fn client_order_id(&self) -> u64 {
        self.client_order_id
    }

    #[inline(always)]
    pub fn remaining(&self) -> u32 {
        self.remaining
    }

    #[inline(always)]
    fn links_u64(&self) -> u64 {
        let mut b = [0u8; 8];
        b[..6].copy_from_slice(&self.links);
        u64::from_le_bytes(b)
    }

    #[inline(always)]
    fn set_links_u64(&mut self, value: u64) {
        self.links.copy_from_slice(&value.to_le_bytes()[..6]);
    }

    #[inline(always)]
    pub fn next(&self) -> u32 {
        (self.links_u64() as u32) & LINK_MASK
    }

    #[inline(always)]
    pub fn prev(&self) -> u32 {
        ((self.links_u64() >> 23) as u32) & LINK_MASK
    }

    #[inline(always)]
    pub fn set_next(&mut self, value: u32) {
        assert!(value <= LINK_MASK);
        let prev = self.prev();
        self.set_links_u64(value as u64 | ((prev as u64) << 23));
    }

    #[inline(always)]
    pub fn set_prev(&mut self, value: u32) {
        assert!(value <= LINK_MASK);
        let next = self.next();
        self.set_links_u64(next as u64 | ((value as u64) << 23));
    }

    #[inline(always)]
    fn meta_u32(&self) -> u32 {
        u16::from_le_bytes([self.meta[0], self.meta[1]]) as u32
    }

    #[inline(always)]
    fn set_meta_u32(&mut self, value: u32) {
        let b = (value as u16).to_le_bytes();
        self.meta.copy_from_slice(&b);
    }

    #[inline(always)]
    pub fn instrument_id(&self) -> u16 {
        (self.meta_u32() & INSTRUMENT_MASK) as u16
    }

    #[inline(always)]
    pub fn side(&self) -> u8 {
        ((self.meta_u32() >> SIDE_SHIFT) & SIDE_MASK) as u8
    }

    #[inline(always)]
    pub fn command_kind(&self) -> u8 {
        ((self.meta_u32() >> KIND_SHIFT) & KIND_MASK) as u8
    }

    #[inline(always)]
    pub fn set_meta(&mut self, instrument_id: u16, side: u8, command_kind: CommandKind) {
        assert!((instrument_id as u32) <= INSTRUMENT_MASK);
        self.set_meta_u32(
            instrument_id as u32
                | ((side as u32 & SIDE_MASK) << SIDE_SHIFT)
                | (((command_kind as u32) & KIND_MASK) << KIND_SHIFT),
        );
    }

    #[inline(always)]
    pub fn set_command_kind_new(&mut self) {
        let mut meta = self.meta_u32();
        meta = (meta & !(KIND_MASK << KIND_SHIFT)) | ((CommandKind::New as u32) << KIND_SHIFT);
        self.set_meta_u32(meta);
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
            order.set_next((i + 1) as u32);
        }
        data[MAX_ORDERS].set_next(FREE_SENTINEL);
        Self {
            data,
            free_head: 1,
            allocated_count: 0,
        }
    }

    #[inline(always)]
    pub fn allocate(&mut self) -> Result<u32, PoolError> {
        let idx = self.free_head;
        if idx == FREE_SENTINEL {
            return Err(PoolError::Exhausted);
        }
        self.free_head = self.data[idx as usize].next();
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
                order.set_sequence_number(sequence_number.0);
            }
            OrderCommand::Cancel(cancel_cmd) => {
                order.client_order_id = cancel_cmd.client_order_id.0;
                order.account_id = cancel_cmd.account_id;
                order.set_meta(cancel_cmd.instrument_id, 0, CommandKind::Cancel);
                order.set_sequence_number(sequence_number.0);
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
                order.set_sequence_number(sequence_number.0);
                order.set_prev(replace_cmd.target_client_order_id.0 as u32);
            }
        }
        order.set_exchange_order_id(exchange_order_id.0);
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
        self.data[idx as usize].set_next(self.free_head);
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
