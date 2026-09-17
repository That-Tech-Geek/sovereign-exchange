use crate::constants::NULL_ORDER;
use crate::pool::OrderPool;
use std::collections::{BTreeMap, HashMap};

/// Represents a single price level with a linked list of orders for strict FIFO priority.
#[derive(Clone, Debug, Default)]
pub struct PriceLevel {
    pub head: u32,   // Index of first order in the level (oldest)
    pub tail: u32,   // Index of last order in the level (newest)
    pub volume: u32, // Total quantity at this price
    pub order_count: u32,
}

impl PriceLevel {
    pub fn new() -> Self {
        Self {
            head: NULL_ORDER,
            tail: NULL_ORDER,
            volume: 0,
            order_count: 0,
        }
    }
}

/// Order book for a single ticker (Central Limit Order Book).
pub struct OrderBook {
    pub bids: BTreeMap<u32, PriceLevel>, // Sorted ascending (highest bid is last)
    pub asks: BTreeMap<u32, PriceLevel>, // Sorted ascending (lowest ask is first)
    pub order_map: HashMap<u64, u32>,    // O(1) order_id -> pool slot index mapping
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_map: HashMap::new(),
        }
    }

    /// Insert a limit order into the book.
    /// O(log N) for BTreeMap lookup + O(1) linked list append + O(1) index map.
    #[inline(always)]
    pub fn insert_limit(&mut self, idx: u32, pool: &mut OrderPool) {
        let side = pool.data[idx as usize].side;
        let price = pool.data[idx as usize].price;
        let remaining = pool.data[idx as usize].remaining;
        let order_id = pool.data[idx as usize].order_id;
        let map = if side == 0 {
            &mut self.bids
        } else {
            &mut self.asks
        };

        let level = map.entry(price).or_insert_with(PriceLevel::new);

        // Append to tail of linked list (FIFO priority)
        if level.tail == NULL_ORDER {
            level.head = idx;
        } else {
            pool.data[level.tail as usize].next = idx;
            pool.data[idx as usize].prev = level.tail;
        }
        level.tail = idx;
        level.volume += remaining;
        level.order_count += 1;

        // Register in fast lookup table for O(1) cancellations
        if order_id != 0 {
            self.order_map.insert(order_id, idx);
        }
    }

    /// Remove an order from the book (cancellation or full fill).
    /// O(log N) for BTreeMap lookup + O(1) linked list removal + O(1) pool deallocation.
    #[inline(always)]
    pub fn remove_order(&mut self, idx: u32, pool: &mut OrderPool) {
        let side = pool.data[idx as usize].side;
        let price = pool.data[idx as usize].price;
        let remaining = pool.data[idx as usize].remaining;
        let prev = pool.data[idx as usize].prev;
        let next = pool.data[idx as usize].next;
        let order_id = pool.data[idx as usize].order_id;

        // Clean up lookup map
        if order_id != 0 {
            self.order_map.remove(&order_id);
        }

        let map = if side == 0 {
            &mut self.bids
        } else {
            &mut self.asks
        };

        if let Some(level) = map.get_mut(&price) {
            // Unlink from linked list
            if prev != NULL_ORDER {
                pool.data[prev as usize].next = next;
            } else {
                level.head = next; // Removing head
            }

            if next != NULL_ORDER {
                pool.data[next as usize].prev = prev;
            } else {
                level.tail = prev; // Removing tail
            }

            if level.volume >= remaining {
                level.volume -= remaining;
            } else {
                level.volume = 0;
            }

            if level.order_count > 0 {
                level.order_count -= 1;
            }

            let should_remove = level.head == NULL_ORDER || level.volume == 0;
            if should_remove {
                map.remove(&price);
            }

            // Return order to free list
            pool.deallocate(idx);
        }
    }

    /// Remove an order by its client-provided `order_id`.
    ///
    /// Efficient non-blocking cancellation process:
    /// 1. Direct O(1) index lookup via order_map.
    /// 2. If map missed (fallback), scans active bids/asks linked lists.
    /// 3. Safely unlinks from the price level's linked list.
    /// 4. Updates price level's volume and order count (prunes level if empty).
    /// 5. Deallocates the order slot back to the OrderPool free list.
    ///
    /// Returns `true` if the order was found and removed, `false` otherwise.
    #[inline(always)]
    pub fn remove_order_by_id(&mut self, order_id: u64, pool: &mut OrderPool) -> bool {
        // Step 1: Fast O(1) lookup
        if let Some(&idx) = self.order_map.get(&order_id) {
            self.remove_order(idx, pool);
            return true;
        }

        // Step 2: Fallback search across bids price levels
        for level in self.bids.values() {
            let mut curr = level.head;
            while curr != NULL_ORDER {
                if pool.data[curr as usize].order_id == order_id {
                    self.remove_order(curr, pool);
                    return true;
                }
                curr = pool.data[curr as usize].next;
            }
        }

        // Step 3: Fallback search across asks price levels
        for level in self.asks.values() {
            let mut curr = level.head;
            while curr != NULL_ORDER {
                if pool.data[curr as usize].order_id == order_id {
                    self.remove_order(curr, pool);
                    return true;
                }
                curr = pool.data[curr as usize].next;
            }
        }

        false
    }

    /// Alias for `remove_order_by_id`.
    #[inline(always)]
    pub fn cancel_order(&mut self, order_id: u64, pool: &mut OrderPool) -> bool {
        self.remove_order_by_id(order_id, pool)
    }

    /// Get the best bid price (highest bid).
    #[inline(always)]
    pub fn best_bid(&self) -> Option<u32> {
        self.bids.keys().next_back().copied()
    }

    /// Get the best ask price (lowest ask).
    #[inline(always)]
    pub fn best_ask(&self) -> Option<u32> {
        self.asks.keys().next().copied()
    }

    /// Get the head order at the best bid level.
    #[inline(always)]
    pub fn best_bid_head(&self) -> Option<u32> {
        self.bids.iter().next_back().map(|(_, level)| level.head)
    }

    /// Get the head order at the best ask level.
    #[inline(always)]
    pub fn best_ask_head(&self) -> Option<u32> {
        self.asks.iter().next().map(|(_, level)| level.head)
    }

    /// Total bids count and asks count
    #[inline(always)]
    pub fn total_levels(&self) -> (usize, usize) {
        (self.bids.len(), self.asks.len())
    }
}
