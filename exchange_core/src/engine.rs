use crate::book::OrderBook;
use crate::command::{CommandRejectReason, ExchangeEvent, OrderCommand};
use crate::command_journal::{CommandJournal, CommandJournalError};
use crate::constants::{INITIAL_TRADE_CAPACITY, MAX_INSTRUMENTS};
use crate::instrument::{Instrument, InstrumentRegistry, SOVEREIGNS};
use crate::order::{ClientOrderId, ExchangeOrderId, OrderPacket};
use crate::pool::{CommandKind, OrderPool, PoolError};
use crate::ring::OrderQueue;
use crate::sequence::SequenceNumber;

const INITIAL_EVENT_CAPACITY: usize = INITIAL_TRADE_CAPACITY + 4;

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
    /// Deterministic logical exchange timestamp, equal to the incoming
    /// command's exchange sequence number. This is not wall-clock time.
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderAcceptError {
    InvalidInstrument(u16),
    InvalidSide(u8),
    DuplicateClientOrderId {
        instrument_id: u16,
        account_id: u32,
        client_order_id: ClientOrderId,
    },
    UnknownOrder {
        instrument_id: u16,
        account_id: u32,
        client_order_id: ClientOrderId,
    },
    ExchangeOrderIdExhausted,
    SequenceNumberExhausted,
    OrderPoolExhausted,
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
            Self::OrderPoolExhausted => CommandRejectReason::OrderPoolExhausted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedOrder {
    pub pool_index: u32,
    pub exchange_order_id: ExchangeOrderId,
    pub sequence_number: SequenceNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngressError {
    QueueFull,
    QueueDisconnected,
}

#[derive(Debug)]
pub enum DurableAcceptError {
    Validation(OrderAcceptError),
    Journal(CommandJournalError),
}
impl From<OrderAcceptError> for DurableAcceptError {
    fn from(e: OrderAcceptError) -> Self {
        Self::Validation(e)
    }
}

#[derive(Debug)]
pub enum DurableRecoveryError {
    Journal(CommandJournalError),
    Accept(OrderAcceptError),
}
impl From<OrderAcceptError> for DurableRecoveryError {
    fn from(e: OrderAcceptError) -> Self {
        Self::Accept(e)
    }
}

impl From<CommandJournalError> for DurableRecoveryError {
    fn from(e: CommandJournalError) -> Self {
        Self::Journal(e)
    }
}

pub struct MatchingEngine {
    pub pool: OrderPool,
    pub books: Vec<OrderBook>,
    pub instruments: InstrumentRegistry,
    pub trades: Vec<Trade>,
    pub trade_count: usize,
    pub last_events: Vec<ExchangeEvent>,
    pub event_count: usize,
    next_exchange_order_id: u64,
    next_sequence_number: u64,
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchingEngine {
    pub fn new() -> Self {
        Self {
            pool: OrderPool::new(),
            books: (0..MAX_INSTRUMENTS).map(|_| OrderBook::new()).collect(),
            instruments: InstrumentRegistry::from_sovereigns(SOVEREIGNS),
            trades: Vec::with_capacity(INITIAL_TRADE_CAPACITY),
            trade_count: 0,
            last_events: Vec::with_capacity(INITIAL_EVENT_CAPACITY),
            event_count: 0,
            next_exchange_order_id: 1,
            next_sequence_number: SequenceNumber::FIRST.0,
        }
    }

    #[inline(always)]
    pub fn instrument(&self, id: u16) -> Option<&Instrument> {
        self.instruments.get(id)
    }

    #[inline(always)]
    pub fn book(&self, id: u16) -> Option<&OrderBook> {
        self.books.get(id as usize)
    }

    #[inline(always)]
    pub fn book_mut(&mut self, id: u16) -> Option<&mut OrderBook> {
        self.books.get_mut(id as usize)
    }

    #[inline]
    pub fn accept_order(
        &mut self,
        packet: &OrderPacket,
    ) -> Result<AcceptedOrder, OrderAcceptError> {
        let command = OrderCommand::from_packet(packet).map_err(|reason| match reason {
            CommandRejectReason::InvalidSide => OrderAcceptError::InvalidSide(packet.side),
            _ => unreachable!("packet decoder only rejects invalid side"),
        })?;
        self.accept_command(&command)
    }

    /// Admit a command atomically with respect to sequence/order identity and
    /// pool capacity. No sequence or exchange ID is consumed if allocation fails.
    #[inline]
    pub fn accept_command(
        &mut self,
        command: &OrderCommand,
    ) -> Result<AcceptedOrder, OrderAcceptError> {
        let (instrument_id, account_id, client_order_id) = match *command {
            OrderCommand::New(o) => (o.instrument_id, o.account_id, o.client_order_id),
            OrderCommand::Cancel(o) => (o.instrument_id, o.account_id, o.client_order_id),
            OrderCommand::Replace(o) => (o.instrument_id, o.account_id, o.new_client_order_id),
        };

        if !self.instruments.contains(instrument_id) {
            return Err(OrderAcceptError::InvalidInstrument(instrument_id));
        }

        let book = &self.books[instrument_id as usize];
        match *command {
            OrderCommand::New(_) if book.contains_order(account_id, client_order_id) => {
                return Err(OrderAcceptError::DuplicateClientOrderId {
                    instrument_id,
                    account_id,
                    client_order_id,
                });
            }
            OrderCommand::Cancel(_) if !book.contains_order(account_id, client_order_id) => {
                return Err(OrderAcceptError::UnknownOrder {
                    instrument_id,
                    account_id,
                    client_order_id,
                });
            }
            OrderCommand::Replace(o) => {
                if !book.contains_order(account_id, o.target_client_order_id) {
                    return Err(OrderAcceptError::UnknownOrder {
                        instrument_id,
                        account_id,
                        client_order_id: o.target_client_order_id,
                    });
                }
                if o.new_client_order_id != o.target_client_order_id
                    && book.contains_order(account_id, o.new_client_order_id)
                {
                    return Err(OrderAcceptError::DuplicateClientOrderId {
                        instrument_id,
                        account_id,
                        client_order_id: o.new_client_order_id,
                    });
                }
            }
            _ => {}
        }

        let sequence = self.next_sequence_number;
        let Some(next_sequence) = sequence.checked_add(1) else {
            return Err(OrderAcceptError::SequenceNumberExhausted);
        };

        let exchange_order_id = match command {
            OrderCommand::Cancel(_) => ExchangeOrderId(0),
            OrderCommand::New(_) | OrderCommand::Replace(_) => {
                let id = self.next_exchange_order_id;
                let Some(_) = id.checked_add(1) else {
                    return Err(OrderAcceptError::ExchangeOrderIdExhausted);
                };
                ExchangeOrderId(id)
            }
        };

        let pool_index = self
            .pool
            .allocate_from_command(command, exchange_order_id, SequenceNumber(sequence))
            .map_err(|PoolError::Exhausted| OrderAcceptError::OrderPoolExhausted)?;

        self.next_sequence_number = next_sequence;
        if !matches!(command, OrderCommand::Cancel(_)) {
            self.next_exchange_order_id = exchange_order_id.0 + 1;
        }

        Ok(AcceptedOrder {
            pool_index,
            exchange_order_id,
            sequence_number: SequenceNumber(sequence),
        })
    }

    /// Enqueue an accepted order for the single matching consumer.
    ///
    /// Queue-full and queue-disconnected cases return ownership of the pool
    /// slot to this engine immediately. Callers cannot accidentally leak a
    /// reserved order slot when ingress is saturated.
    #[inline]
    pub fn enqueue_order(
        &mut self,
        queue: &OrderQueue,
        pool_index: u32,
    ) -> Result<(), IngressError> {
        match queue.try_send(pool_index) {
            Ok(()) => Ok(()),
            Err(crossbeam_channel::TrySendError::Full(idx)) => {
                self.pool.deallocate(idx);
                Err(IngressError::QueueFull)
            }
            Err(crossbeam_channel::TrySendError::Disconnected(idx)) => {
                self.pool.deallocate(idx);
                Err(IngressError::QueueDisconnected)
            }
        }
    }

    pub fn accept_durable(
        &mut self,
        command: &OrderCommand,
        journal: &mut CommandJournal,
    ) -> Result<AcceptedOrder, DurableAcceptError> {
        let accepted = self.accept_command(command)?;
        if let Err(error) = journal.append(command) {
            self.pool.deallocate(accepted.pool_index);
            self.next_sequence_number = accepted.sequence_number.0;
            if accepted.exchange_order_id.0 != 0 {
                self.next_exchange_order_id = accepted.exchange_order_id.0;
            }
            return Err(DurableAcceptError::Journal(error));
        }
        Ok(accepted)
    }

    pub fn recover_from_command_journal(
        &mut self,
        journal: &mut CommandJournal,
    ) -> Result<usize, DurableRecoveryError> {
        let commands = journal.read_all()?;
        for command in &commands {
            let accepted = self.accept_command(command)?;
            self.process_order(accepted.pool_index);
        }
        Ok(commands.len())
    }

    #[inline(always)]
    pub fn process_command(&mut self, idx: u32) -> usize {
        self.process_order(idx)
    }

    #[inline(always)]
    pub fn process_order(&mut self, idx: u32) -> usize {
        self.trades.clear();
        self.trade_count = 0;
        self.last_events.clear();
        self.event_count = 0;

        let instrument_id = self.pool.data[idx as usize].instrument_id;
        let command_kind = self.pool.data[idx as usize].command_kind;
        let sequence_number = SequenceNumber(self.pool.data[idx as usize].sequence_number);
        let account_id = self.pool.data[idx as usize].account_id;
        let client_order_id = ClientOrderId(self.pool.data[idx as usize].client_order_id);

        if !self.instruments.contains(instrument_id) {
            self.pool.deallocate(idx);
            return 0;
        }

        match command_kind {
            x if x == CommandKind::Cancel as u8 => {
                if self.cancel_order(instrument_id, account_id, client_order_id) {
                    self.push_event(ExchangeEvent::OrderCancelled {
                        instrument_id,
                        account_id,
                        client_order_id,
                        sequence_number,
                    });
                }
                self.pool.deallocate(idx);
                return 0;
            }
            x if x == CommandKind::Replace as u8 => {
                let target =
                    ClientOrderId(self.pool.data[idx as usize].replace_target_client_order_id);
                let exchange_order_id =
                    ExchangeOrderId(self.pool.data[idx as usize].exchange_order_id);
                if !self.cancel_order(instrument_id, account_id, target) {
                    self.pool.deallocate(idx);
                    return 0;
                }
                self.pool.data[idx as usize].command_kind = CommandKind::New as u8;
                self.push_event(ExchangeEvent::OrderReplaced {
                    instrument_id,
                    account_id,
                    old_client_order_id: target,
                    new_client_order_id: client_order_id,
                    exchange_order_id,
                    sequence_number,
                });
            }
            x if x == CommandKind::New as u8 => {}
            _ => {
                self.pool.deallocate(idx);
                return 0;
            }
        }

        let side = self.pool.data[idx as usize].side;
        if side > 1 {
            self.pool.deallocate(idx);
            return 0;
        }

        self.push_event(ExchangeEvent::OrderAccepted {
            instrument_id,
            account_id,
            client_order_id,
            exchange_order_id: ExchangeOrderId(self.pool.data[idx as usize].exchange_order_id),
            sequence_number,
        });

        {
            let book = &mut self.books[instrument_id as usize];
            let pool = &mut self.pool;
            let trades = &mut self.trades;
            if side == 0 {
                Self::match_buy(pool, book, trades, instrument_id, idx);
            } else {
                Self::match_sell(pool, book, trades, instrument_id, idx);
            }
            if pool.data[idx as usize].remaining > 0 {
                book.insert_limit(idx, pool);
            } else {
                pool.deallocate(idx);
            }
        }

        for i in 0..self.trades.len() {
            let trade = self.trades[i];
            self.push_event(ExchangeEvent::Trade {
                buyer: trade.buyer,
                seller: trade.seller,
                instrument_id: trade.instrument_id,
                buyer_exchange_order_id: ExchangeOrderId(trade.buyer_exchange_order_id),
                seller_exchange_order_id: ExchangeOrderId(trade.seller_exchange_order_id),
                buyer_client_order_id: ClientOrderId(trade.buyer_client_order_id),
                seller_client_order_id: ClientOrderId(trade.seller_client_order_id),
                buyer_sequence_number: SequenceNumber(trade.buyer_sequence_number),
                seller_sequence_number: SequenceNumber(trade.seller_sequence_number),
                price: trade.price,
                qty: trade.qty,
                timestamp: trade.timestamp,
            });
        }

        self.trade_count = self.trades.len();
        self.trade_count
    }

    #[inline(always)]
    fn push_event(&mut self, event: ExchangeEvent) {
        self.last_events.push(event);
        self.event_count = self.last_events.len();
    }

    #[inline(always)]
    pub fn events(&self) -> &[ExchangeEvent] {
        &self.last_events
    }

    /// Deterministic digest of live matcher state for replica convergence checks.
    /// Hash-map iteration order is normalized by sorting client identity keys first.
    pub fn state_fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        let mut mix = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(0x100000001b3);
        };

        mix(self.next_exchange_order_id);
        mix(self.next_sequence_number);
        mix(self.pool.allocated_count as u64);

        for (instrument_id, book) in self.books.iter().enumerate() {
            let mut orders = Vec::with_capacity(book.order_map.len());
            for (key, &idx) in &book.order_map {
                let order = &self.pool.data[idx as usize];
                orders.push((
                    key.account_id,
                    key.client_order_id.0,
                    order.exchange_order_id,
                    order.side,
                    order.price,
                    order.quantity,
                    order.remaining,
                    order.sequence_number,
                    order.client_timestamp,
                ));
            }
            orders.sort_unstable();
            mix(instrument_id as u64);
            mix(orders.len() as u64);
            for (
                account_id,
                client_order_id,
                exchange_order_id,
                side,
                price,
                quantity,
                remaining,
                sequence_number,
                client_timestamp,
            ) in orders
            {
                mix(account_id as u64);
                mix(client_order_id);
                mix(exchange_order_id);
                mix(side as u64);
                mix(price as u64);
                mix(quantity as u64);
                mix(remaining as u64);
                mix(sequence_number);
                mix(client_timestamp);
            }
        }
        hash
    }

    #[inline(always)]
    pub fn cancel_order(
        &mut self,
        instrument_id: u16,
        account_id: u32,
        client_order_id: ClientOrderId,
    ) -> bool {
        if !self.instruments.contains(instrument_id) {
            return false;
        }
        self.books[instrument_id as usize].cancel_order(account_id, client_order_id, &mut self.pool)
    }

    #[inline(always)]
    pub fn remove_order_by_id(
        &mut self,
        instrument_id: u16,
        account_id: u32,
        client_order_id: ClientOrderId,
    ) -> bool {
        self.cancel_order(instrument_id, account_id, client_order_id)
    }

    #[inline(always)]
    fn match_buy(
        pool: &mut OrderPool,
        book: &mut OrderBook,
        trades: &mut Vec<Trade>,
        instrument_id: u16,
        incoming_idx: u32,
    ) {
        let incoming_price = pool.data[incoming_idx as usize].price;

        while pool.data[incoming_idx as usize].remaining > 0 {
            let Some(best_ask) = book.best_ask() else {
                break;
            };
            if incoming_price < best_ask {
                break;
            }
            let Some(ask_idx) = book.best_ask_head() else {
                break;
            };
            let ask_price = pool.data[ask_idx as usize].price;
            let fill_qty = pool.data[incoming_idx as usize]
                .remaining
                .min(pool.data[ask_idx as usize].remaining);

            pool.data[incoming_idx as usize].remaining -= fill_qty;
            pool.data[ask_idx as usize].remaining -= fill_qty;

            if let Some(level) = book.asks.get_mut(&ask_price) {
                level.volume = level.volume.saturating_sub(fill_qty);
            }

            trades.push(Trade {
                buyer: pool.data[incoming_idx as usize].account_id,
                seller: pool.data[ask_idx as usize].account_id,
                price: ask_price,
                qty: fill_qty,
                instrument_id,
                buyer_exchange_order_id: pool.data[incoming_idx as usize].exchange_order_id,
                seller_exchange_order_id: pool.data[ask_idx as usize].exchange_order_id,
                buyer_client_order_id: pool.data[incoming_idx as usize].client_order_id,
                seller_client_order_id: pool.data[ask_idx as usize].client_order_id,
                buyer_sequence_number: pool.data[incoming_idx as usize].sequence_number,
                seller_sequence_number: pool.data[ask_idx as usize].sequence_number,
                // Logical exchange time is derived from the deterministic
                // command sequence. Never consult wall-clock time in the
                // matching path.
                timestamp: pool.data[incoming_idx as usize].sequence_number,
            });

            if pool.data[ask_idx as usize].remaining == 0 {
                book.remove_order(ask_idx, pool);
            }
        }
    }

    #[inline(always)]
    fn match_sell(
        pool: &mut OrderPool,
        book: &mut OrderBook,
        trades: &mut Vec<Trade>,
        instrument_id: u16,
        incoming_idx: u32,
    ) {
        let incoming_price = pool.data[incoming_idx as usize].price;

        while pool.data[incoming_idx as usize].remaining > 0 {
            let Some(best_bid) = book.best_bid() else {
                break;
            };
            if incoming_price > best_bid {
                break;
            }
            let Some(bid_idx) = book.best_bid_head() else {
                break;
            };
            let bid_price = pool.data[bid_idx as usize].price;
            let fill_qty = pool.data[incoming_idx as usize]
                .remaining
                .min(pool.data[bid_idx as usize].remaining);

            pool.data[incoming_idx as usize].remaining -= fill_qty;
            pool.data[bid_idx as usize].remaining -= fill_qty;

            if let Some(level) = book.bids.get_mut(&bid_price) {
                level.volume = level.volume.saturating_sub(fill_qty);
            }

            trades.push(Trade {
                buyer: pool.data[bid_idx as usize].account_id,
                seller: pool.data[incoming_idx as usize].account_id,
                price: bid_price,
                qty: fill_qty,
                instrument_id,
                buyer_exchange_order_id: pool.data[bid_idx as usize].exchange_order_id,
                seller_exchange_order_id: pool.data[incoming_idx as usize].exchange_order_id,
                buyer_client_order_id: pool.data[bid_idx as usize].client_order_id,
                seller_client_order_id: pool.data[incoming_idx as usize].client_order_id,
                buyer_sequence_number: pool.data[bid_idx as usize].sequence_number,
                seller_sequence_number: pool.data[incoming_idx as usize].sequence_number,
                // Logical exchange time is derived from the deterministic
                // command sequence. Never consult wall-clock time in the
                // matching path.
                timestamp: pool.data[incoming_idx as usize].sequence_number,
            });

            if pool.data[bid_idx as usize].remaining == 0 {
                book.remove_order(bid_idx, pool);
            }
        }
    }
}
