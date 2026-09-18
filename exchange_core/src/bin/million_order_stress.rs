use exchange_core::{MatchingEngine, OrderPacket};
use std::time::Instant;

const PAIRS: usize = 1_000_000;
const INSTRUMENT: u16 = 0;
const PRICE: u32 = 10_000;
const QTY: u32 = 1;

fn packet(client_order_id: u64, account_id: u32, side: u8) -> OrderPacket {
    OrderPacket {
        client_order_id,
        account_id,
        instrument_id: INSTRUMENT,
        side,
        price: PRICE,
        quantity: QTY,
        timestamp: 1_700_000_000_000_000_000 + client_order_id,
        _pad: 0,
    }
}

fn main() {
    println!("MILLION-PAIR EXCHANGE STRESS");
    println!(
        "pairs={} total_orders={} instrument={} price={} qty={}",
        PAIRS,
        PAIRS * 2,
        INSTRUMENT,
        PRICE,
        QTY
    );

    let init = Instant::now();
    let mut engine = MatchingEngine::new();
    println!(
        "engine_init_ms={:.3}",
        init.elapsed().as_secs_f64() * 1_000.0
    );

    let admission_start = Instant::now();
    let mut buy_indices = Vec::with_capacity(PAIRS);
    let mut sell_indices = Vec::with_capacity(PAIRS);

    for i in 0..PAIRS {
        let accepted = engine
            .accept_order(&packet(i as u64 + 1, (i % 100_000) as u32 + 1, 0))
            .expect("buy admission failed");
        buy_indices.push(accepted.pool_index);
    }
    for i in 0..PAIRS {
        let accepted = engine
            .accept_order(&packet(
                PAIRS as u64 + i as u64 + 1,
                (i % 100_000) as u32 + 1,
                1,
            ))
            .expect("sell admission failed");
        sell_indices.push(accepted.pool_index);
    }
    let admission_ms = admission_start.elapsed().as_secs_f64() * 1_000.0;

    let clear_start = Instant::now();
    let mut total_trades = 0usize;
    for idx in buy_indices {
        engine.process_command(idx);
        total_trades += engine.trade_count;
    }
    for idx in sell_indices {
        engine.process_command(idx);
        total_trades += engine.trade_count;
    }
    let clear = clear_start.elapsed();

    let total_orders = PAIRS * 2;
    let seconds = clear.as_secs_f64();
    let orders_per_sec = total_orders as f64 / seconds;
    let trades_per_sec = total_trades as f64 / seconds;
    let delivery_rate = total_trades as f64 / PAIRS as f64 * 100.0;

    println!("admission_ms={admission_ms:.3}");
    println!("clear_ms={:.3}", seconds * 1_000.0);
    println!("orders_processed={total_orders}");
    println!("trades_generated={total_trades}");
    println!("order_delivery_rate_pct={delivery_rate:.3}");
    println!("orders_per_sec={orders_per_sec:.3}");
    println!("trades_per_sec={trades_per_sec:.3}");
    println!("remaining_active_orders={}", engine.pool.allocated_count);

    assert_eq!(total_trades, PAIRS, "every sell must match exactly one buy");
    assert_eq!(
        engine.pool.allocated_count, 0,
        "all orders must be fully cleared"
    );
    assert_eq!(delivery_rate, 100.0);
}
