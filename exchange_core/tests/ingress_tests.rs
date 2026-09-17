use exchange_core::{
    ClientOrderId, IngressError, MatchingEngine, NewOrder, OrderAcceptError, OrderCommand,
    OrderQueue, OrderSide, PoolError, SequenceNumber,
};

fn new_order(id: u64) -> OrderCommand {
    OrderCommand::New(NewOrder {
        client_order_id: ClientOrderId(id),
        account_id: 10,
        instrument_id: 0,
        side: OrderSide::Buy,
        price: 1000,
        quantity: 1,
        client_timestamp: 0,
    })
}

#[test]
fn pool_exhaustion_is_typed_and_does_not_panic() {
    let mut engine = MatchingEngine::new();
    engine.pool.free_head = u32::MAX;

    let result = engine.accept_command(&new_order(1));
    assert_eq!(result, Err(OrderAcceptError::OrderPoolExhausted));
    assert_eq!(engine.pool.allocated_count, 0);

    engine.pool.free_head = 1;
    let accepted = engine.accept_command(&new_order(2)).unwrap();
    assert_eq!(accepted.sequence_number, SequenceNumber::FIRST);
    assert_eq!(accepted.exchange_order_id.0, 1);
}

#[test]
fn pool_allocate_returns_error_instead_of_panicking() {
    let mut pool = exchange_core::OrderPool::new();
    pool.free_head = u32::MAX;
    assert_eq!(pool.allocate(), Err(PoolError::Exhausted));
    assert_eq!(pool.allocated_count, 0);
}

#[test]
fn queue_full_releases_reserved_pool_slot() {
    let mut engine = MatchingEngine::new();
    let queue = OrderQueue::with_capacity(1);

    let first = engine.accept_command(&new_order(1)).unwrap();
    let second = engine.accept_command(&new_order(2)).unwrap();
    assert_eq!(engine.pool.allocated_count, 2);

    assert_eq!(engine.enqueue_order(&queue, first.pool_index), Ok(()));
    assert_eq!(
        engine.enqueue_order(&queue, second.pool_index),
        Err(IngressError::QueueFull)
    );
    assert_eq!(engine.pool.allocated_count, 1);
    assert_eq!(queue.len(), 1);

    let queued_idx = queue.try_recv().unwrap();
    assert_eq!(queued_idx, first.pool_index);
}

#[test]
fn queue_capacity_is_bounded_and_non_blocking() {
    let queue = OrderQueue::with_capacity(2);
    assert!(queue.try_send(10).is_ok());
    assert!(queue.try_send(11).is_ok());
    assert!(queue.try_send(12).is_err());
    assert_eq!(queue.len(), 2);
}
