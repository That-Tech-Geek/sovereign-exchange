use std::collections::{BTreeMap, HashMap};

use crate::constants::NULL_ORDER;
use crate::order::{ClientOrderId, OrderKey};
use crate::pool::OrderPool;

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
/// Client identity is unique within this book by `(account_id,
/// client_order_id)`. The exchange-assigned ID is stored on each pool order
/// and is not used for client cancellation lookup.
///
/// FIFO at a price level is canonical exchange sequence order. `insert_limit`
/// therefore appends to the tail: the single-writer matcher must receive
/// accepted commands in ascending `SequenceNumber` order.
pub struct OrderBook {
    pub bids: BTreeMap<u32, PriceLevel>,
    pub asks: BTreeMap<u32, PriceLevel>,
    pub order_map: HashMap<OrderKey, u32>,
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
    pub fn contains_order(&self, account_id: u32, client_order_id: ClientOrderId) -> bool {
        self.order_map.contains_key(&OrderKey {
            account_id,
            client_order_id,
        })
    }

    #[inline(always)]
    pub fn insert_limit(&mut self, idx: u32, pool: &mut OrderPool) {
        let side = pool.data[idx as usize].side;
        let price = pool.data[idx as usize].price;
        let remaining = pool.data[idx as usize].remaining;
        let key = OrderKey {
            account_id: pool.data[idx as usize].account_id,
            client_order_id: ClientOrderId(pool.data[idx as usize].client_order_id),
        };
        let map = if side == 0 { &mut self.bids } else { &mut self.asks };

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
        self.order_map.insert(key, idx);
    }

    #[inline(always)]
    pub fn remove_order(&mut self, idx: u32, pool: &mut OrderPool) {
        let side = pool.data[idx as usize].side;
        let price = pool.data[idx as usize].price;
        let remaining = pool.data[idx as usize].remaining;
        let prev = pool.data[idx as usize].prev;
        let next = pool.data[idx as usize].next;
        let key = OrderKey {
            account_id: pool.data[idx as usize].account_id,
            client_order_id: ClientOrderId(pool.data[idx as usize].client_order_id),
        };

        self.order_map.remove(&key);
        let map = if side == 0 { &mut self.bids } else { &mut self.asks };

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
    pub fn remove_order_by_id(
        &mut self,
        account_id: u32,
        client_order_id: ClientOrderId,
        pool: &mut OrderPool,
    ) -> bool {
        let key = OrderKey {
            account_id,
            client_order_id,
        };
        let Some(idx) = self.order_map.get(&key).copied() else {
            return false;
        };
        self.remove_order(idx, pool);
        true
    }

    #[inline(always)]
    pub fn cancel_order(
        &mut self,
        account_id: u32,
        client_order_id: ClientOrderId,
        pool: &mut OrderPool,
    ) -> bool {
        self.remove_order_by_id(account_id, client_order_id, pool)
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
