use crate::constants::{MAX_ORDERS, NULL_ORDER};

/// Cache-line aligned to prevent false sharing between CPU cores.
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    pub order_id: u64,       // Exchange/client order ID; identity semantics are refined in PR 2.
    pub account_id: u32,     // Trader/account identifier.
    pub instrument_id: u16,  // Index into the instrument registry.
    pub side: u8,            // 0 = Buy, 1 = Sell, 2 = Cancel command.
    pub price: u32,          // Scaled integer price.
    pub quantity: u32,       // Original quantity.
    pub remaining: u32,      // Quantity left to fill.
    pub next: u32,           // Next order in price-level FIFO linked list.
    pub prev: u32,           // Previous order in price-level FIFO linked list.
    pub timestamp: u64,      // Client timestamp; not used for cross-instrument routing.
}

pub struct OrderPool {
    pub data: Vec<Order>,
    pub free_head: u32,
    pub allocated_count: u32,
}

impl OrderPool {
    /// Creates a pool with MAX_ORDERS pre-allocated slots.
    pub fn new() -> Self {
        let mut data = Vec::with_capacity(MAX_ORDERS);
        data.resize(MAX_ORDERS, Order::default());

        // Slot 0 is reserved as NULL_ORDER sentinel.
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
