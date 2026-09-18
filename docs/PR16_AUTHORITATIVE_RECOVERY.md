# PR16: Authoritative journal and full CLOB recovery

## Objective
Make the durable journal authoritative for recovery and make snapshot + journal tail sufficient to reconstruct the complete live exchange state.

## Scope
- Define a canonical, versioned state-event schema containing every field required to reproduce state.
- Record accepted commands and all resulting state transitions on the authoritative journal path.
- Cover New, Cancel, Replace, partial Fill, complete Fill, rejection/expiry where applicable, lifecycle transitions, and canonical sequence state.
- Persist deterministic event-time/state inputs; replay must not consult wall-clock time, randomness, network state, or mutable external configuration.
- Add a recovery builder that loads snapshot N and applies journal events N+1 through M.
- Reconstruct all 392 books, price levels, FIFO order chains, active-order registry, IDs, remaining quantities, sequence watermark, and trade identity state.
- Define durable acknowledgement semantics: an event is acknowledged as durable only after the configured journal durability boundary.
- Reject gaps, duplicates, regressions, malformed events, unsupported schema versions, and invalid snapshot/journal boundaries.
- Add live-vs-replay equivalence tests covering cross-instrument isolation, FIFO, cancel, replace, partial fills, full fills, and long tails.
- Add crash/restart tests proving acknowledged state survives process restart.

## Invariants
- The same canonical event stream produces the same observable exchange state.
- No event after the recovery watermark may be applied twice.
- No durably acknowledged event may disappear after recovery.
- Recovered order identity and FIFO priority exactly match the pre-crash state.
- Canonical sequence is monotonic and contiguous where required.

## Explicitly NOT in this PR
- Network replication or standby promotion.
- Market-data fanout.
- Chaos/SLO harness beyond recovery tests required here.
- Matcher sharding or latency optimisation.

## Acceptance
- Snapshot N + journal N+1..M reconstructs state equivalent to the live matcher at M.
- Randomized command streams pass with deterministic seeds.
- Corrupt/truncated/gapped journals fail closed without mutating recovered state.
- Durable-acknowledged events have zero recovery loss in CI.
- Full Rust CI remains green.