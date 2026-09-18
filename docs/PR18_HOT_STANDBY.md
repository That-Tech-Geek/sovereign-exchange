# PR18: Hot standby replication and fenced failover

## Objective
Provide a continuously advancing standby that can reconstruct and assume exchange ownership without split brain.

## Scope
- Define active/standby roles and explicit role transitions.
- Replicate the authoritative canonical event stream in sequence order.
- Track journal durability and standby-applied watermarks separately.
- Add heartbeat/liveness state and a monotonic generation/fencing token.
- Prevent an old primary from accepting commands after fencing.
- Promote standby only from a verified snapshot plus contiguous journal tail.
- Make promotion idempotent and reject stale generations.
- Expose active, standby, journal, and recovery watermarks operationally.
- Add failover tests for active crash, standby lag, stale-primary messages, duplicate replication, and restart.
- Define degraded behavior when standby cannot satisfy the replication contract.

## Invariants
- At most one generation is authoritative for command admission.
- Standby never invents canonical sequence.
- Promotion cannot skip an acknowledged journal event.
- Old-primary commands are rejected after fencing.
- Promoted standby is equivalent to the last valid replicated state.

## Explicitly NOT in this PR
- Multi-region consensus.
- Automatic cross-datacenter topology.
- Market-data fanout.
- Performance tuning beyond replication correctness.

## Acceptance
- Active crash followed by promotion yields equivalent CLOB and sequence watermark.
- Stale-primary attempts cannot mutate state.
- Duplicate/out-of-order replication is detected and rejected.
- Watermarks are observable and testable.
- PR16 and PR17 tests remain green.