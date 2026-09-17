# Deterministic Replay Boundary

## PR8 contract

The journal is the canonical ordered event stream. Replay consumes journal bytes and applies events directly; it never re-runs matching commands.

PR8 currently reconstructs the deterministic event ledger: accepted identities, cancellation/replacement identity transitions, canonical sequence, and immutable trade records.

It deliberately does **not** claim to reconstruct price-time order-book state. The PR7 `OrderAccepted` event contains identity but not the immutable side/price/quantity required to rebuild a resting order. Treating missing fields as recoverable would manufacture state and make crash recovery unsafe.

## Required PR9 extension

Before snapshot + journal-tail recovery is enabled, accepted/replacement events must carry enough immutable order state to reconstruct:

- instrument and account identity
- client/exchange order identity
- side
- limit price
- original quantity
- client timestamp metadata
- canonical sequence
- remaining quantity through subsequent trade events

Replay must apply events as state transitions, never regenerate matches or wall-clock timestamps.

## Failure policy

Malformed headers, checksum failures, unsupported versions, payload truncation, header/event sequence disagreement, and sequence regression are fatal replay errors. Replay must never silently skip a damaged record.
