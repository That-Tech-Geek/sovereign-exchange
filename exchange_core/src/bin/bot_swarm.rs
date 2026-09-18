//! 10,000 Bot Swarm Simulator for Exchange Core.

use std::env;
use std::sync::Arc;
use std::time::Instant;

use exchange_core::engine::MatchingEngine;
use exchange_core::instrument::spot_instrument_id;
use exchange_core::metrics::PerformanceMetrics;
use exchange_core::order::OrderPacket;
use exchange_core::ring::RingBuffer;
use exchange_core::ticker::TICKERS;

#[derive(Debug, Clone, Copy)]
pub enum BotArchetype {
    MarketMaker,
    MomentumTaker,
    SovereignArbitrage,
    NoiseTrader,
}

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
    #[inline(always)]
    pub fn generate_order(&mut self, seq_id: u64, fair_price: u32) -> OrderPacket {
        self.orders_sent += 1;
        let (side, price, quantity) = match self.archetype {
            BotArchetype::MarketMaker => {
                let is_buy = seq_id % 2 == 0;
                let spread = 5 + ((seq_id * 3) % 15) as u32;
                let prc = if is_buy {
                    fair_price.saturating_sub(spread)
                } else {
                    fair_price + spread
                };
                (if is_buy { 0 } else { 1 }, prc, 100)
            }
            BotArchetype::MomentumTaker => {
                let is_buy = (seq_id + self.bot_id as u64) % 3 != 0;
                let aggressive_offset = 2 + (seq_id % 5) as u32;
                let prc = if is_buy {
                    fair_price + aggressive_offset
                } else {
                    fair_price.saturating_sub(aggressive_offset)
                };
                (if is_buy { 0 } else { 1 }, prc, 50)
            }
            BotArchetype::SovereignArbitrage => {
                let is_buy = self.bot_id % 2 == 0;
                let prc = if is_buy {
                    fair_price - 2
                } else {
                    fair_price + 2
                };
                (if is_buy { 0 } else { 1 }, prc, 75)
            }
            BotArchetype::NoiseTrader => {
                let is_buy = seq_id % 2 == 1;
                let noise_offset = ((seq_id * 7) % 25) as u32;
                let prc = if is_buy {
                    fair_price.saturating_sub(noise_offset)
                } else {
                    fair_price + noise_offset
                };
                let qty = 10 + ((seq_id * 13) % 40) as u32;
                (if is_buy { 0 } else { 1 }, prc, qty)
            }
        };
        OrderPacket {
            client_order_id: self.account_id as u64 * 1_000_000 + self.orders_sent,
            account_id: self.account_id,
            instrument_id: spot_instrument_id(self.preferred_ticker),
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
        10_000
    };
    println!("============================================================");
    println!("🤖 EXCHANGE CORE - 10K BOT SWARM CONTROLLER");
    println!("============================================================");
    println!("\n📦 Initializing {} algorithmic bots...", num_bots);
    let mut bots = Vec::with_capacity(num_bots);
    let (mut mm_count, mut taker_count, mut arb_count, mut noise_count) = (0, 0, 0, 0);
    for i in 0..num_bots {
        let bot = TradingBot::new(i as u32, num_bots);
        match bot.archetype {
            BotArchetype::MarketMaker => mm_count += 1,
            BotArchetype::MomentumTaker => taker_count += 1,
            BotArchetype::SovereignArbitrage => arb_count += 1,
            BotArchetype::NoiseTrader => noise_count += 1,
        }
        bots.push(bot)
    }
    println!(
        "Market Makers: {} | Momentum Takers: {} | Arbitrage: {} | Noise: {}",
        mm_count, taker_count, arb_count, noise_count
    );
    let mut engine = MatchingEngine::new();
    let metrics = Arc::new(PerformanceMetrics::new());
    let _ring = RingBuffer::new();
    println!("\n⚡ Seeding initial sovereign orderbooks...");
    let seed_start = Instant::now();
    for i in 0..mm_count {
        let bot = &mut bots[i];
        let ticker = &TICKERS[bot.preferred_ticker as usize];
        let pkt = bot.generate_order(i as u64, ticker.base_price);
        let accepted = engine
            .accept_order(&pkt)
            .expect("seed order must be accepted");
        engine.process_order(accepted.pool_index);
    }
    println!(
        "✅ Pre-seeded {} limit orders in {:.2?}",
        mm_count,
        seed_start.elapsed()
    );
    println!("\n🔥 Launching 10k Bot Swarm Execution Wave (50,000 orders)...");
    let execution_start = Instant::now();
    let (mut total_trades, mut total_orders) = (0usize, 0usize);
    for wave in 0..5 {
        for bot in bots.iter_mut() {
            let ticker = &TICKERS[bot.preferred_ticker as usize];
            let seq = (wave * num_bots + bot.bot_id as usize) as u64;
            let pkt = bot.generate_order(seq, ticker.base_price);
            let t0 = Instant::now();
            let accepted = engine
                .accept_order(&pkt)
                .expect("swarm order must be accepted");
            let trades = engine.process_order(accepted.pool_index);
            metrics.record_order_latency(t0.elapsed().as_nanos() as u64, trades);
            total_orders += 1;
            total_trades += trades
        }
    }
    let elapsed = execution_start.elapsed();
    let orders_per_sec = (total_orders as f64 / elapsed.as_secs_f64()) as u64;
    println!("\n🏁 10K BOT SWARM EXECUTION RESULTS:");
    println!("Total Orders Ingested: {}", total_orders);
    println!("Total Trades Executed: {}", total_trades);
    println!("Elapsed Wall Time: {:.2?}", elapsed);
    println!("Swarm Ingestion Rate: {} orders/sec", orders_per_sec);
    println!(
        "Mean E[S] Service Time: {:.3} µs",
        metrics.average_latency_micros()
    );
    println!(
        "Pool Slots Allocated: {} / 5,000,000",
        engine.pool.allocated_count
    );
}
