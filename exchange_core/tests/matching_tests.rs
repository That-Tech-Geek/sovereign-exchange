use exchange_core::engine::{MatchingEngine, OrderAcceptError};
use exchange_core::instrument::{future_instrument_id, spot_instrument_id};
use exchange_core::order::{ClientOrderId, ExchangeOrderId, OrderPacket};
use exchange_core::pool::OrderPool;

fn packet(
    client_order_id: u64,
    account_id: u32,
    instrument_id: u16,
    side: u8,
    price: u32,
    quantity: u32,
    timestamp: u64,
) -> OrderPacket {
    OrderPacket {
        order_id: client_order_id,
        account_id,
        instrument_id,
        side,
        price,
        quantity,
        timestamp,
        _pad: 0,
    }
}

fn submit(engine: &mut MatchingEngine, packet: &OrderPacket) -> u64 {
    let accepted = engine.accept_order(packet).expect("packet must be accepted");
    engine.process_order(accepted.pool_index);
    accepted.exchange_order_id.0
}

#[test]
fn test_order_packet_size_and_client_identity() {
    assert_eq!(std::mem::size_of::<OrderPacket>(), 32);

    let packet = packet(42, 7, 0, 0, 10000, 10, 123);
    assert_eq!(packet.client_order_id(), ClientOrderId(42));

    let raw = [0u8; 32];
    let decoded = OrderPacket::from_bytes(&raw);
    assert_eq!(decoded.client_order_id(), ClientOrderId(0));
}

#[test]
fn test_pool_stores_distinct_client_and_exchange_ids() {
    let mut pool = OrderPool::new();
    let packet = packet(42, 7, 0, 0, 10000, 10, 123);
    let idx = pool.allocate_from_packet(&packet, ExchangeOrderId(9001));

    assert_eq!(pool.get(idx).client_order_id, 42);
    assert_eq!(pool.get(idx).exchange_order_id, 9001);
    assert_ne!(pool.get(idx).client_order_id, pool.get(idx).exchange_order_id);

    pool.deallocate(idx);
    assert_eq!(pool.allocated_count, 0);
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
fn test_exchange_order_ids_are_monotonic_and_independent_of_client_ids() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    let first = engine
        .accept_order(&packet(9000, 1, instrument, 1, 10000, 10, 1))
        .unwrap();
    engine.process_order(first.pool_index);

    let second = engine
        .accept_order(&packet(3, 2, instrument, 1, 10001, 10, 2))
        .unwrap();
    engine.process_order(second.pool_index);

    assert_eq!(first.exchange_order_id, ExchangeOrderId(1));
    assert_eq!(second.exchange_order_id, ExchangeOrderId(2));
    assert_ne!(first.exchange_order_id.0, 9000);
    assert_ne!(second.exchange_order_id.0, 3);
}

#[test]
fn test_duplicate_client_order_id_is_rejected_while_active() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);
    let first_packet = packet(100, 42, instrument, 1, 10000, 10, 1);

    submit(&mut engine, &first_packet);

    let duplicate = engine.accept_order(&packet(100, 42, instrument, 0, 10000, 5, 2));
    assert_eq!(
        duplicate,
        Err(OrderAcceptError::DuplicateClientOrderId {
            instrument_id: instrument,
            account_id: 42,
            client_order_id: ClientOrderId(100),
        })
    );
}

#[test]
fn test_client_order_id_can_be_reused_after_fill() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    submit(&mut engine, &packet(100, 42, instrument, 1, 10000, 10, 1));
    submit(&mut engine, &packet(200, 77, instrument, 0, 10000, 10, 2));

    let reused = engine.accept_order(&packet(100, 42, instrument, 1, 10001, 5, 3));
    assert!(reused.is_ok());
}

#[test]
fn test_same_client_order_id_is_independent_across_accounts_and_instruments() {
    let mut engine = MatchingEngine::new();
    let usa = spot_instrument_id(0);
    let germany = spot_instrument_id(1);

    assert!(engine.accept_order(&packet(55, 1, usa, 1, 10000, 10, 1)).is_ok());
    assert!(engine.accept_order(&packet(55, 2, usa, 1, 10000, 10, 2)).is_ok());
    assert!(engine.accept_order(&packet(55, 1, germany, 1, 10000, 10, 3)).is_ok());
}

#[test]
fn test_cancellation_requires_matching_account_and_client_identity() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    submit(&mut engine, &packet(500, 42, instrument, 0, 9900, 10, 1));

    assert!(!engine.cancel_order(instrument, 99, ClientOrderId(500)));
    assert_eq!(engine.book(instrument).unwrap().best_bid(), Some(9900));

    assert!(engine.cancel_order(instrument, 42, ClientOrderId(500)));
    assert!(engine.book(instrument).unwrap().bids.is_empty());
}

#[test]
fn test_spot_and_future_have_independent_books() {
    let mut engine = MatchingEngine::new();
    let spot = spot_instrument_id(0);
    let future = future_instrument_id(0);

    submit(&mut engine, &packet(1, 101, spot, 1, 10000, 100, 1000));
    submit(&mut engine, &packet(2, 102, future, 1, 10000, 100, 1001));

    let buy = engine
        .accept_order(&packet(3, 201, spot, 0, 10000, 50, 1002))
        .unwrap();
    assert_eq!(engine.process_order(buy.pool_index), 1);
    assert_eq!(engine.trades[0].instrument_id, spot);
    assert_eq!(engine.trades[0].seller, 101);
    assert_eq!(engine.trades[0].seller_client_order_id, 1);

    assert_eq!(engine.book(spot).unwrap().best_ask_head().map(|i| engine.pool.get(i).remaining), Some(50));
    assert_eq!(engine.book(future).unwrap().best_ask_head().map(|i| engine.pool.get(i).remaining), Some(100));
}

#[test]
fn test_trade_contains_both_identity_namespaces() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    let sell = engine
        .accept_order(&packet(1001, 10, instrument, 1, 10000, 25, 1))
        .unwrap();
    engine.process_order(sell.pool_index);

    let buy = engine
        .accept_order(&packet(2002, 20, instrument, 0, 10000, 25, 2))
        .unwrap();
    assert_eq!(engine.process_order(buy.pool_index), 1);

    let trade = engine.trades[0];
    assert_eq!(trade.buyer, 20);
    assert_eq!(trade.seller, 10);
    assert_eq!(trade.buyer_exchange_order_id, buy.exchange_order_id.0);
    assert_eq!(trade.seller_exchange_order_id, sell.exchange_order_id.0);
    assert_eq!(trade.buyer_client_order_id, 2002);
    assert_eq!(trade.seller_client_order_id, 1001);
}

#[test]
fn test_different_sovereigns_cannot_cross() {
    let mut engine = MatchingEngine::new();
    let usa = spot_instrument_id(0);
    let germany = spot_instrument_id(1);

    submit(&mut engine, &packet(10, 100, usa, 1, 10000, 100, 1));

    let buy = engine
        .accept_order(&packet(11, 200, germany, 0, 10000, 100, 2))
        .unwrap();
    assert_eq!(engine.process_order(buy.pool_index), 0);

    assert_eq!(engine.book(usa).unwrap().best_ask(), Some(10000));
    assert_eq!(engine.book(germany).unwrap().best_bid(), Some(10000));
}

#[test]
fn test_basic_limit_matching_and_fifo() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    submit(&mut engine, &packet(1, 101, instrument, 1, 10000, 100, 1000));

    let buy = engine
        .accept_order(&packet(2, 202, instrument, 0, 10000, 50, 1001))
        .unwrap();
    assert_eq!(engine.process_order(buy.pool_index), 1);
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
        submit(&mut engine, &packet(id, acc, instrument, 1, 10000, qty, timestamp));
    }

    let buy = engine
        .accept_order(&packet(3, 3, instrument, 0, 10000, 15, 102))
        .unwrap();
    assert_eq!(engine.process_order(buy.pool_index), 2);
    assert_eq!(engine.trades[0].seller, 1);
    assert_eq!(engine.trades[0].qty, 10);
    assert_eq!(engine.trades[1].seller, 2);
    assert_eq!(engine.trades[1].qty, 5);
}

#[test]
fn test_order_cancellation_and_pool_recovery() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(0);

    let accepted = engine
        .accept_order(&packet(5001, 42, instrument, 0, 9950, 100, 1000))
        .unwrap();
    let initial_allocated = engine.pool.allocated_count;
    engine.process_order(accepted.pool_index);

    assert_eq!(engine.book(instrument).unwrap().best_bid(), Some(9950));
    assert!(engine.cancel_order(instrument, 42, ClientOrderId(5001)));
    assert!(engine.book(instrument).unwrap().bids.is_empty());
    assert_eq!(engine.pool.allocated_count, initial_allocated - 1);
    assert!(!engine.cancel_order(instrument, 42, ClientOrderId(5001)));
}

#[test]
fn test_middle_cancellation_preserves_fifo() {
    let mut engine = MatchingEngine::new();
    let instrument = spot_instrument_id(2);

    for &(id, qty, acc) in &[(101u64, 10u32, 1001u32), (102, 20, 1002), (103, 30, 1003)] {
        submit(&mut engine, &packet(id, acc, instrument, 1, 10000, qty, id));
    }

    assert!(engine.cancel_order(instrument, 1002, ClientOrderId(102)));
    let buy = engine
        .accept_order(&packet(200, 888, instrument, 0, 10000, 15, 5000))
        .unwrap();
    assert_eq!(engine.process_order(buy.pool_index), 2);
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

    let result = engine.accept_order(&packet(999, 1, invalid, 1, 10000, 100, 1));
    assert_eq!(result, Err(OrderAcceptError::InvalidInstrument(invalid)));
    assert_eq!(engine.pool.allocated_count, 0);
    assert!(engine.books.iter().all(|book| book.bids.is_empty() && book.asks.is_empty()));
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
