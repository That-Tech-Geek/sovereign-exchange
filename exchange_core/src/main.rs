#![allow(dead_code, unused_imports)]

mod constants;
mod pool;
mod order;
mod command;
mod book;
mod engine;
mod instrument;
mod sequence;
mod ring;
mod scavenger;
mod ticker;
mod metrics;
mod health;

use core_affinity::set_for_current;
use crossbeam_channel::unbounded;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn main() {
    println!("============================================================");
    println!("🚀 Starting Exchange Core Engine - Sovereign Exchange");
    println!("   Instrument capacity: 196 sovereigns × spot/future = 392");
    println!("   Target: Sub-5µs latency | 200k+ orders/sec | 5M Order Pool");
    println!("============================================================");

    if let Some(core_ids) = core_affinity::get_core_ids() {
        if !core_ids.is_empty() {
            let core = core_ids[constants::CPU_CORE_MATCHING % core_ids.len()];
            if set_for_current(core) { println!("✅ Matching core pinned successfully to Core {:?}", core.id); }
            else { eprintln!("⚠️ Warning: Failed to set CPU affinity (continuing without hard pinning)"); }
        }
    } else { println!("ℹ️ CPU affinity detection unavailable on this environment"); }

    let (trade_tx, trade_rx) = unbounded();
    let is_running = Arc::new(AtomicBool::new(true));
    let metrics = Arc::new(metrics::PerformanceMetrics::new());
    thread::Builder::new().name("scavenger".to_string()).spawn(move || {
        if let Some(core_ids) = core_affinity::get_core_ids() {
            if core_ids.len() > 1 {
                let core = core_ids[constants::CPU_CORE_SCAVENGER % core_ids.len()];
                let _ = set_for_current(core);
                println!("✅ Scavenger thread pinned to Core {:?}", core.id);
            }
        }
        scavenger::run(trade_rx);
    }).expect("Failed to spawn scavenger thread");

    println!("📦 Pre-allocating OrderPool slab (5,000,000 slots)...");
    let init_start = Instant::now();
    let mut engine = engine::MatchingEngine::new();
    let queue = ring::OrderQueue::new();
    println!("✅ OrderPool initialized in {:.2?}", init_start.elapsed());
    println!("✅ Matching engine initialized with {} independent books", engine.books.len());
    println!("✅ Registered instruments: {} / {} capacity", engine.instruments.len(), constants::MAX_INSTRUMENTS);

    let health_ctx = health::HealthContext::new(is_running.clone(), metrics.clone(), queue.clone(), constants::HTTP_PORT);
    health::start_health_server(health_ctx, constants::HTTP_PORT);

    println!("🔥 Running warmup benchmark (10,000 synthetic sovereign orders)...");
    let mut warmup_trades = 0usize;
    let warmup_start = Instant::now();
    for i in 0..10_000 {
        let side = (i % 2) as u8;
        let price = 10000 + ((i * 7) % 50) as u32;
        let packet = order::OrderPacket { client_order_id: (i + 1) as u64, account_id: (100 + (i % 50)) as u32, instrument_id: (i % 24) as u16, side, price, quantity: 100, timestamp: 1_700_000_000_000_000_000 + i as u64, _pad: 0 };
        let t0 = Instant::now();
        let accepted = engine.accept_order(&packet).expect("warmup packet must be accepted");
        let count = engine.process_command(accepted.pool_index);
        metrics.record_order_latency(t0.elapsed().as_nanos() as u64, count);
        warmup_trades += count;
        for t in 0..count { let _ = trade_tx.send(engine.trades[t]); }
    }
    let warmup_duration = warmup_start.elapsed();
    println!("✅ Warmup complete: 10,000 orders matched in {:.2?} (~{:.2} µs/order)", warmup_duration, warmup_duration.as_micros() as f64 / 10_000.0);
    println!("   Trades generated: {}", warmup_trades);
    if let Some(book) = engine.book(0) { println!("   Instrument 0 levels: bids={}, asks={}", book.bids.len(), book.asks.len()); }

    let cancel_test_id = 888_888u64;
    let cancel_packet = order::OrderPacket { client_order_id: cancel_test_id, account_id: 999, instrument_id: 0, side: 0, price: 9000, quantity: 50, timestamp: 123456, _pad: 0 };
    let accepted = engine.accept_order(&cancel_packet).expect("cancellation test order must be accepted");
    engine.process_command(accepted.pool_index);
    let cancel_packet = order::OrderPacket { client_order_id: cancel_test_id, account_id: 999, instrument_id: 0, side: 2, price: 0, quantity: 0, timestamp: 123457, _pad: 0 };
    let cancel_accepted = engine.accept_order(&cancel_packet).expect("cancel command must be accepted");
    engine.process_command(cancel_accepted.pool_index);
    println!("✅ Typed cancellation command verified (Client Order ID: {}, Exchange Order ID: {})", cancel_test_id, accepted.exchange_order_id.0);

    println!("⚡ Matching engine active and listening on order queue...");
    let mut idle_spins = 0u64;
    loop {
        let mut had_work = false;
        while let Some(idx) = queue.try_recv() {
            had_work = true;
            idle_spins = 0;
            let t0 = Instant::now();
            let trade_count = engine.process_command(idx);
            metrics.record_order_latency(t0.elapsed().as_nanos() as u64, trade_count);
            for i in 0..trade_count { let _ = trade_tx.send(engine.trades[i]); }
        }
        if !had_work {
            idle_spins += 1;
            if idle_spins > 10_000 { thread::yield_now(); }
        }
    }
}
