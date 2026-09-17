# Baseline Validation Contract

## Required local checks

Run from `exchange_core/`:

```text
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

The release binary must be produced successfully. Benchmark runs should be recorded separately from correctness tests.

## Required CI checks

Pull requests targeting `main` must execute the Rust build/test workflow. A missing workflow run is a validation failure, not a passing result.

## Correctness invariants

- Instrument IDs are validated before book access.
- Books are isolated by instrument.
- Client and exchange order IDs are separate.
- Active client identity is scoped by account and instrument.
- Canonical sequence numbers are assigned only to accepted commands.
- Client timestamps do not determine FIFO priority.
- Matching is single-writer.
- No trade may cross instruments.
- Expected runtime failures must not silently become undefined state.

## Deferred production gaps

The baseline deliberately records, rather than hides, the following gaps:

- pool exhaustion currently requires later typed-error work,
- ingress queue ownership/backpressure is not yet production-complete,
- event persistence is not yet durable,
- deterministic crash recovery is not yet implemented,
- market lifecycle/risk/margin/settlement are not yet implemented,
- the matching trade buffer still has a fixed 64-trade limit,
- wire protocol endian/version hardening remains future work,
- exchange sequence continuity across process restart requires the journal/recovery design.

These gaps are tracked in the roadmap and are not claims of production readiness.
