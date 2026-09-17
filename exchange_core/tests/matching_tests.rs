use exchange_core::engine::MatchingEngine;
use exchange_core::order::OrderPacket;
use exchange_core::pool::OrderPool;

#[test]
fn test_order_packet_size_and_parsing() {
    assert_eq!(std::mem::size_of::<OrderPacket>(), 32);

    let raw = [0u8; 32];
    let packet = OrderPacket::from_bytes(&raw);
    let order_id = packet.order_id;
    let price = packet.price;
    assert_eq!(order_id, 0);
    assert_eq!(price, 0);
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
    assert_eq!(idx3, idx1); // Free list LIFO reuse
    assert_eq!(pool.allocated_count, 2);
}

#[test]
fn test_basic_limit_order_matching_and_fifo() {
    let mut engine = MatchingEngine::new();

    // 1. Resting Sell: 100 shares @ $100.00 (price = 10000)
    let sell_packet = OrderPacket {
        order_id: 1,
        account_id: 101,
        ticker_id: 0, // USA
        side: 1,      // Sell
        price: 10000,
        quantity: 100,
        timestamp: 1000,
        _pad: 0,
    };
    let s_idx = engine.pool.allocate_from_packet(&sell_packet);
    let trades = engine.process_order(s_idx);
    assert_eq!(trades, 0);
    assert_eq!(engine.book.asks.len(), 1);
    assert_eq!(engine.book.best_ask(), Some(10000));

    // 2. Incoming Buy: 50 shares @ $100.00 (price = 10000)
    let buy_packet = OrderPacket {
        order_id: 2,
        account_id: 202,
        ticker_id: 0, // USA
        side: 0,      // Buy
        price: 10000,
        quantity: 50,
        timestamp: 1001,
        _pad: 0,
    };
    let b_idx = engine.pool.allocate_from_packet(&buy_packet);
    let trades = engine.process_order(b_idx);
    assert_eq!(trades, 1);
    assert_eq!(engine.trades[0].qty, 50);
    assert_eq!(engine.trades[0].price, 10000);
    assert_eq!(engine.trades[0].buyer, 202);
    assert_eq!(engine.trades[0].seller, 101);

    // Resting sell should now have 50 remaining
    let ask_idx = engine.book.best_ask_head().unwrap();
    assert_eq!(engine.pool.get(ask_idx).remaining, 50);

    // 3. Second Buy: 60 shares @ $100.50 (crosses and finishes resting ask)
    let buy_packet2 = OrderPacket {
        order_id: 3,
        account_id: 303,
        ticker_id: 0,
        side: 0,
        price: 10050,
        quantity: 60,
        timestamp: 1002,
        _pad: 0,
    };
    let b2_idx = engine.pool.allocate_from_packet(&buy_packet2);
    let trades = engine.process_order(b2_idx);
    assert_eq!(trades, 1);
    assert_eq!(engine.trades[0].qty, 50); // Filled the rest of order 1
    assert_eq!(engine.trades[0].seller, 101);

    // The remaining 10 shares of buy_packet2 should now be resting as a bid @ 10050
    assert_eq!(engine.book.asks.len(), 0);
    assert_eq!(engine.book.best_bid(), Some(10050));
    let bid_idx = engine.book.best_bid_head().unwrap();
    assert_eq!(engine.pool.get(bid_idx).remaining, 10);
}

#[test]
fn test_strict_fifo_price_time_priority() {
    let mut engine = MatchingEngine::new();

    // Seller 1 places 10 shares @ 10000 at t=100
    let s1 = OrderPacket {
        order_id: 1,
        account_id: 1,
        ticker_id: 1, // GERMANY
        side: 1,
        price: 10000,
        quantity: 10,
        timestamp: 100,
        _pad: 0,
    };
    let idx1 = engine.pool.allocate_from_packet(&s1);
    engine.process_order(idx1);

    // Seller 2 places 20 shares @ 10000 at t=101
    let s2 = OrderPacket {
        order_id: 2,
        account_id: 2,
        ticker_id: 1,
        side: 1,
        price: 10000,
        quantity: 20,
        timestamp: 101,
        _pad: 0,
    };
    let idx2 = engine.pool.allocate_from_packet(&s2);
    engine.process_order(idx2);

    // Buyer comes in with 15 shares @ 10000
    let b = OrderPacket {
        order_id: 3,
        account_id: 3,
        ticker_id: 1,
        side: 0,
        price: 10000,
        quantity: 15,
        timestamp: 102,
        _pad: 0,
    };
    let b_idx = engine.pool.allocate_from_packet(&b);
    let trade_count = engine.process_order(b_idx);

    // Must generate 2 trades: 10 from Seller 1 (first), then 5 from Seller 2 (second)
    assert_eq!(trade_count, 2);
    assert_eq!(engine.trades[0].seller, 1);
    assert_eq!(engine.trades[0].qty, 10);

    assert_eq!(engine.trades[1].seller, 2);
    assert_eq!(engine.trades[1].qty, 5);

    // Seller 2 should still have 15 remaining in the book
    let remaining_ask = engine.book.best_ask_head().unwrap();
    assert_eq!(engine.pool.get(remaining_ask).account_id, 2);
    assert_eq!(engine.pool.get(remaining_ask).remaining, 15);
}

#[test]
fn test_order_cancellation_direct_and_pool_recovery() {
    let mut engine = MatchingEngine::new();

    // 1. Place resting Limit Buy Order
    let buy = OrderPacket {
        order_id: 5001,
        account_id: 42,
        ticker_id: 0, // USA
        side: 0,      // Buy
        price: 9950,
        quantity: 100,
        timestamp: 1000,
        _pad: 0,
    };
    let idx = engine.pool.allocate_from_packet(&buy);
    let initial_allocated = engine.pool.allocated_count;
    let trades = engine.process_order(idx);
    assert_eq!(trades, 0);

    // Verify order rests in book
    assert_eq!(engine.book.bids.len(), 1);
    assert_eq!(engine.book.best_bid(), Some(9950));
    let level = engine.book.bids.get(&9950).unwrap();
    assert_eq!(level.volume, 100);
    assert_eq!(level.order_count, 1);

    // 2. Cancel the order by order_id
    let cancelled = engine.cancel_order(5001);
    assert!(cancelled, "Cancel order 5001 must return true");

    // 3. Verify book state is cleared and level pruned
    assert_eq!(engine.book.bids.len(), 0);
    assert_eq!(engine.book.best_bid(), None);

    // 4. Verify slot was deallocated back to OrderPool free list
    assert_eq!(engine.pool.allocated_count, initial_allocated - 1);

    // 5. Cancelling the same order again must return false (already removed)
    let cancelled_again = engine.cancel_order(5001);
    assert!(!cancelled_again, "Duplicate cancellation must return false");

    // 6. Cancelling non-existent order must return false
    assert!(!engine.cancel_order(999999));
}

#[test]
fn test_order_cancellation_middle_linked_list_preserves_fifo() {
    let mut engine = MatchingEngine::new();

    // Place 3 resting sells at the exact same price: 10000
    // Order 1: qty 10, Order 2: qty 20, Order 3: qty 30
    let orders = [
        (101u64, 10u32, 1001u32),
        (102u64, 20u32, 1002u32),
        (103u64, 30u32, 1003u32),
    ];

    for &(id, qty, acc) in &orders {
        let pkt = OrderPacket {
            order_id: id,
            account_id: acc,
            ticker_id: 2, // JPN
            side: 1,      // Sell
            price: 10000,
            quantity: qty,
            timestamp: id,
            _pad: 0,
        };
        let idx = engine.pool.allocate_from_packet(&pkt);
        engine.process_order(idx);
    }

    let level_before = engine.book.asks.get(&10000).unwrap();
    assert_eq!(level_before.volume, 60);
    assert_eq!(level_before.order_count, 3);

    // Cancel middle order: 102 (qty 20)
    let cancelled = engine.cancel_order(102);
    assert!(cancelled);

    // Verify level volume is now 40, order count is 2
    let level_after = engine.book.asks.get(&10000).unwrap();
    assert_eq!(level_after.volume, 40);
    assert_eq!(level_after.order_count, 2);

    // Incoming match for qty 15
    // Should fill Order 101 completely (10), and Order 103 partially (5), skipping cancelled 102
    let buy_pkt = OrderPacket {
        order_id: 200,
        account_id: 888,
        ticker_id: 2,
        side: 0,
        price: 10000,
        quantity: 15,
        timestamp: 5000,
        _pad: 0,
    };
    let b_idx = engine.pool.allocate_from_packet(&buy_pkt);
    let trade_count = engine.process_order(b_idx);

    assert_eq!(trade_count, 2);
    assert_eq!(engine.trades[0].seller, 1001); // Order 101
    assert_eq!(engine.trades[0].qty, 10);
    assert_eq!(engine.trades[1].seller, 1003); // Order 103 (bypassed cancelled 102!)
    assert_eq!(engine.trades[1].qty, 5);

    // Remaining in book should be Order 103 with 25 shares
    let rem_level = engine.book.asks.get(&10000).unwrap();
    assert_eq!(rem_level.volume, 25);
    assert_eq!(rem_level.order_count, 1);
}

#[test]
fn test_order_cancellation_packet_protocol() {
    let mut engine = MatchingEngine::new();

    // Resting limit order
    let pkt = OrderPacket {
        order_id: 777,
        account_id: 99,
        ticker_id: 1,
        side: 1, // Sell
        price: 15000,
        quantity: 50,
        timestamp: 100,
        _pad: 0,
    };
    let idx = engine.pool.allocate_from_packet(&pkt);
    engine.process_order(idx);
    assert_eq!(engine.book.asks.len(), 1);

    // Send cancellation via side = 2 packet
    let cancel_pkt = OrderPacket {
        order_id: 777, // target to cancel
        account_id: 99,
        ticker_id: 1,
        side: 2, // Cancel command
        price: 0,
        quantity: 0,
        timestamp: 101,
        _pad: 0,
    };
    let cancel_idx = engine.pool.allocate_from_packet(&cancel_pkt);
    let trades = engine.process_order(cancel_idx);
    assert_eq!(trades, 0);
    assert_eq!(engine.book.asks.len(), 0); // Order 777 successfully removed!
}

#[test]
fn test_health_check_payload_and_server() {
    use exchange_core::health::{generate_health_json, start_health_server, HealthContext};
    use exchange_core::metrics::PerformanceMetrics;
    use exchange_core::ring::RingBuffer;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    let is_running = Arc::new(AtomicBool::new(true));
    let metrics = Arc::new(PerformanceMetrics::new());
    metrics.record_order_latency(1500, 2); // 1.5 µs latency, 2 trades
    let ring = RingBuffer::new();

    let context = HealthContext::new(is_running.clone(), metrics.clone(), ring.clone(), 18088);
    let json = generate_health_json(&context);

    assert!(json.contains(r#""status":"HEALTHY""#));
    assert!(json.contains(r#""engine_running":true"#));
    assert!(json.contains(r#""ring_buffer_depth":0"#));
    assert!(json.contains(r#""orders_processed":1"#));
    assert!(json.contains(r#""trades_generated":2"#));
    assert!(json.contains(r#""avg_latency_micros":1.5000"#));

    // Spawn server on test port 18088
    let _handle = start_health_server(context, 18088);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Connect via TcpStream and query HTTP GET /health
    if let Ok(mut stream) = TcpStream::connect("127.0.0.1:18088") {
        let _ = stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");
        let mut resp = [0u8; 1024];
        if let Ok(n) = stream.read(&mut resp) {
            let response = String::from_utf8_lossy(&resp[..n]);
            assert!(response.contains("HTTP/1.1 200 OK"));
            assert!(response.contains(r#""status":"HEALTHY""#));
            assert!(response.contains("application/json"));
        }
    }
}
