# Contributing

Sovereign Exchange is an experimental exchange-core project. Contributions are welcome.

## Before opening a PR

- Read the architecture and production contract documentation.
- Run `cd exchange_core && cargo fmt -- --check`.
- Run `cd exchange_core && cargo test`.
- Run `cd exchange_core && cargo clippy --all-targets --all-features -- -D warnings`.
- Do not add benchmark claims without reproducible benchmark output.
- Do not weaken deterministic replay, journal integrity, sequencing, or failover invariants.
- Changes to wire formats, journal formats, or public APIs must document compatibility impact.

## Pull requests

Keep one architectural change per PR where practical. Explain the invariant being added or protected and include tests for failure paths, not only the happy path.
