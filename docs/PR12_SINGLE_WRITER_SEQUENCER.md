# PR12: Single-writer sequencer

## Objective
Separate multi-producer ingress from the single owner of mutable market state.

## Scope
- Canonical ingress sequencer.
- Preallocated queue/sequencer structures.
- Explicit ownership boundaries around matcher state.
- Backpressure and saturation semantics.
- Throughput/tail-latency regression harness comparing the existing queue with the new design.

## Acceptance
- One canonical sequence source.
- No shared mutable book state across producers.
- Saturation remains bounded and observable.
- Existing deterministic matching semantics remain unchanged.
