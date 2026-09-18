# PR13: Hot standby replication and failover

## Objective
Turn journal durability into continuous replica readiness and deterministic failover.

## Scope
- Active/standby roles.
- Journal sequence watermarks and standby lag.
- Heartbeats and fencing tokens.
- Snapshot + journal-tail promotion.
- Deterministic promotion and stale-primary rejection.

## Acceptance
- Standby never promotes from an unverified state.
- Promotion resumes at the last durable canonical sequence.
- No split-brain writer is permitted.
- Recovery and failover SLOs are measured in CI.
