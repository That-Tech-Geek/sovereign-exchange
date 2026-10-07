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
fn pool_reaches_declared_capacity_and_rejects_without_advancing_sequence() {
    let mut pool = OrderPool::new();
    let mut allocated = 0usize;
    while pool.allocate().is_ok() {
        allocated += 1;
    }
    assert_eq!(allocated, MAX_ORDERS);
    assert_eq!(pool.allocated_count as usize, MAX_ORDERS);
    assert!(pool.allocate().is_err());
    drop(pool);

    // Pool exhaustion is an ingress rejection, not a canonical sequence event.
    // Free one slot after rejection and verify the next accepted command is
    // still sequence 1.
    let mut engine = MatchingEngine::new();
    while engine.pool.allocate().is_ok() {}
    assert_eq!(engine.pool.allocated_count as usize, MAX_ORDERS);

    let rejected = engine.accept_order(&packet(1, 1, 1, 10_000, 1));
    assert!(matches!(
        rejected,
        Err(exchange_core::OrderAcceptError::OrderPoolExhausted)
    ));

    engine.pool.deallocate(MAX_ORDERS as u32);
    let accepted = engine.accept_order(&packet(2, 2, 1, 10_000, 1)).unwrap();
    assert_eq!(accepted.sequence_number.0, 1);
}

#[test]
fn pool_reuses_a_slot_after_full_capacity() {
    let mut pool = OrderPool::new();
    while pool.allocate().is_ok() {}
    assert_eq!(pool.allocated_count as usize, MAX_ORDERS);

    let freed = MAX_ORDERS as u32;
    pool.deallocate(freed);
    assert_eq!(pool.allocated_count as usize, MAX_ORDERS - 1);

    let reused = pool.allocate().unwrap();
    assert_eq!(reused, freed);
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
        level.volume >= u32::MAX as u64,
        "price-level aggregate wrapped: volume={}",
        level.volume
    );
}

#[test]
fn partial_fill_cancel_and_level_accounting_remain_consistent() {
    let mut engine = MatchingEngine::new();

    let ask = engine.accept_order(&packet(10, 10, 1, 100, 100)).unwrap();
    engine.process_order(ask.pool_index);

    let bid = engine.accept_order(&packet(11, 11, 0, 100, 40)).unwrap();
    assert_eq!(engine.process_order(bid.pool_index), 1);

    let level = engine.book(0).unwrap().asks.get(&100).unwrap();
    assert_eq!(level.volume, 60);
    assert_eq!(level.order_count, 1);

    assert!(engine.cancel_order(0, 10, exchange_core::order::ClientOrderId(10)));
    assert!(engine.book(0).unwrap().asks.get(&100).is_none());
    assert_eq!(engine.pool.allocated_count, 0);
}

#[test]
fn replace_preserves_identity_and_pool_accounting() {
    let mut engine = MatchingEngine::new();

    let original = engine.accept_order(&packet(20, 20, 1, 100, 10)).unwrap();
    engine.process_order(original.pool_index);

    let replacement = engine.accept_order(&packet(21, 20, 1, 90, 15)).unwrap();
    engine.process_order(replacement.pool_index);

    let book = engine.book(0).unwrap();
    assert!(!book.contains_order(20, exchange_core::order::ClientOrderId(20)));
    assert!(book.contains_order(20, exchange_core::order::ClientOrderId(21)));
    assert_eq!(engine.pool.allocated_count, 1);
}

#[test]
fn deterministic_state_fingerprint_survives_replay_order() {
    let mut left = MatchingEngine::new();
    let mut right = MatchingEngine::new();

    let commands = [
        packet(30, 30, 1, 100, 10),
        packet(31, 31, 1, 101, 20),
        packet(32, 32, 0, 100, 5),
    ];

    for command in commands {
        let a = left.accept_order(&command).unwrap();
        left.process_order(a.pool_index);
        let b = right.accept_order(&command).unwrap();
        right.process_order(b.pool_index);
    }

    assert_eq!(left.state_fingerprint(), right.state_fingerprint());
}

#[test]
fn price_levels_are_not_silently_capped_below_declared_constant() {
    // MAX_PRICE_LEVELS is currently a named capacity target, not a hard
    // matching limit. This deliberately crosses it to ensure levels are not
    // silently truncated or corrupted.
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
