use exchange_core::engine::MatchingEngine;
use exchange_core::instrument::{future_instrument_id, spot_instrument_id};
use exchange_core::order::OrderPacket;
use exchange_core::pool::OrderPool;

fn packet(
    order_id: u64,
    account_id: u32,
    instrument_id: u16,
    side: u8,
    price: u32,
    quantity: u32,
    timestamp: u64,
) -> OrderPacket {
    OrderPacket {
        order_id,
        account_id,
        instrument_id,
        side,
        price,
        quantity,
        timestamp,
        _pad: 0,
    }
}

#[test]
fn test_order_packet_size_and_parsing() {
    assert_eq!(std::mem::size_of::<OrderPacket>(), 32);

    let raw = [0u8; 32];
    let packet = OrderPacket::from_bytes(&raw);
    let order_id = packet.order_id;
    let price = packet.price;
    let instrument_id = packet.instrument_id;
    assert_eq!(order_id, 0);
    assert_eq!(price, 0);
    assert_eq!(instrument_id, 0);
}

#[test]
fn test_pool_allocation_and_free_list() {
    let mut pool = OrderPool::new();
    assert_eq!(pool.allocated_count, 0);

    let idx1 = pool.allocate();
    let idx2 = pool.allocate();
    assert_ne!(idx1, idx2);
    assert_eq!(pool.allocated_count, 2);

    pool.deallocate(idx1);
    assert_eq!(pool.allocated_count, 1);

    let idx3 = pool.allocate();
    assert_eq!(idx3, idx1);
    assert_eq!(pool.allocated_count, 2);
}

#[test]
fn test_registry_supports_392_instrument_slots() {
    let engine = MatchingEngine::new();
    assert_eq!(engine.books.len(), 392);
    assert_eq!(engine.instruments.capacity(), 392);
    assert!(engine.instrument(spot_instrument_id(0)).is_some());
    assert!(engine.instrument(future_instrument_id(0)).is_some());
    assert!(engine.instrument(spot_instrument_id(195)).is_none());
}

#[test]
fn test_spot_and_future_have_independent_books() {
    let mut engine = MatchingEngine::new();
    let spot = spot_instrument_id(0);
    let future = future_instrument_id(0);

    let sell_spot = packet(1, 101, spot, 1, 10000, 100, 1000);
    let sell_future = packet(2, 102, future, 1, 10000, 100, 1001);

    let idx = engine.pool.allocate_from_packet(&sell_spot);
    assert_eq!(engine.process_order(idx), 0);
    let idx = engine.pool.allocate_from_packet(&sell_future);
    assert_eq!(engine.process_order(idx), 0);

    assert_eq!(engine.book(spot).unwrap().best_ask(), Some(10000));
    assert_eq!(engine.book(future).unwrap().best_ask(), Some(10000));

    let buy_spot = packet(3, 201, spot, 0, 10000, 50, 1002);
    let idx = engine.pool.allocate_from_packet(&buy_spot);
    assert_eq!(engine.process_order(idx), 1);
    assert_eq!(engine.trades[0].instrument_id, spot);
    assert_eq!(engine.trades[0].seller, 101);

    assert_eq!(
        engine
            .book(spot)
            .unwrap()
            .best_ask_head()
            .map(|i| engine.pool.get(i).remaining),
        Some(50)
    );
    assert_eq!(
        engine
            .book(future)
            .unwrap()
            .best_ask_head()
            .map(|i| engine.pool.get(i).remaining),
        Some(100)
    );
}

#[test]
fn test_different_sovereigns_cannot_cross() {
    let mut engine = MatchingEngine::new();
    let usa = spot_instrument_id(0);
    let germany = spot_instrument_id(1);

    let idx = engine
        .pool
        .allocate_from_packet(&packet(10, 100, usa, 1, 10000, 100, 1));
    engine.process_order(idx);

    let idx = engine
        .pool
        .allocate_from_packet(&packet(11, 200, germany, 0, 10000, 100, 2));
    assert_eq!(engine.process_order(idx), 0);

    assert_eq!(engine.book(usa).unwrap().best_ask(), Some(10000));
    assert_eq!(engine.book(germany).unwrap().best_bid(), Some(10000));
}

#[test]
fn test_cancellation_is_scoped_to_instrument() {
    let mut engine = MatchingEngine::new();
    let usa = spot_instrument_id(0);
    let germany = spot_instrument_id(1);

    let idx = engine
        .pool
        .allocate_from_packet(&packet(100, 1, usa, 0, 9900, 10, 1));
    engine.process_order(idx);
    let idx = engine
        .pool
        .allocate_from_packet(&packet(100, 2, germany, 0, 9800, 20, 2));
    engine.process_order(idx);

    assert!(engine.cancel_order(usa, 100));
    assert!(engine.book(usa).unwrap().bids.is_empty());
    assert_eq!(engine.book(germany).unwrap().best_bid(), Some(9800));

    // The same client/order ID remains alive in the other instrument.
    assert!(engine.cancel_order(germany, 100));
    assert!(engine.book(germany).unwrap().bids.is_empty());
}

#[test]
fn test_basic_limit_matching_and_fifo() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    let idx = engine
        .pool
        .allocate_from_packet(&packet(1, 101, instrument, 1, 10000, 100, 1000));
    assert_eq!(engine.process_order(idx), 0);

    let idx = engine
        .pool
        .allocate_from_packet(&packet(2, 202, instrument, 0, 10000, 50, 1001));
    assert_eq!(engine.process_order(idx), 1);
    assert_eq!(engine.trades[0].qty, 50);
    assert_eq!(engine.trades[0].price, 10000);
    assert_eq!(engine.trades[0].buyer, 202);
    assert_eq!(engine.trades[0].seller, 101);

    let ask_idx = engine.book(instrument).unwrap().best_ask_head().unwrap();
    assert_eq!(engine.pool.get(ask_idx).remaining, 50);
}

#[test]
fn test_strict_fifo_price_time_priority() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(1);

    for &(id, qty, acc, timestamp) in &[(1u64, 10u32, 1u32, 100u64), (2, 20, 2, 101)] {
        let idx = engine
            .pool
            .allocate_from_packet(&packet(id, acc, instrument, 1, 10000, qty, timestamp));
        engine.process_order(idx);
    }

    let idx = engine
        .pool
        .allocate_from_packet(&packet(3, 3, instrument, 0, 10000, 15, 102));
    assert_eq!(engine.process_order(idx), 2);
    assert_eq!(engine.trades[0].seller, 1);
    assert_eq!(engine.trades[0].qty, 10);
    assert_eq!(engine.trades[1].seller, 2);
    assert_eq!(engine.trades[1].qty, 5);

    let remaining = engine.book(instrument).unwrap().best_ask_head().unwrap();
    assert_eq!(engine.pool.get(remaining).account_id, 2);
    assert_eq!(engine.pool.get(remaining).remaining, 15);
}

#[test]
fn test_order_cancellation_and_pool_recovery() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    let idx = engine
        .pool
        .allocate_from_packet(&packet(5001, 42, instrument, 0, 9950, 100, 1000));
    let initial_allocated = engine.pool.allocated_count;
    engine.process_order(idx);

    assert_eq!(engine.book(instrument).unwrap().best_bid(), Some(9950));
    assert!(engine.cancel_order(instrument, 5001));
    assert!(engine.book(instrument).unwrap().bids.is_empty());
    assert_eq!(engine.pool.allocated_count, initial_allocated - 1);
    assert!(!engine.cancel_order(instrument, 5001));
}

#[test]
fn test_middle_cancellation_preserves_fifo() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(2);

    for &(id, qty, acc) in &[(101u64, 10u32, 1001u32), (102, 20, 1002), (103, 30, 1003)] {
        let idx = engine
            .pool
            .allocate_from_packet(&packet(id, acc, instrument, 1, 10000, qty, id));
        engine.process_order(idx);
    }

    assert!(engine.cancel_order(instrument, 102));
    let idx = engine
        .pool
        .allocate_from_packet(&packet(200, 888, instrument, 0, 10000, 15, 5000));
    assert_eq!(engine.process_order(idx), 2);
    assert_eq!(engine.trades[0].seller, 1001);
    assert_eq!(engine.trades[0].qty, 10);
    assert_eq!(engine.trades[1].seller, 1003);
    assert_eq!(engine.trades[1].qty, 5);
}

#[test]
fn test_invalid_instrument_is_rejected_without_mutating_any_book() {
    let mut engine = MatchingEngine::new();
    let invalid = 391u16;
    assert!(engine.instrument(invalid).is_none());

    let idx = engine
        .pool
        .allocate_from_packet(&packet(999, 1, invalid, 1, 10000, 100, 1));
    assert_eq!(engine.process_order(idx), 0);
    assert_eq!(engine.pool.allocated_count, 0);
    assert!(engine
        .books
        .iter()
        .all(|book| book.bids.is_empty() && book.asks.is_empty()));
}

#[test]
fn test_instrument_capacity_boundary() {
    let engine = MatchingEngine::new();
    assert!(engine.book(391).is_some());
    assert!(engine.book(392).is_none());
}

#[test]
fn test_health_check_payload_and_server() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    use exchange_core::health::{generate_health_json, start_health_server, HealthContext};
    use exchange_core::metrics::PerformanceMetrics;
    use exchange_core::ring::RingBuffer;

    let is_running = Arc::new(AtomicBool::new(true));
    let metrics = Arc::new(PerformanceMetrics::new());
    metrics.record_order_latency(1500, 2);
    let ring = RingBuffer::new();

    let context = HealthContext::new(is_running.clone(), metrics.clone(), ring.clone(), 18088);
    let json = generate_health_json(&context);

    assert!(json.contains(r#"\"status\":\"HEALTHY\""#));
    assert!(json.contains(r#"\"engine_running\":true"#));
    assert!(json.contains(r#"\"ring_buffer_depth\":0"#));
    assert!(json.contains(r#"\"orders_processed\":1"#));
    assert!(json.contains(r#"\"trades_generated\":2"#));
    assert!(json.contains(r#"\"avg_latency_micros\":1.5000"#));

    let _handle = start_health_server(context, 18088);
    std::thread::sleep(std::time::Duration::from_millis(100));

    if let Ok(mut stream) = TcpStream::connect("127.0.0.1:18088") {
        let _ = stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");
        let mut resp = [0u8; 1024];
        if let Ok(n) = stream.read(&mut resp) {
            let response = String::from_utf8_lossy(&resp[..n]);
            assert!(response.contains("HTTP/1.1 200 OK"));
            assert!(response.contains(r#"\"status\":\"HEALTHY\""#));
            assert!(response.contains("application/json"));
        }
    }
}
