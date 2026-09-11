use crate::constants::{MAX_ORDERS, NULL_ORDER};

/// Cache-line aligned to prevent false sharing between CPU cores.
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    pub order_id:   u64,       // Client-generated ID
    pub account_id: u32,       // Trader/bot identifier
    pub ticker_id:  u16,       // Index into Ticker registry
    pub side:       u8,        // 0 = Buy, 1 = Sell
    pub price:      u32,       // Scaled integer (e.g., 10050 = $100.50)
    pub quantity:   u32,       // Original quantity
    pub remaining:  u32,       // Quantity left to fill
    pub next:       u32,       // Index of next order in price level (linked list)
    pub prev:       u32,       // Index of previous order in price level
    pub timestamp:  u64,       // Arrival time (nanoseconds)
}

pub struct OrderPool {
    pub data: Vec<Order>,      // Contiguous memory, index = order ID
    pub free_head: u32,        // Head of free list (linked list of available slots)
    pub allocated_count: u32,  // For debugging & telemetry
}

impl OrderPool {
    /// Creates a new pool with MAX_ORDERS pre-allocated.
    /// O(MAX_ORDERS) - called once at startup.
    pub fn new() -> Self {
        let mut data = Vec::with_capacity(MAX_ORDERS);
        data.resize(MAX_ORDERS, Order::default());
        
        // Slot 0 is reserved as NULL_ORDER sentinel
        // Build free list: 1 -> 2 -> ... -> MAX_ORDERS-1
        for i in 1..MAX_ORDERS - 1 {
            data[i].next = (i + 1) as u32;
        }
        data[MAX_ORDERS - 1].next = u32::MAX; // Terminator
        
        Self { data, free_head: 1, allocated_count: 0 }
    }

    /// Allocate a new order slot. Returns the index.
    /// Panics if out of memory (we over-provisioned).
    #[inline(always)]
    pub fn allocate(&mut self) -> u32 {
        let idx = self.free_head;
        if idx == u32::MAX {
            panic!("OrderPool exhausted: MAX_ORDERS = {}", MAX_ORDERS);
        }
        self.free_head = self.data[idx as usize].next;
        self.data[idx as usize] = Order::default(); // Reset to zeros
        self.allocated_count += 1;
        idx
    }

    /// Return an order to the free list.
    #[inline(always)]
    pub fn deallocate(&mut self, idx: u32) {
        if idx == NULL_ORDER {
            return;
        }
        self.data[idx as usize].next = self.free_head;
        self.free_head = idx;
        self.allocated_count -= 1;
    }

    /// Get mutable reference to an order. Inline for speed.
    #[inline(always)]
    pub fn get_mut(&mut self, idx: u32) -> &mut Order {
        &mut self.data[idx as usize]
    }

    /// Get immutable reference to an order.
    #[inline(always)]
    pub fn get(&self, idx: u32) -> &Order {
        &self.data[idx as usize]
    }
}
