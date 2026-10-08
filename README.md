# Sovereign Exchange

A Rust-native exchange backend: deterministic matching, durable recovery, risk, settlement, peer replication, and low-overhead market-data infrastructure. The frontend is intentionally a separate repository.

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

The repository is the backend reference implementation: matching, deterministic risk/account state, settlement, durable recovery, consensus, native session protocol, market data, peer-node transport, and certification workflows. The frontend is a separate client and never becomes an exchange authority.

It does not claim regulatory approval, custody or banking integration, deployment-specific key management, or production certification for a particular operating environment. Those controls belong at deployment and jurisdictional integration boundaries.

## Vercel deployment

Vercel is the exchange cockpit/control-plane deployment. It serves the React/Vite interface and a stateless API gateway that forwards requests to the authoritative Rust engine.

The gateway **does not match orders**. It cannot allocate an independent book, assign exchange sequence numbers, or become a second matcher. See [docs/VERCEL_DEPLOYMENT.md](docs/VERCEL_DEPLOYMENT.md).

Set `EXCHANGE_ENGINE_URL` in Vercel to the URL of the authoritative native engine.

## Build

```bash
cd exchange_core
cargo build --release
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Architecture

The intended production recovery model is:

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

## Deployment target

The reference authoritative deployment targets Oracle Cloud Always Free Ampere A1 on a small Arm64 Linux VM. The Vercel project is the public cockpit/control plane; it is not a second matching authority. One authoritative exchange process owns the live books, canonical sequence, and durable command journal.

## Benchmarks

Benchmark workflows measure throughput and latency on the actual CI runner. Hardware-dependent numbers must be treated as measurements, not guarantees.

## License

Apache-2.0. See [LICENSE](LICENSE).

## Oracle Always Free recovery benchmark

`cargo test --release --test oracle_always_free_recovery -- --nocapture` measures durable journal replay time, replay throughput, journal bytes per command, and Linux RSS at increasing journal sizes. The certification budget is replay in under 1 second per benchmark case.
