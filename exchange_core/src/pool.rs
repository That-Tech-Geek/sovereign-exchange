use crate::constants::{MAX_ORDERS, NULL_ORDER};

/// Cache-line aligned to prevent false sharing between CPU cores.
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    pub order_id: u64,
    pub account_id: u32,
    pub ticker_id: u16,
    pub side: u8,
    pub price: u32,
    pub quantity: u32,
    pub remaining: u32,
    pub next: u32,
    pub prev: u32,
    pub timestamp: u64,
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
        let mut data = Vec::with_capacity(MAX_ORDERS);
        data.resize(MAX_ORDERS, Order::default());

        for (i, order) in data
            .iter_mut()
            .enumerate()
            .skip(1)
            .take(MAX_ORDERS.saturating_sub(2))
        {
            order.next = (i + 1) as u32;
        }
        data[MAX_ORDERS - 1].next = u32::MAX;

        Self {
            data,
            free_head: 1,
            allocated_count: 0,
        }
    }

    #[inline(always)]
    pub fn allocate(&mut self) -> u32 {
        let idx = self.free_head;
        if idx == u32::MAX {
            panic!("OrderPool exhausted: MAX_ORDERS = {}", MAX_ORDERS);
        }
        self.free_head = self.data[idx as usize].next;
        self.data[idx as usize] = Order::default();
        self.allocated_count += 1;
        idx
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
