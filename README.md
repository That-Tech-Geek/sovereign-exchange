# Sovereign Exchange

A Rust exchange-core research project focused on deterministic matching, durable recovery, replication, and availability-first exchange infrastructure.

> **Status: reference exchange. Ready for external engineering evaluation.**

## What exists

- 392 instrument order-book topology
- deterministic price/time matching
- typed client and exchange order identity
- canonical sequencing
- bounded ingress and explicit backpressure
- preallocated order pool
- append-only durable command/event journaling
- deterministic replay and snapshots
- Raft-style consensus core with term fencing and majority commit
- deterministic account/risk controls with integer financial state
- atomic double-entry ledger, fees and settlement
- native binary session protocol with authentication and RBAC
- full-depth market-data primitives with sequence-gap detection
- adversarial certification, release-mode performance and recovery workflows

## Scope and remaining deployment work

The repository is a complete reference spot-exchange core for evaluation: matching, deterministic risk/account state, settlement, durable recovery primitives, consensus core, native session protocol, market data, and certification workflows.

It does not claim regulatory approval, custody or banking integration, deployment-specific key management, or production certification for a particular operating environment. Those controls belong at deployment and jurisdictional integration boundaries.

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
