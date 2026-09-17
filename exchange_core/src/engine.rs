use crate::book::OrderBook;
use crate::constants::MAX_TRADES_PER_MATCH;
use crate::pool::OrderPool;

/// A trade event emitted by the matching engine.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Trade {
    pub buyer: u32,
    pub seller: u32,
    pub price: u32,
    pub qty: u32,
    pub ticker: u16,
    pub timestamp: u64,
}

pub struct MatchingEngine {
    pub pool: OrderPool,
    pub book: OrderBook,
    pub trades: [Trade; MAX_TRADES_PER_MATCH],
    pub trade_count: usize,
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
            book: OrderBook::new(),
            trades: [Trade::default(); MAX_TRADES_PER_MATCH],
            trade_count: 0,
        }
    }

    #[inline(always)]
    pub fn process_order(&mut self, idx: u32) -> usize {
        self.trade_count = 0;
        let side = self.pool.data[idx as usize].side;

        if side == 2 {
            let target_order_id = self.pool.data[idx as usize].order_id;
            self.cancel_order(target_order_id);
            self.pool.deallocate(idx);
            return 0;
        }

        let is_buy = side == 0;

        if is_buy {
            self.match_buy(idx);
        } else {
            self.match_sell(idx);
        }

        if self.pool.data[idx as usize].remaining > 0 {
            self.book.insert_limit(idx, &mut self.pool);
        } else {
            self.pool.deallocate(idx);
        }

        self.trade_count
    }

    #[inline(always)]
    pub fn cancel_order(&mut self, order_id: u64) -> bool {
        self.book.cancel_order(order_id, &mut self.pool)
    }

    #[inline(always)]
    pub fn remove_order_by_id(&mut self, order_id: u64) -> bool {
        self.cancel_order(order_id)
    }

    #[inline(always)]
    fn match_buy(&mut self, incoming_idx: u32) {
        let incoming_price = self.pool.data[incoming_idx as usize].price;

        while self.pool.data[incoming_idx as usize].remaining > 0 {
            let best_ask = match self.book.best_ask() {
                Some(p) => p,
                None => break,
            };

            if incoming_price < best_ask {
                break;
            }

            let ask_idx = match self.book.best_ask_head() {
                Some(idx) => idx,
                None => break,
            };

            let ask_price = self.pool.data[ask_idx as usize].price;
            let ask_remaining = self.pool.data[ask_idx as usize].remaining;
            let incoming_remaining = self.pool.data[incoming_idx as usize].remaining;
            let fill_qty = std::cmp::min(incoming_remaining, ask_remaining);

            self.pool.data[incoming_idx as usize].remaining -= fill_qty;
            self.pool.data[ask_idx as usize].remaining -= fill_qty;

            if let Some(level) = self.book.asks.get_mut(&ask_price) {
                if level.volume >= fill_qty {
                    level.volume -= fill_qty;
                } else {
                    level.volume = 0;
                }
            }

            self.trades[self.trade_count] = Trade {
                buyer: self.pool.data[incoming_idx as usize].account_id,
                seller: self.pool.data[ask_idx as usize].account_id,
                price: ask_price,
                qty: fill_qty,
                ticker: self.pool.data[incoming_idx as usize].ticker_id,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64,
            };
            self.trade_count += 1;

            if self.pool.data[ask_idx as usize].remaining == 0 {
                self.book.remove_order(ask_idx, &mut self.pool);
            }

            if self.trade_count >= MAX_TRADES_PER_MATCH {
                break;
            }
        }
    }

    #[inline(always)]
    fn match_sell(&mut self, incoming_idx: u32) {
        let incoming_price = self.pool.data[incoming_idx as usize].price;

        while self.pool.data[incoming_idx as usize].remaining > 0 {
            let best_bid = match self.book.best_bid() {
                Some(p) => p,
                None => break,
            };

            if incoming_price > best_bid {
                break;
            }

            let bid_idx = match self.book.best_bid_head() {
                Some(idx) => idx,
                None => break,
            };

            let bid_price = self.pool.data[bid_idx as usize].price;
            let bid_remaining = self.pool.data[bid_idx as usize].remaining;
            let incoming_remaining = self.pool.data[incoming_idx as usize].remaining;
            let fill_qty = std::cmp::min(incoming_remaining, bid_remaining);

            self.pool.data[incoming_idx as usize].remaining -= fill_qty;
            self.pool.data[bid_idx as usize].remaining -= fill_qty;

            if let Some(level) = self.book.bids.get_mut(&bid_price) {
                if level.volume >= fill_qty {
                    level.volume -= fill_qty;
                } else {
                    level.volume = 0;
                }
            }

            self.trades[self.trade_count] = Trade {
                buyer: self.pool.data[bid_idx as usize].account_id,
                seller: self.pool.data[incoming_idx as usize].account_id,
                price: bid_price,
                qty: fill_qty,
                ticker: self.pool.data[incoming_idx as usize].ticker_id,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64,
            };
            self.trade_count += 1;

            if self.pool.data[bid_idx as usize].remaining == 0 {
                self.book.remove_order(bid_idx, &mut self.pool);
            }

            if self.trade_count >= MAX_TRADES_PER_MATCH {
                break;
            }
        }
    }
}
