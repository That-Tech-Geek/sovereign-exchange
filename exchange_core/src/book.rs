use std::collections::{BTreeMap, HashMap};

use crate::constants::NULL_ORDER;
use crate::pool::OrderPool;

/// A single price level with strict FIFO order priority.
#[derive(Clone, Debug, Default)]
pub struct PriceLevel {
    pub head: u32,
    pub tail: u32,
    pub volume: u32,
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

/// Central Limit Order Book for exactly one instrument.
///
/// The matcher owns one OrderBook per instrument. There is deliberately no
/// instrument identifier inside the book itself: the containing exchange
/// determines which book is authoritative for a given instrument ID.
pub struct OrderBook {
    pub bids: BTreeMap<u32, PriceLevel>,
    pub asks: BTreeMap<u32, PriceLevel>,
    pub order_map: HashMap<u64, u32>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_map: HashMap::new(),
        }
    }

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

        if level.tail == NULL_ORDER {
            level.head = idx;
        } else {
            pool.data[level.tail as usize].next = idx;
            pool.data[idx as usize].prev = level.tail;
        }
        level.tail = idx;
        level.volume += remaining;
        level.order_count += 1;

        if order_id != 0 {
            self.order_map.insert(order_id, idx);
        }
    }

    #[inline(always)]
    pub fn remove_order(&mut self, idx: u32, pool: &mut OrderPool) {
        let side = pool.data[idx as usize].side;
        let price = pool.data[idx as usize].price;
        let remaining = pool.data[idx as usize].remaining;
        let prev = pool.data[idx as usize].prev;
        let next = pool.data[idx as usize].next;
        let order_id = pool.data[idx as usize].order_id;

        if order_id != 0 {
            self.order_map.remove(&order_id);
        }

        let map = if side == 0 {
            &mut self.bids
        } else {
            &mut self.asks
        };

        if let Some(level) = map.get_mut(&price) {
            if prev != NULL_ORDER {
                pool.data[prev as usize].next = next;
            } else {
                level.head = next;
            }

            if next != NULL_ORDER {
                pool.data[next as usize].prev = prev;
            } else {
                level.tail = prev;
            }

            level.volume = level.volume.saturating_sub(remaining);
            level.order_count = level.order_count.saturating_sub(1);

            if level.head == NULL_ORDER || level.order_count == 0 {
                map.remove(&price);
            }

            pool.deallocate(idx);
        }
    }

    #[inline(always)]
    pub fn remove_order_by_id(&mut self, order_id: u64, pool: &mut OrderPool) -> bool {
        let Some(&idx) = self.order_map.get(&order_id) else {
            return false;
        };
        self.remove_order(idx, pool);
        true
    }

    #[inline(always)]
    pub fn cancel_order(&mut self, order_id: u64, pool: &mut OrderPool) -> bool {
        self.remove_order_by_id(order_id, pool)
    }

    #[inline(always)]
    pub fn best_bid(&self) -> Option<u32> {
        self.bids.keys().next_back().copied()
    }

    #[inline(always)]
    pub fn best_ask(&self) -> Option<u32> {
        self.asks.keys().next().copied()
    }

    #[inline(always)]
    pub fn best_bid_head(&self) -> Option<u32> {
        self.bids.iter().next_back().map(|(_, level)| level.head)
    }

    #[inline(always)]
    pub fn best_ask_head(&self) -> Option<u32> {
        self.asks.iter().next().map(|(_, level)| level.head)
    }

    #[inline(always)]
    pub fn total_levels(&self) -> (usize, usize) {
        (self.bids.len(), self.asks.len())
    }
}
