# PR17: Explicit single-writer sequencer and ingress architecture

## Objective
Make canonical sequencing and single-writer ownership explicit in the production data path while preserving deterministic behavior and availability-first operation.

## Scope
- Separate ingress producers from matcher-owned mutable state.
- Introduce an explicit canonical sequencer boundary that assigns exchange sequence exactly once.
- Define producer-visible admission results and backpressure semantics.
- Ensure no producer, journal writer, market-data consumer, or standby path mutates live books.
- Preserve transactional pool ownership: rejected admission releases resources exactly once.
- Keep one deterministic matcher owner for all live CLOB state.
- Add multi-producer ordering and burst-contention tests.
- Add queue saturation and producer-failure tests.
- Add a benchmark harness measuring throughput, p50/p99/p99.9 queueing/processing latency, jitter, drops/rejections, and ordering.
- Treat benchmark output as measurement, not a performance claim.

## Invariants
- Exactly one component assigns canonical exchange sequence.
- Matcher processes commands in canonical sequence.
- A command is either owned/admitted or explicitly rejected, never ambiguously both.
- Backpressure cannot reorder commands.
- Downstream consumers cannot affect matching state.

## Explicitly NOT in this PR
- Hot standby/failover.
- Market-data protocol.
- Risk/accounting.
- Micro-optimisation before benchmark evidence.

## Acceptance
- Multi-producer tests yield one deterministic canonical sequence.
- No duplicate or missing sequences under contention.
- Pool and queue resources remain balanced through rejection paths.
- Benchmark output is retained as CI artifact without arbitrary hardware throughput gates.
- PR16 recovery tests remain green.