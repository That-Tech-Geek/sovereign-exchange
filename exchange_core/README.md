# Exchange Core: Ultra-Low-Latency Country-Shares Matching Engine

A production-grade, single-threaded, CPU-pinned Central Limit Order Book (CLOB) matching engine written in Rust for trading sovereign country shares (USA, Germany, Japan, UK, France, etc.) at sub-5 microsecond latency with zero heap allocation on the hot path.

---

## 🏛️ Architecture Overview

### The Golden Rule
**The matching core touches NOTHING external.** No Firestore, no HTTP, no disk I/O, no database queries. The hot path exclusively:
1. Pops pre-allocated order indices from an LMAX-style lock-free ring buffer
2. Matches against the CLOB (PriceLevel doubly linked lists ordered in a BTreeMap)
3. Writes trade events into a pre-allocated stack array
4. Updates slab allocator pointers and linked lists in contiguous memory

All external I/O (Firestore synchronization, WAL compression, periodic book snapshots, and GitHub archival) executes asynchronously on an isolated scavenger thread pinned to Core 1.

```
┌──────────────────────────────────────────────────────────────────┐
│                         ORACLE CLOUD VM                         │
│                         (4 Cores, 24GB RAM)                    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────────┐│
│  │                     CORE 0 (MATCHING)                       ││
│  │  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐   ││
│  │  │ RING    │──▶│ ENGINE  │──▶│ POOL    │──▶│ BOOK    │   ││
│  │  │ BUFFER  │   │ (E[S])  │   │ (Slab)  │   │ (CLOB)  │   ││
│  │  └─────────┘   └────┬────┘   └─────────┘   └─────────┘   ││
│  │                     │ Trades (MPSC)                         ││
│  └─────────────────────┼──────────────────────────────────────┘│
│                        │                                        │
│                        ▼                                        │
│  ┌──────────────────────────────────────────────────────────────┐│
│  │                     CORE 1 (SCAVENGER)                      ││
│  │  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐   ││
│  │  │ BATCH   │──▶│FIRESTORE│   │ WAL     │──▶│ GITHUB  │   ││
│  │  │ AGGREG  │   │ (L2)    │   │ COMPRESS│   │ PUSH    │   ││
│  │  └─────────┘   └─────────┘   └─────────┘   └─────────┘   ││
│  └──────────────────────────────────────────────────────────────┘│
│                                                                  │
│  ┌──────────────────────────────────────────────────────────────┐│
│  │                     CORE 2-3 (INGRESS / BOT EMULATORS)       ││
│  └──────────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────┘
```

---

## ⚡ Performance Targets & Specs

| Metric | Target | Realized Design |
|---|---|---|
| **Service Time ($E[S]$)** | < 5.0 µs | Monotonically inlined pointer operations, O(1) linked-list traversals |
| **Throughput** | 200,000+ orders/sec | Lock-free LMAX disruptor ring buffer with pre-allocated slots |
| **Memory Footprint** | < 320 MB | Compact 64-byte `Order` struct with cache-line alignment (`repr(C, align(64))`) |
| **Zero Hot-Path Allocation** | 0 bytes `malloc` | Custom `OrderPool` slab allocator with free-list recycling |
| **Order Priority** | Strict Price-Time (FIFO) | Doubly linked lists per `PriceLevel` indexed in `BTreeMap` |

---

## 📂 Directory Structure

```
exchange_core/
├── Cargo.toml                  # Compiler optimizations (LTO, panic=abort, codegen-units=1)
├── .github/workflows/build.yml # CI pipeline with build & test steps
├── README.md                   # Technical specification & quickstart
├── tests/
│   └── matching_tests.rs       # Unit & edge-case matching tests
└── src/
    ├── main.rs                 # Entry point, CPU affinity pinning & hot loop
    ├── constants.rs            # Compile-time constants (5M orders, ring buffer size)
    ├── pool.rs                 # OrderPool slab allocator with free list
    ├── order.rs                # 32-byte binary protocol and zero-copy packet parser
    ├── book.rs                 # CLOB price levels and doubly linked lists
    ├── engine.rs               # MatchingEngine (match_buy, match_sell)
    ├── ring.rs                 # Lock-free ring buffer (crossbeam MPMC)
    ├── scavenger.rs            # Core 1 async persistence (Firestore + WAL)
    ├── ticker.rs               # Sovereign country shares registry (USA, GER, JPN, etc.)
    ├── metrics.rs              # Nanosecond telemetry & latency counters
    └── lib.rs                  # Module root & public exports
```

---

## 🛠️ Quickstart Commands

```bash
# Enter exchange core directory
cd exchange_core

# Build release binary with full Link-Time Optimization (LTO)
cargo build --release

# Run unit tests
cargo test

# Run with CPU core pinning (Linux)
cargo run --release

# Inspect binary size (< 10MB stripped)
ls -lh target/release/exchange_core
```

---

## 📦 Binary Order Protocol (32 Bytes)

UDP packets are parsed zero-copy into raw memory:
```
+---------------+---------------+---------------+------+---------------+---------------+---------------+------+
| Order ID (8B) | Account (4B)  | Ticker ID (2B)| Side | Price (4B)    | Qty (4B)      | Timestamp (8B)| Pad  |
| u64           | u32           | u16           | u8   | u32           | u32           | u64           | 1B   |
+---------------+---------------+---------------+------+---------------+---------------+---------------+------+
Total: Exactly 32 bytes (Power of 2 for memory alignment).
```
