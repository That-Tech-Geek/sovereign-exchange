use exchange_core::{MatchingEngine, OrderPacket};
use std::time::Instant;

const ORDERS: usize = 500_000;
const WARMUP: usize = 10_000;
const INSTRUMENT: u16 = 0;
const PRICE: u32 = 10_000;
const QTY: u32 = 1;

fn packet(id: u64, account: u32, side: u8, price: u32) -> OrderPacket {
    OrderPacket {
        client_order_id: id,
        account_id: account,
        instrument_id: INSTRUMENT,
        side,
        price,
        quantity: QTY,
        timestamp: 1_700_000_000_000_000_000 + id,
        _pad: 0,
    }
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[rank]
}

fn print_latency(name: &str, samples: &[u64]) {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    println!("{name}_latency_p50_ns={}", percentile(&sorted, 0.50));
    println!("{name}_latency_p95_ns={}", percentile(&sorted, 0.95));
    println!("{name}_latency_p99_ns={}", percentile(&sorted, 0.99));
    println!("{name}_latency_max_ns={}", sorted.last().copied().unwrap_or(0));
}

fn main() {
    println!("SOVEREIGN EXCHANGE PERFORMANCE HARNESS");
    println!("orders_per_phase={ORDERS} warmup={WARMUP}");

    let mut warmup = MatchingEngine::new();
    for i in 0..WARMUP {
        let accepted = warmup
            .accept_order(&packet(i as u64 + 1, 1 + (i % 1000) as u32, 0, PRICE))
            .expect("warmup admission");
        warmup.process_order(accepted.pool_index);
    }

    let mut engine = MatchingEngine::new();
    let mut resting = Vec::with_capacity(ORDERS);
    for i in 0..ORDERS {
        let accepted = engine
            .accept_order(&packet(
                i as u64 + 1,
                1 + (i % 100_000) as u32,
                0,
                PRICE,
            ))
            .expect("resting admission");
        resting.push(accepted.pool_index);
    }

    let start = Instant::now();
    let mut samples = Vec::with_capacity(ORDERS);
    for idx in resting {
        let t0 = Instant::now();
        engine.process_order(idx);
        samples.push(t0.elapsed().as_nanos() as u64);
    }
    let elapsed = start.elapsed();
    let throughput = ORDERS as f64 / elapsed.as_secs_f64();

    println!("resting_orders={ORDERS}");
    println!("resting_elapsed_ms={:.3}", elapsed.as_secs_f64() * 1000.0);
    println!("resting_orders_per_sec={throughput:.3}");
    print_latency("resting", &samples);

    let mut crossing = MatchingEngine::new();
    for i in 0..ORDERS {
        let accepted = crossing
            .accept_order(&packet(
                i as u64 + 1,
                1 + (i % 100_000) as u32,
                0,
                PRICE,
            ))
            .expect("buy admission");
        crossing.process_order(accepted.pool_index);
    }

    let start = Instant::now();
    let mut samples = Vec::with_capacity(ORDERS);
    let mut trades = 0usize;
    for i in 0..ORDERS {
        let accepted = crossing
            .accept_order(&packet(
                ORDERS as u64 + i as u64 + 1,
                200_001 + (i % 100_000) as u32,
                1,
                PRICE,
            ))
            .expect("sell admission");
        let t0 = Instant::now();
        trades += crossing.process_order(accepted.pool_index);
        samples.push(t0.elapsed().as_nanos() as u64);
    }
    let elapsed = start.elapsed();
    let throughput = ORDERS as f64 / elapsed.as_secs_f64();

    println!("crossing_orders={ORDERS}");
    println!("crossing_trades={trades}");
    println!("crossing_elapsed_ms={:.3}", elapsed.as_secs_f64() * 1000.0);
    println!("crossing_orders_per_sec={throughput:.3}");
    println!(
        "crossing_trades_per_sec={:.3}",
        trades as f64 / elapsed.as_secs_f64()
    );
    print_latency("crossing", &samples);

    assert_eq!(trades, ORDERS);
    assert_eq!(crossing.pool.allocated_count, 0);
}
