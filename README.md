# Sovereign Exchange

A Rust exchange-core research project focused on deterministic matching, durable recovery, replication, and availability-first exchange infrastructure.

> **Status: experimental. Not production trading infrastructure.**

## What exists

- 392 instrument order-book topology
- deterministic price/time matching
- typed client and exchange order identity
- canonical sequencing
- bounded ingress and explicit backpressure
- preallocated order pool
- append-only durable command/event journaling
- deterministic replay and snapshots
- primary/standby replication primitives with fencing and convergence checks
- full-depth market-data primitives with sequence-gap detection
- reproducible release-mode performance and recovery test workflows

## What does not exist yet

This repository does **not** claim a production-grade consensus implementation, exchange network protocol, clearing/settlement system, regulatory controls, or production deployment readiness.

The replication layer currently provides deterministic quorum/log primitives, not a complete Raft implementation or automatic leader election.

## Build

```bash
cd exchange_core
cargo build --release
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Architecture

The intended recovery model is:

```
LIVE MATCHER
    |
    v
DURABLE LOG
    |
    +--> SNAPSHOT @ N
    |
    +--> LOG TAIL N+1...
    |
    v
RECONSTRUCTED MATCHER
```

The core invariant is deterministic state reconstruction from an ordered durable input stream.

## Benchmarks

Benchmark workflows measure throughput and latency on the actual CI runner. Hardware-dependent numbers must be treated as measurements, not guarantees.

## License

Apache-2.0. See [LICENSE](LICENSE).
