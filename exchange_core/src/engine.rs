use crate::book::OrderBook;
use crate::command::{CommandRejectReason, ExchangeEvent, OrderCommand};
use crate::constants::{MAX_INSTRUMENTS, MAX_TRADES_PER_MATCH};
use crate::instrument::{Instrument, InstrumentRegistry, SOVEREIGNS};
use crate::order::{ClientOrderId, ExchangeOrderId, OrderPacket};
use crate::pool::{CommandKind, OrderPool};
use crate::sequence::SequenceNumber;

const MAX_EVENTS_PER_COMMAND: usize = MAX_TRADES_PER_MATCH + 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Trade {
    pub buyer: u32,
    pub seller: u32,
    pub price: u32,
    pub qty: u32,
    pub instrument_id: u16,
    pub buyer_exchange_order_id: u64,
    pub seller_exchange_order_id: u64,
    pub buyer_client_order_id: u64,
    pub seller_client_order_id: u64,
    pub buyer_sequence_number: u64,
    pub seller_sequence_number: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderAcceptError {
    InvalidInstrument(u16),
    InvalidSide(u8),
    DuplicateClientOrderId { instrument_id: u16, account_id: u32, client_order_id: ClientOrderId },
    UnknownOrder { instrument_id: u16, account_id: u32, client_order_id: ClientOrderId },
    ExchangeOrderIdExhausted,
    SequenceNumberExhausted,
}

impl OrderAcceptError {
    pub const fn reason(self) -> CommandRejectReason {
        match self {
            Self::InvalidInstrument(_) => CommandRejectReason::InvalidInstrument,
            Self::InvalidSide(_) => CommandRejectReason::InvalidSide,
            Self::DuplicateClientOrderId { .. } => CommandRejectReason::DuplicateClientOrderId,
            Self::UnknownOrder { .. } => CommandRejectReason::UnknownOrder,
            Self::ExchangeOrderIdExhausted => CommandRejectReason::ExchangeOrderIdExhausted,
            Self::SequenceNumberExhausted => CommandRejectReason::SequenceNumberExhausted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedOrder {
    pub pool_index: u32,
    pub exchange_order_id: ExchangeOrderId,
    pub sequence_number: SequenceNumber,
}

pub struct MatchingEngine {
    pub pool: OrderPool,
    pub books: Vec<OrderBook>,
    pub instruments: InstrumentRegistry,
    pub trades: [Trade; MAX_TRADES_PER_MATCH],
    pub trade_count: usize,
    pub last_events: [Option<ExchangeEvent>; MAX_EVENTS_PER_COMMAND],
    pub event_count: usize,
    next_exchange_order_id: u64,
    next_sequence_number: u64,
}

impl MatchingEngine {
    pub fn new() -> Self {
        Self {
            pool: OrderPool::new(),
            books: (0..MAX_INSTRUMENTS).map(|_| OrderBook::new()).collect(),
            instruments: InstrumentRegistry::from_sovereigns(SOVEREIGNS),
            trades: [Trade::default(); MAX_TRADES_PER_MATCH],
            trade_count: 0,
            last_events: [None; MAX_EVENTS_PER_COMMAND],
            event_count: 0,
            next_exchange_order_id: 1,
            next_sequence_number: SequenceNumber::FIRST.0,
        }
    }

    #[inline(always)]
    pub fn instrument(&self, instrument_id: u16) -> Option<&Instrument> { self.instruments.get(instrument_id) }
    #[inline(always)]
    pub fn book(&self, instrument_id: u16) -> Option<&OrderBook> { self.books.get(instrument_id as usize) }
    #[inline(always)]
    pub fn book_mut(&mut self, instrument_id: u16) -> Option<&mut OrderBook> { self.books.get_mut(instrument_id as usize) }

    /// Legacy packet adapter. Numeric wire command values are decoded once here.
    #[inline]
    pub fn accept_order(&mut self, packet: &OrderPacket) -> Result<AcceptedOrder, OrderAcceptError> {
        let command = OrderCommand::from_packet(packet).map_err(|reason| match reason {
            CommandRejectReason::InvalidSide => OrderAcceptError::InvalidSide(packet.side),
            _ => unreachable!("packet decoder only rejects invalid side"),
        })?;
        self.accept_command(&command)
    }

    /// Admit a typed command. Validation precedes exchange-ID and sequence allocation.
    #[inline]
    pub fn accept_command(&mut self, command: &OrderCommand) -> Result<AcceptedOrder, OrderAcceptError> {
        let (instrument_id, account_id, client_order_id) = match *command {
            OrderCommand::New(order) => (order.instrument_id, order.account_id, order.client_order_id),
            OrderCommand::Cancel(order) => (order.instrument_id, order.account_id, order.client_order_id),
            OrderCommand::Replace(order) => (order.instrument_id, order.account_id, order.new_client_order_id),
        };
        if !self.instruments.contains(instrument_id) { return Err(OrderAcceptError::InvalidInstrument(instrument_id)); }

        let book = &self.books[instrument_id as usize];
        match *command {
            OrderCommand::New(_) if book.contains_order(account_id, client_order_id) => {
                return Err(OrderAcceptError::DuplicateClientOrderId { instrument_id, account_id, client_order_id });
            }
            OrderCommand::Cancel(_) if !book.contains_order(account_id, client_order_id) => {
                return Err(OrderAcceptError::UnknownOrder { instrument_id, account_id, client_order_id });
            }
            OrderCommand::Replace(order) => {
                if !book.contains_order(account_id, order.target_client_order_id) {
                    return Err(OrderAcceptError::UnknownOrder { instrument_id, account_id, client_order_id: order.target_client_order_id });
                }
                if order.new_client_order_id != order.target_client_order_id && book.contains_order(account_id, order.new_client_order_id) {
                    return Err(OrderAcceptError::DuplicateClientOrderId { instrument_id, account_id, client_order_id: order.new_client_order_id });
                }
            }
            _ => {}
        }

        let sequence = self.next_sequence_number;
        let Some(next_sequence) = sequence.checked_add(1) else { return Err(OrderAcceptError::SequenceNumberExhausted); };
        let exchange_order_id = match command {
            OrderCommand::Cancel(_) => ExchangeOrderId(0),
            OrderCommand::New(_) | OrderCommand::Replace(_) => {
                let id = self.next_exchange_order_id;
                let Some(next) = id.checked_add(1) else { return Err(OrderAcceptError::ExchangeOrderIdExhausted); };
                self.next_exchange_order_id = next;
                ExchangeOrderId(id)
            }
        };
        self.next_sequence_number = next_sequence;
        let pool_index = self.pool.allocate_from_command(command, exchange_order_id, SequenceNumber(sequence));
        Ok(AcceptedOrder { pool_index, exchange_order_id, sequence_number: SequenceNumber(sequence) })
    }

    #[inline(always)]
    pub fn process_command(&mut self, idx: u32) -> usize { self.process_order(idx) }

    /// Process an admitted internal command envelope. Command semantics come from
    /// `CommandKind`, not the wire `side` value.
    #[inline(always)]
    pub fn process_order(&mut self, idx: u32) -> usize {
        self.trade_count = 0;
        self.event_count = 0;
        self.last_events.fill(None);
        let instrument_id = self.pool.data[idx as usize].instrument_id;
        let command_kind = self.pool.data[idx as usize].command_kind;
        let sequence_number = SequenceNumber(self.pool.data[idx as usize].sequence_number);
        let account_id = self.pool.data[idx as usize].account_id;
        let client_order_id = ClientOrderId(self.pool.data[idx as usize].client_order_id);
        if !self.instruments.contains(instrument_id) { self.pool.deallocate(idx); return 0; }

        match command_kind {
            x if x == CommandKind::Cancel as u8 => {
                if self.cancel_order(instrument_id, account_id, client_order_id) {
                    self.push_event(ExchangeEvent::OrderCancelled { instrument_id, account_id, client_order_id, sequence_number });
                }
                self.pool.deallocate(idx);
                return 0;
            }
            x if x == CommandKind::Replace as u8 => {
                let target = ClientOrderId(self.pool.data[idx as usize].replace_target_client_order_id);
                let exchange_order_id = ExchangeOrderId(self.pool.data[idx as usize].exchange_order_id);
                if !self.cancel_order(instrument_id, account_id, target) { self.pool.deallocate(idx); return 0; }
                self.pool.data[idx as usize].command_kind = CommandKind::New as u8;
                self.push_event(ExchangeEvent::OrderReplaced { instrument_id, account_id, old_client_order_id: target, new_client_order_id: client_order_id, exchange_order_id, sequence_number });
            }
            x if x == CommandKind::New as u8 => {}
            _ => { self.pool.deallocate(idx); return 0; }
        }

        let side = self.pool.data[idx as usize].side;
        if side > 1 { self.pool.deallocate(idx); return 0; }
        self.push_event(ExchangeEvent::OrderAccepted {
            instrument_id, account_id, client_order_id,
            exchange_order_id: ExchangeOrderId(self.pool.data[idx as usize].exchange_order_id),
            sequence_number,
        });

        let book = &mut self.books[instrument_id as usize];
        let pool = &mut self.pool;
        let trades = &mut self.trades;
        let trade_count = &mut self.trade_count;
        if side == 0 { Self::match_buy(pool, book, trades, trade_count, instrument_id, idx); }
        else { Self::match_sell(pool, book, trades, trade_count, instrument_id, idx); }

        for i in 0..*trade_count {
            let t = trades[i];
            self.push_event(ExchangeEvent::Trade {
                buyer: t.buyer, seller: t.seller, instrument_id: t.instrument_id,
                buyer_exchange_order_id: ExchangeOrderId(t.buyer_exchange_order_id),
                seller_exchange_order_id: ExchangeOrderId(t.seller_exchange_order_id),
                buyer_client_order_id: ClientOrderId(t.buyer_client_order_id),
                seller_client_order_id: ClientOrderId(t.seller_client_order_id),
                buyer_sequence_number: SequenceNumber(t.buyer_sequence_number),
                seller_sequence_number: SequenceNumber(t.seller_sequence_number),
                price: t.price, qty: t.qty, timestamp: t.timestamp,
            });
        }
        if pool.data[idx as usize].remaining > 0 { book.insert_limit(idx, pool); } else { pool.deallocate(idx); }
        *trade_count
    }

    #[inline(always)] fn push_event(&mut self, event: ExchangeEvent) {
        if self.event_count < MAX_EVENTS_PER_COMMAND { self.last_events[self.event_count] = Some(event); self.event_count += 1; }
    }
    #[inline(always)] pub fn events(&self) -> &[Option<ExchangeEvent>] { &self.last_events[..self.event_count] }

    #[inline(always)]
    pub fn cancel_order(&mut self, instrument_id: u16, account_id: u32, client_order_id: ClientOrderId) -> bool {
        if !self.instruments.contains(instrument_id) { return false; }
        self.books[instrument_id as usize].cancel_order(account_id, client_order_id, &mut self.pool)
    }
    #[inline(always)]
    pub fn remove_order_by_id(&mut self, instrument_id: u16, account_id: u32, client_order_id: ClientOrderId) -> bool {
        self.cancel_order(instrument_id, account_id, client_order_id)
    }

    #[inline(always)]
    fn match_buy(pool: &mut OrderPool, book: &mut OrderBook, trades: &mut [Trade; MAX_TRADES_PER_MATCH], trade_count: &mut usize, instrument_id: u16, incoming_idx: u32) {
        let incoming_price = pool.data[incoming_idx as usize].price;
        while pool.data[incoming_idx as usize].remaining > 0 {
            let Some(best_ask) = book.best_ask() else { break };
            if incoming_price < best_ask { break; }
            let Some(ask_idx) = book.best_ask_head() else { break };
            let ask_price = pool.data[ask_idx as usize].price;
            let fill_qty = pool.data[incoming_idx as usize].remaining.min(pool.data[ask_idx as usize].remaining);
            pool.data[incoming_idx as usize].remaining -= fill_qty;
            pool.data[ask_idx as usize].remaining -= fill_qty;
            if let Some(level) = book.asks.get_mut(&ask_price) { level.volume = level.volume.saturating_sub(fill_qty); }
            if *trade_count < MAX_TRADES_PER_MATCH {
                trades[*trade_count] = Trade { buyer: pool.data[incoming_idx as usize].account_id, seller: pool.data[ask_idx as usize].account_id, price: ask_price, qty: fill_qty, instrument_id, buyer_exchange_order_id: pool.data[incoming_idx as usize].exchange_order_id, seller_exchange_order_id: pool.data[ask_idx as usize].exchange_order_id, buyer_client_order_id: pool.data[incoming_idx as usize].client_order_id, seller_client_order_id: pool.data[ask_idx as usize].client_order_id, buyer_sequence_number: pool.data[incoming_idx as usize].sequence_number, seller_sequence_number: pool.data[ask_idx as usize].sequence_number, timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64 };
                *trade_count += 1;
            }
            if pool.data[ask_idx as usize].remaining == 0 { book.remove_order(ask_idx, pool); }
            if *trade_count >= MAX_TRADES_PER_MATCH { break; }
        }
    }

    #[inline(always)]
    fn match_sell(pool: &mut OrderPool, book: &mut OrderBook, trades: &mut [Trade; MAX_TRADES_PER_MATCH], trade_count: &mut usize, instrument_id: u16, incoming_idx: u32) {
        let incoming_price = pool.data[incoming_idx as usize].price;
        while pool.data[incoming_idx as usize].remaining > 0 {
            let Some(best_bid) = book.best_bid() else { break };
            if incoming_price > best_bid { break; }
            let Some(bid_idx) = book.best_bid_head() else { break };
            let bid_price = pool.data[bid_idx as usize].price;
            let fill_qty = pool.data[incoming_idx as usize].remaining.min(pool.data[bid_idx as usize].remaining);
            pool.data[incoming_idx as usize].remaining -= fill_qty;
            pool.data[bid_idx as usize].remaining -= fill_qty;
            if let Some(level) = book.bids.get_mut(&bid_price) { level.volume = level.volume.saturating_sub(fill_qty); }
            if *trade_count < MAX_TRADES_PER_MATCH {
                trades[*trade_count] = Trade { buyer: pool.data[bid_idx as usize].account_id, seller: pool.data[incoming_idx as usize].account_id, price: bid_price, qty: fill_qty, instrument_id, buyer_exchange_order_id: pool.data[bid_idx as usize].exchange_order_id, seller_exchange_order_id: pool.data[incoming_idx as usize].exchange_order_id, buyer_client_order_id: pool.data[bid_idx as usize].client_order_id, seller_client_order_id: pool.data[incoming_idx as usize].client_order_id, buyer_sequence_number: pool.data[bid_idx as usize].sequence_number, seller_sequence_number: pool.data[incoming_idx as usize].sequence_number, timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64 };
                *trade_count += 1;
            }
            if pool.data[bid_idx as usize].remaining == 0 { book.remove_order(bid_idx, pool); }
            if *trade_count >= MAX_TRADES_PER_MATCH { break; }
        }
    }
}
