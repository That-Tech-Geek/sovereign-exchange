# Sovereign Exchange Production Contract

## Purpose

This document freezes the engineering contract for the exchange core before further architectural changes.

The matcher is a deterministic, single-writer central limit order book. Correctness, availability, recovery, and auditability are production requirements. Latency and throughput claims are performance targets, not correctness guarantees.

## Capacity

- 196 sovereigns maximum.
- Spot and future markets per sovereign.
- 392 instrument books maximum under the current instrument model.
- One matching owner for the current engine. Instrument partitioning does not imply 392 matcher threads.
- 5,000,000 preallocated order-pool slots in the current implementation.

If multiple future expiries are introduced later, the instrument capacity contract must be revised explicitly rather than silently exceeding 392.

## Ordering

The exchange, not the client, owns canonical ordering.

- Accepted commands receive exchange-assigned sequence numbers.
- Client timestamps are metadata only.
- FIFO at a price level is determined by canonical exchange admission order.
- Rejected commands must not consume canonical sequence numbers.
- Durable sequence continuity across process restart is a later journal/recovery requirement.

## Identity

Client order identity and exchange order identity are separate namespaces.

Active client-order uniqueness is scoped to account + instrument + client order ID. Clients cannot select exchange order IDs.

## State ownership

The matching engine owns live market state. A future durable event journal will own the historical event stream.

Downstream components must consume exchange events rather than independently reconstructing authoritative matcher state.

## Availability and recovery

A production deployment must eventually support:

1. append-only durable event recording,
2. deterministic replay,
3. consistent snapshots,
4. corruption detection and fail-closed recovery,
5. standby state reconstruction,
6. controlled failover without dual matching.

Until those capabilities are implemented, the engine is a development-stage matcher and must not be represented as crash-recoverable exchange infrastructure.

## Critical-path rules

The following are prohibited from the critical matching path unless a later design explicitly proves they are bounded and deterministic:

- blocking network I/O,
- database writes,
- cloud API calls,
- filesystem synchronization,
- unbounded allocation,
- wall-clock sleeps,
- non-deterministic external state.

## Persistence rule

Firestore, GitHub, analytics stores, and archival systems are not authoritative matching state and must never become a prerequisite for accepting or matching an order.

## Error handling

Production code must prefer typed, observable errors over process panics for expected runtime conditions such as capacity exhaustion, invalid commands, queue saturation, and stale state.

## Performance claims

Benchmark numbers must identify:

- workload and instrument mix,
- hardware and OS,
- build profile,
- warm/cold state,
- latency percentile and sample count,
- whether measurement includes ingress, admission, matching, and egress,
- allocation behavior,
- queueing/backpressure conditions.

A benchmark is evidence for a workload, not a universal guarantee.

## Change discipline

Each architectural PR must:

- address one primary concern,
- preserve existing contracts unless the PR explicitly changes them,
- add regression tests for new semantics,
- avoid speculative infrastructure,
- leave the repository buildable,
- document deferred concerns rather than silently solving them in unrelated code.

## Current roadmap

0. Production contract and baseline freeze
1. Instrument partitioning
2. Client/exchange order identity
3. Canonical exchange sequencing
4. Typed command/event model
5. Remove artificial matching limits
6. Ingress correctness and backpressure
7. Durable event journal
8. Deterministic replay
9. Snapshots and crash recovery
10. Market lifecycle
11. Market data
12. Account/risk foundation
13. Self-match prevention and throttling
14. Trade ledger and fees
15. Position/P&L
16. Futures lifecycle
17. Margin
18. Settlement
19. Operational controls
20. Configuration versioning
21. Standby recovery node
22. Failover
23. Adversarial production suite
24. Performance regression suite
25. Production cleanup
