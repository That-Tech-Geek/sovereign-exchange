# Bare-cloud deployment

Sovereign Exchange is deployed as a native Rust process on an ordinary Linux VM.

There is deliberately **no Docker, Kubernetes, managed database, Redis, Kafka, or cloud-specific runtime dependency** in the exchange core.

## Current deployment boundary

This deployment is intentionally a **single authoritative central exchange process**.

```text
Internet / client traffic
          |
     optional edge
          |
   +------+------+
   |   exchange |
   |   process  |
   +------+------+
          |
     durable WAL
          |
      snapshots
          |
   persistent disk
```

There is no production requirement for networked Raft or multiple active exchange processes. Availability comes from durable state, deterministic replay, snapshots, systemd supervision, and fast process restart. Consensus modules remain available as research infrastructure but are not on the central execution path.

## VM requirements

The reference deployment target is **Oracle Cloud Always Free Ampere A1**.

- Arm64 / `aarch64`
- target budget: **2 OCPUs / 12 GB RAM** for the current Always Free allowance
- Linux
- persistent block volume for the journal and snapshots
- systemd
- no container runtime
- no managed database, Redis, Kafka, or other correctness dependency

Keep the exchange process itself boring: one process, RAM-resident books, synchronous durable command journal, periodic snapshots, and automatic restart.

The recovery-capacity benchmark is:

`cargo test --release --test oracle_always_free_recovery -- --nocapture`

It measures journal size, bytes per command, replay throughput, replay time, and Linux RSS across increasing durable journal sizes. The acceptance budget is replay of each benchmark case in under 1 second.

## Build and install

On the VM:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl git
git clone https://github.com/That-Tech-Geek/sovereign-exchange.git
cd sovereign-exchange/exchange_core
cargo build --release
cd ..
sudo bash deploy/install.sh
```

The installer:

1. creates the unprivileged `exchange` service account;
2. creates `/var/lib/sovereign-exchange`;
3. installs the release binary under `/opt/sovereign-exchange/bin`;
4. installs a hardened systemd unit;
5. enables automatic restart;
6. sends stdout/stderr to journald.

## Operations

```bash
sudo systemctl status sovereign-exchange
sudo systemctl restart sovereign-exchange
sudo journalctl -u sovereign-exchange -f
sudo journalctl -u sovereign-exchange --since "1 hour ago"
```

The journal and snapshots live under:

```
/var/lib/sovereign-exchange/
```

Back up that directory at the infrastructure boundary. Never copy a live journal by truncating or rewriting it in place.

## Security boundary

The service runs without root privileges and uses systemd filesystem/process hardening. The exchange process is the source of truth for matching and recovery. Cloud services may provide DNS, a load balancer, object storage, monitoring, or access control, but they must not become required dependencies for deterministic matching.

## Roadmap

1. Native client/API gateway on the central process.
2. Production readiness/liveness separation.
3. Graceful shutdown with durable flush before exit.
4. Snapshot cadence and journal archival.
5. Oracle VM resource, disk, and cost-per-million-order measurements.
6. External backup/restore drills using object storage without making object storage a matching dependency.

The acceptance criterion is: **kill the process, restart it, replay durable state, accept the first post-recovery order, and prove the recovery budget with measurements.**