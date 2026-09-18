# Certification and Release Gate

Sovereign Exchange is a reference exchange implementation intended for external engineering evaluation.

## Required release gates

1. `cargo fmt -- --check`
2. `cargo build --release`
3. `cargo test`
4. `cargo clippy --all-targets --all-features -- -D warnings`
5. recovery corruption drills
6. deterministic replay certification
7. ledger atomicity and per-asset conservation
8. consensus term fencing and majority-commit tests
9. protocol framing, checksum, sequence-gap and RBAC tests
10. release-mode performance and stress benchmarks

## State-machine guarantees

The authoritative exchange path is deterministic for the same ordered command stream. Consensus only exposes committed entries to the application state machine. The matching engine, risk state and ledger use integer arithmetic for financial quantities.

## Failure semantics

- A malformed journal or snapshot fails closed.
- A stale consensus term cannot commit.
- A minority partition cannot form a majority.
- A failed ledger batch is atomic and leaves no partial postings.
- A protocol checksum or sequence violation is rejected.

## Scope

This release is a complete reference spot-exchange core with 392 configured instrument slots, matching, risk/account state, double-entry settlement, durable recovery primitives, consensus core, native binary session protocol, market-data primitives and certification workflows.

It is not a claim of regulatory approval, custody licensing, banking connectivity, production key management, or deployment-specific security certification. Those controls remain environment-specific integration work.
