# PR19: Deterministic full-depth market data

## Objective
Expose canonical exchange state as a recoverable full-depth market-data stream without creating a second source of truth.

## Scope
- Define typed market-data events derived only from canonical exchange events.
- Publish order-book changes, trades, lifecycle changes, and sequence metadata.
- Maintain per-instrument market-data sequence tied to canonical exchange sequence.
- Provide full-depth snapshots with explicit snapshot sequence.
- Provide incremental updates N+1..M after a snapshot.
- Add gap detection and deterministic snapshot-resync.
- Preserve price-time ordering and remaining quantities in published depth.
- Ensure cancelled/replaced/filled orders update exactly once.
- Test empty books, multiple levels, FIFO, partial fills, cancels, replaces, isolation, and resync.
- Consumers are read-only with respect to matcher state.

## Invariants
- Market data is derived from canonical state transitions, never independently matched.
- Snapshot sequence defines the exact base for incremental replay.
- Updates cannot be silently skipped, duplicated, or applied out of order.
- Gap recovery returns an exact current snapshot.

## Explicitly NOT in this PR
- Full WebSocket/HTTP gateway unless only a thin transport adapter is needed.
- UI/cockpit work.
- New matching semantics.
- Standby election.

## Acceptance
- Snapshot N + deltas N+1..M reconstructs the exact published book at M.
- Gap detection is deterministic.
- Resync restores consumers without mutating matcher state.
- Trade/depth events preserve canonical ordering.
- PR16–18 tests remain green.