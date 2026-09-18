# PR15: Failure-injection and recovery SLO suite

## Objective
Prove availability and correctness under realistic exchange failures.

## Failure matrix
- Matcher crash/restart.
- Journal truncation/corruption.
- Snapshot interruption.
- Queue saturation.
- Pool exhaustion.
- Standby lag.
- Stale-primary promotion.
- Disk-full simulation.

## Acceptance
- No phantom or duplicated trades.
- No sequence regression.
- Durable acknowledged events have zero RPO.
- RTO, standby lag, and recovery success rate are measured and retained as CI artifacts.
