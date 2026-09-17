# Exchange Core Architecture

## Current authority model

```text
Client / Network
      |
      v
Admission validation
      |
      v
Canonical sequence assignment
      |
      v
Single-writer matcher
      |
      +----> instrument book 0..391
      |
      v
Trade / exchange events
      |
      +----> persistence
      +----> market data
      +----> ledger / positions
      +----> settlement
      +----> analytics
```

The matcher owns live order-book state. Consumers do not write back into matcher state.

## Instrument partitioning

The engine contains a fixed vector of independent books indexed by `instrument_id`. Instrument routing happens before matching. Orders from different instruments cannot cross.

The current maximum is 392 instruments: 196 sovereigns multiplied by spot and future. This is a logical capacity, not a requirement to create a thread per instrument.

## Identity namespaces

Every accepted non-cancel order has:

- client order ID: supplied by the client,
- exchange order ID: assigned by the exchange,
- canonical sequence number: assigned by the exchange admission path.

These fields answer different questions and must not be conflated.

## Ordering

The canonical sequence is the ordering authority. The client timestamp remains useful for diagnostics and external reconciliation but cannot affect matching priority.

A single-writer matcher must receive accepted commands in sequence order. This gives replay a deterministic command stream and prevents thread scheduling from becoming market semantics.

## Persistence boundary

The current scavenger/persistence components are not authoritative persistence. Durable journaling is a later architectural stage. Until then, a process crash can lose in-memory state.

The planned durable architecture is:

```text
Matcher state
     |
     +--> append-only event journal
               |
               +--> replay
               +--> snapshots
               +--> market data
               +--> ledger
               +--> recovery / standby
```

Cloud services and repositories are archival/operational consumers, never matching dependencies.

## Non-goals of the baseline

This baseline does not introduce:

- Kafka,
- Redis,
- Kubernetes,
- distributed consensus,
- network sharding,
- multiple matcher threads,
- speculative hardware acceleration.

Those decisions require a concrete availability or correctness requirement first.
