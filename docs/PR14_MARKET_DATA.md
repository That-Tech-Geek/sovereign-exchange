# PR14: Deterministic full-depth market data

## Objective
Expose a replayable market-data stream derived from canonical matching events.

## Scope
- Incremental order-book events.
- Trade events.
- Per-instrument sequence numbers.
- Book snapshots with explicit snapshot sequence.
- Snapshot + incremental gap recovery.

## Acceptance
- Snapshot at N followed by N+1..M reconstructs the exact book.
- Sequence gaps are detectable.
- Every outbound event is attributable to a canonical matcher event.
