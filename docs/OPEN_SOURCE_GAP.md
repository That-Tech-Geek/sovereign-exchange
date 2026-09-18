# Open-Source Gap Register

This document is intentionally blunt about the remaining gap to production exchange infrastructure.

## Replication

- Complete consensus protocol and automatic leader election
- Durable majority commit semantics integrated with the journal
- Snapshot + log-tail transport between nodes
- Network partitions, stale leaders, split-brain, and quorum-loss behavior
- Measured RPO/RTO

## Market data

- Wire-level incremental feed
- Snapshot + replay recovery service
- Multicast/UDP or equivalent low-overhead distribution
- Feed sequence persistence and consumer resynchronization

## Trading connectivity

- FIX/FIXT/FIXP-compatible session implementation
- OUCH-style native binary order gateway
- authentication, authorization, replay, rate limits, and session persistence

## Exchange controls

- Pre-trade risk
- account balances and reservations
- fees/rebates
- ledger
- settlement
- trading-session enforcement wired into the matcher
- self-trade prevention and throttling
- admin controls and audit trail

## Verification

- property-based matching tests
- differential replay tests
- randomized replica equivalence
- fault injection and process-kill tests
- multi-machine benchmarks
- cache/NUMA characterization
- sustained load and tail-latency regression thresholds

## Operations

- metrics/tracing
- durable backup and restore
- key/credential management
- deployment manifests
- upgrade/rollback compatibility
- documented disaster-recovery runbooks
