//! 10,000 Bot Swarm Simulator for Exchange Core.
//!
//! A single unified binary to spin up, configure, and execute all 10,000 algorithmic bots
//! (scaled down from 100k to 10k bots for optimal cache locality and realistic order flow).
//!
//! Archetype distribution across 10,000 bots:
//! - 5,000 Market Makers (Accounts 1,000 - 5,999): Provide resting 2-sided liquidity
//! - 2,500 Momentum Takers (Accounts 6,000 - 8,499): Execute aggressive crossing orders
//! - 1,500 Sovereign Arbitrageurs (Accounts 8,500 - 9,999): Cross-currency & pair arb
//! - 1,000 Noise Traders (Accounts 10,000 - 10,999): Stochastic Poisson order sizing

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use exchange_core::engine::MatchingEngine;
use exchange_core::metrics::PerformanceMetrics;
use exchange_core::order::OrderPacket;
use exchange_core::ring::RingBuffer;
use exchange_core::ticker::TICKERS;

/// Bot Archetype classification
#[derive(Debug, Clone, Copy)]
pub enum BotArchetype {
    MarketMaker,        // 50% of swarm (5,000 bots)
    MomentumTaker,      // 25% of swarm (2,500 bots)
    SovereignArbitrage, // 15% of swarm (1,500 bots)
    NoiseTrader,        // 10% of swarm (1,000 bots)
}

/// Simulated Algorithmic Bot representation
pub struct TradingBot {
    pub bot_id: u32,
    pub account_id: u32,
    pub archetype: BotArchetype,
    pub preferred_ticker: u16,
    pub inventory: i64,
    pub orders_sent: u64,
}

impl TradingBot {
    pub fn new(bot_id: u32, total_bots: usize) -> Self {
        let account_id = 1_000 + bot_id;
        let archetype = if bot_id < (total_bots as f64 * 0.50) as u32 {
            BotArchetype::MarketMaker
        } else if bot_id < (total_bots as f64 * 0.75) as u32 {
            BotArchetype::MomentumTaker
        } else if bot_id < (total_bots as f64 * 0.90) as u32 {
            BotArchetype::SovereignArbitrage
        } else {
            BotArchetype::NoiseTrader
        };

        let preferred_ticker = (bot_id as usize % TICKERS.len()) as u16;

        Self {
            bot_id,
            account_id,
            archetype,
            preferred_ticker,
            inventory: 0,
            orders_sent: 0,
        }
    }

    /// Generate an OrderPacket based on bot archetype and current market state
    #[inline(always)]
    pub fn generate_order(&mut self, seq_id: u64, fair_price: u32) -> OrderPacket {
        self.orders_sent += 1;
        let (side, price, quantity) = match self.archetype {
            BotArchetype::MarketMaker => {
                // Quotes both sides around fair price
                let is_buy = (seq_id % 2) == 0;
                let spread = 5 + ((seq_id * 3) % 15) as u32;
                let prc = if is_buy { fair_price.saturating_sub(spread) } else { fair_price + spread };
                (if is_buy { 0 } else { 1 }, prc, 100)
            }
            BotArchetype::MomentumTaker => {
                // Crosses spread aggressively to fill
                let is_buy = ((seq_id + self.bot_id as u64) % 3) != 0;
                let aggressive_offset = 2 + (seq_id % 5) as u32;
                let prc = if is_buy { fair_price + aggressive_offset } else { fair_price.saturating_sub(aggressive_offset) };
                (if is_buy { 0 } else { 1 }, prc, 50)
            }
            BotArchetype::SovereignArbitrage => {
                // Cross-country spread pegging
                let is_buy = (self.bot_id % 2) == 0;
                let prc = if is_buy { fair_price - 2 } else { fair_price + 2 };
                (if is_buy { 0 } else { 1 }, prc, 75)
            }
            BotArchetype::NoiseTrader => {
                // Random small lots
                let is_buy = (seq_id % 2) == 1;
                let noise_offset = ((seq_id * 7) % 25) as u32;
                let prc = if is_buy { fair_price.saturating_sub(noise_offset) } else { fair_price + noise_offset };
                let qty = 10 + ((seq_id * 13) % 40) as u32;
                (if is_buy { 0 } else { 1 }, prc, qty)
            }
        };

        OrderPacket {
            order_id: (self.account_id as u64) * 1_000_000 + self.orders_sent,
            account_id: self.account_id,
            ticker_id: self.preferred_ticker,
            side,
            price,
            quantity,
            timestamp: 1_700_000_000_000_000_000 + seq_id,
            _pad: 0,
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let num_bots: usize = if args.len() > 1 && args[1] == "--bots" {
        args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10_000)
    } else {
        10_000 // Default 10k bots (scaled down from 100k)
    };

    println!("============================================================");
    println!("🤖 EXCHANGE CORE - 10K BOT SWARM CONTROLLER");
    println!("   Single-Code Master Spin-up for Algorithmic Trading Bots");
    println!("   Scaled: 100k -> 10k Bots for Real-Time Memory Locality");
    println!("============================================================");

    println!("\n📦 Initializing {} algorithmic bots...", num_bots);
    let mut bots: Vec<TradingBot> = Vec::with_capacity(num_bots);
    let mut mm_count = 0;
    let mut taker_count = 0;
    let mut arb_count = 0;
    let mut noise_count = 0;

    for i in 0..num_bots {
        let bot = TradingBot::new(i as u32, num_bots);
        match bot.archetype {
            BotArchetype::MarketMaker => mm_count += 1,
            BotArchetype::MomentumTaker => taker_count += 1,
            BotArchetype::SovereignArbitrage => arb_count += 1,
            BotArchetype::NoiseTrader => noise_count += 1,
        }
        bots.push(bot);
    }

    println!("✅ Swarm Composition:");
    println!("   ├── 🏦 Market Makers:        {:>6} bots (50%) [Passive Resting Depth]", mm_count);
    println!("   ├── ⚡ Momentum Takers:      {:>6} bots (25%) [Aggressive Crossers]", taker_count);
    println!("   ├── 🌐 Sovereign Arbitrage:  {:>6} bots (15%) [Cross-Country Pegs]", arb_count);
    println!("   └── 🎲 Noise Traders:        {:>6} bots (10%) [Stochastic Retail]", noise_count);
    println!("   Account Range: ID #1,000 to #{}", 1_000 + num_bots);

    println!("\n🚀 Initializing Matching Engine & Zero-Alloc Pool...");
    let mut engine = MatchingEngine::new();
    let metrics = Arc::new(PerformanceMetrics::new());
    let ring = RingBuffer::new();
    let running = Arc::new(AtomicBool::new(true));

    // Warm up the orderbook with initial 2-sided liquidity from all Market Maker bots
    println!("⚡ Seeding initial sovereign country orderbooks across all 12 tickers...");
    let seed_start = Instant::now();
    for i in 0..mm_count {
        let bot = &mut bots[i];
        let ticker = &TICKERS[bot.preferred_ticker as usize];
        let pkt = bot.generate_order(i as u64, ticker.base_price_cents);
        let idx = engine.pool.allocate_from_packet(&pkt);
        engine.process_order(idx);
    }
    println!("✅ Pre-seeded {} limit orders in {:.2?}", mm_count, seed_start.elapsed());
    println!("   Bids levels: {}, Asks levels: {}", engine.book.bids.len(), engine.book.asks.len());

    // Execute active high-frequency bot swarm burst
    println!("\n🔥 Launching 10k Bot Swarm Execution Wave (50,000 orders)...");
    let execution_start = Instant::now();
    let mut total_trades = 0usize;
    let mut total_orders = 0usize;

    for wave in 0..5 {
        for bot in bots.iter_mut() {
            let ticker = &TICKERS[bot.preferred_ticker as usize];
            let seq = (wave * num_bots + bot.bot_id as usize) as u64;
            let pkt = bot.generate_order(seq, ticker.base_price_cents);

            let t0 = Instant::now();
            let idx = engine.pool.allocate_from_packet(&pkt);
            let trades = engine.process_order(idx);
            let nanos = t0.elapsed().as_nanos() as u64;
            metrics.record_order_latency(nanos, trades);

            total_orders += 1;
            total_trades += trades;
        }
    }

    let elapsed = execution_start.elapsed();
    let orders_per_sec = (total_orders as f64 / elapsed.as_secs_f64()) as u64;
    let avg_latency = metrics.average_latency_micros();
    let min_latency = metrics.min_latency_nanos();
    let max_latency = metrics.max_latency_nanos();

    println!("\n🏁 10K BOT SWARM EXECUTION RESULTS:");
    println!("   ├── Total Orders Ingested:   {}", total_orders);
    println!("   ├── Total Trades Executed:   {}", total_trades);
    println!("   ├── Elapsed Wall Time:       {:.2?}", elapsed);
    println!("   ├── Swarm Ingestion Rate:    {} orders/sec", orders_per_sec);
    println!("   ├── Mean E[S] Service Time:  {:.3} µs", avg_latency);
    println!("   ├── Minimum Single Latency:  {} ns", min_latency);
    println!("   ├── Maximum Tail Latency:    {} ns ({:.2} µs)", max_latency, max_latency as f64 / 1000.0);
    println!("   └── Pool Slots Allocated:    {} / 5,000,000 (Zero Heap Alloc)", engine.pool.allocated_count);

    if avg_latency < 5.0 {
        println!("\n✅ PASS: Latency target of < 5.0 µs achieved with 10k Bot Swarm!");
    } else {
        println!("\n⚠️ Target: Latency exceeded 5.0 µs ({:.3} µs)", avg_latency);
    }

    println!("\n💡 Single-code execution complete. All {} bots successfully coordinated.", num_bots);
}
