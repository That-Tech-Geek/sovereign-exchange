# PR20: Failure-injection and recovery SLO suite

## Objective
Turn availability from an architectural goal into continuously measured evidence.

## Failure matrix
- Matcher crash at deterministic command boundaries.
- Restart during journal append/batch durability.
- Journal truncation and CRC corruption.
- Snapshot interruption before commit and snapshot corruption.
- Queue saturation and producer failure.
- Pool exhaustion and admission rollback.
- Standby lag and replication interruption.
- Stale-primary command injection after fencing.
- Disk-full / journal write failure simulation.
- Repeated failover/recovery cycles.

## Scope
- Add deterministic fault-injection points, not timing-based flaky sleeps.
- Run controlled command streams with known expected canonical events and final state.
- Assert no phantom trades, duplicate trades, sequence regression, or illegal book mutation.
- Assert every durable acknowledgement survives recovery.
- Measure RTO, journal tail lag, standby lag, recovery success rate, and rejected/ambiguous commands.
- Preserve failing seeds and failure artifacts for reproduction.
- Publish machine-readable CI artifacts.
- Separate correctness gates from hardware-dependent performance observations.

## SLO definitions
- RPO for durably acknowledged events: 0.
- Recovery correctness: 100% of deterministic scenarios reconstruct expected state.
- Split-brain prevention: 0 stale-primary state mutations after fencing.
- Sequence integrity: 0 duplicate/regressed canonical sequences.
- Recovery metrics retained per CI run.

## Acceptance
- Failure matrix runs in CI without wall-clock-dependent flakiness.
- Every injected failure either recovers to expected state or fails closed with no corrupted live state.
- Durable acknowledged events have zero RPO.
- Recovery/failover measurements are retained as CI artifacts.
- Full Rust CI remains green.