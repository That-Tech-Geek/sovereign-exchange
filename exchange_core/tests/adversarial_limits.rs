use exchange_core::constants::{MAX_ORDERS, MAX_PRICE_LEVELS, RING_BUFFER_SIZE};
use exchange_core::ring::OrderQueue;
use exchange_core::{MatchingEngine, OrderPacket, OrderPool};

fn packet(id: u64, account: u32, side: u8, price: u32, quantity: u32) -> OrderPacket {
    OrderPacket {
        client_order_id: id,
        account_id: account,
        instrument_id: 0,
        side,
        price,
        quantity,
        timestamp: id,
        _pad: 0,
    }
}

#[test]
fn pool_reaches_declared_capacity_without_panicking() {
    // The contract declares MAX_ORDERS usable preallocated slots. Index 0 must not
    // silently reduce that public capacity.
    let mut pool = OrderPool::new();
    let mut allocated = 0usize;
    while pool.allocate().is_ok() {
        allocated += 1;
    }
    assert_eq!(allocated, MAX_ORDERS);
    assert_eq!(pool.allocated_count as usize, MAX_ORDERS);
    assert!(pool.allocate().is_err());
}

#[test]
fn price_level_volume_does_not_wrap_at_u32() {
    let mut engine = MatchingEngine::new();
    let a = engine
        .accept_order(&packet(1, 1, 1, 10_000, u32::MAX))
        .unwrap();
    engine.process_order(a.pool_index);
    let b = engine.accept_order(&packet(2, 2, 1, 10_000, 1)).unwrap();
    engine.process_order(b.pool_index);

    let level = engine.book(0).unwrap().asks.get(&10_000).unwrap();
    assert!(
        level.volume as u64 >= u32::MAX as u64,
        "price-level aggregate wrapped: volume={}",
        level.volume
    );
}

#[test]
fn price_levels_are_not_silently_capped_below_declared_constant() {
    // MAX_PRICE_LEVELS exists as a named capacity constant, but the current
    // OrderBook does not enforce it. This test deliberately crosses it.
    let mut engine = MatchingEngine::new();
    for i in 0..=(MAX_PRICE_LEVELS as u32) {
        let id = i as u64 + 1;
        let accepted = engine
            .accept_order(&packet(id, id as u32, 1, i, 1))
            .unwrap();
        engine.process_order(accepted.pool_index);
    }
    assert!(engine.book(0).unwrap().asks.len() > MAX_PRICE_LEVELS);
}

#[test]
fn ingress_queue_has_exact_bounded_capacity() {
    let queue = OrderQueue::with_capacity(RING_BUFFER_SIZE);
    for i in 1..=RING_BUFFER_SIZE as u32 {
        queue.try_send(i).unwrap();
    }
    assert_eq!(queue.len(), RING_BUFFER_SIZE);
    assert!(queue.try_send(u32::MAX).is_err());
}
