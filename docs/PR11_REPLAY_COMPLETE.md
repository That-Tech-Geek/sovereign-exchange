# PR11: Replay-complete order lifecycle

## Objective
Make the durable event stream sufficient to reconstruct the live CLOB exactly.

## Scope
- Extend accepted/cancel/replace/fill events with every state-bearing field required for deterministic reconstruction.
- Define canonical event ordering and versioned event schemas.
- Rebuild books, active orders, FIFO state, sequence state, IDs, and trade state from snapshot + journal tail.
- Add randomized live-vs-replay equivalence tests.

## Acceptance
- Replayed state equals live state at every checkpoint.
- Journal corruption, truncation, gaps, duplicates, and unsupported versions fail closed.
- No acknowledged order disappears after recovery.
