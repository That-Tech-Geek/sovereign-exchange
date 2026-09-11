use crate::pool::OrderPool;
use crate::book::OrderBook;
use crate::constants::MAX_TRADES_PER_MATCH;

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

impl MatchingEngine {
    pub fn new() -> Self {
        Self {
            pool: OrderPool::new(),
            book: OrderBook::new(),
            trades: [Trade::default(); MAX_TRADES_PER_MATCH],
            trade_count: 0,
        }
    }

    /// Process a single order index from the ring buffer.
    /// Returns the number of trades generated (for downstream consumer/scavenger).
    #[inline(always)]
    pub fn process_order(&mut self, idx: u32) -> usize {
        self.trade_count = 0;
        let side = self.pool.data[idx as usize].side;

        // Side 2 represents an explicit order cancellation command in the binary protocol
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

        // If order has remaining quantity, rest it in the CLOB.
        // Otherwise, deallocate it back to the pool immediately.
        if self.pool.data[idx as usize].remaining > 0 {
            self.book.insert_limit(idx, &mut self.pool);
        } else {
            self.pool.deallocate(idx);
        }

        self.trade_count
    }

    /// Cancel and remove an order by its `order_id` from the CLOB.
    /// Locates the order, unlinks it from the price level FIFO list,
    /// decrements level volume and order count, and returns the slot to the OrderPool free list.
    /// 
    /// O(1) operation designed not to block the matching hot path.
    /// Returns `true` if the order was found and cancelled, `false` if not found or already filled.
    #[inline(always)]
    pub fn cancel_order(&mut self, order_id: u64) -> bool {
        self.book.cancel_order(order_id, &mut self.pool)
    }

    /// Alias for `cancel_order` to support remove_order_by_id naming convention.
    #[inline(always)]
    pub fn remove_order_by_id(&mut self, order_id: u64) -> bool {
        self.cancel_order(order_id)
    }

    /// Match a buy order against the ask side of the book.
    #[inline(always)]
    fn match_buy(&mut self, incoming_idx: u32) {
        let incoming_price = self.pool.data[incoming_idx as usize].price;
        
        while self.pool.data[incoming_idx as usize].remaining > 0 {
            // Get the best ask price
            let best_ask = match self.book.best_ask() {
                Some(p) => p,
                None => break, // No asks available
            };
            
            // Check if the buy can cross the spread
            if incoming_price < best_ask {
                break; // Cannot match, price too low
            }
            
            // Get the head order at the best ask level (FIFO priority)
            let ask_idx = match self.book.best_ask_head() {
                Some(idx) => idx,
                None => break,
            };
            
            // Copy data before mutation to respect borrow checker
            let ask_price = self.pool.data[ask_idx as usize].price;
            let ask_remaining = self.pool.data[ask_idx as usize].remaining;
            let incoming_remaining = self.pool.data[incoming_idx as usize].remaining;
            let fill_qty = std::cmp::min(incoming_remaining, ask_remaining);
            
            // Update quantities zero-alloc in place
            self.pool.data[incoming_idx as usize].remaining -= fill_qty;
            self.pool.data[ask_idx as usize].remaining -= fill_qty;

            // Decrement resting price level volume
            if let Some(level) = self.book.asks.get_mut(&ask_price) {
                if level.volume >= fill_qty {
                    level.volume -= fill_qty;
                } else {
                    level.volume = 0;
                }
            }
            
            // Record the trade into the pre-allocated buffer
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
            
            // If the resting ask order is fully filled, remove it from the book and free it
            if self.pool.data[ask_idx as usize].remaining == 0 {
                self.book.remove_order(ask_idx, &mut self.pool);
            }
            
            // Safety bound: if stack buffer is full, prevent overflow
            if self.trade_count >= MAX_TRADES_PER_MATCH {
                break;
            }
        }
    }

    /// Match a sell order against the bid side of the book.
    #[inline(always)]
    fn match_sell(&mut self, incoming_idx: u32) {
        let incoming_price = self.pool.data[incoming_idx as usize].price;
        
        while self.pool.data[incoming_idx as usize].remaining > 0 {
            // Get the best bid price
            let best_bid = match self.book.best_bid() {
                Some(p) => p,
                None => break, // No bids available
            };
            
            // Check if the sell can cross the spread
            if incoming_price > best_bid {
                break; // Cannot match, price too high
            }
            
            // Get the head order at the best bid level (FIFO priority)
            let bid_idx = match self.book.best_bid_head() {
                Some(idx) => idx,
                None => break,
            };
            
            // Copy data before mutation
            let bid_price = self.pool.data[bid_idx as usize].price;
            let bid_remaining = self.pool.data[bid_idx as usize].remaining;
            let incoming_remaining = self.pool.data[incoming_idx as usize].remaining;
            let fill_qty = std::cmp::min(incoming_remaining, bid_remaining);
            
            // Update quantities
            self.pool.data[incoming_idx as usize].remaining -= fill_qty;
            self.pool.data[bid_idx as usize].remaining -= fill_qty;

            // Decrement resting price level volume
            if let Some(level) = self.book.bids.get_mut(&bid_price) {
                if level.volume >= fill_qty {
                    level.volume -= fill_qty;
                } else {
                    level.volume = 0;
                }
            }
            
            // Record the trade into the pre-allocated buffer
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
            
            // If the resting bid order is fully filled, remove it from the book and free it
            if self.pool.data[bid_idx as usize].remaining == 0 {
                self.book.remove_order(bid_idx, &mut self.pool);
            }
            
            // Safety bound: if stack buffer is full, prevent overflow
            if self.trade_count >= MAX_TRADES_PER_MATCH {
                break;
            }
        }
    }
}
