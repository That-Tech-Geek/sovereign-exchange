# Bare-cloud deployment

Sovereign Exchange is deployed as a native Rust process on an ordinary Linux VM.

There is deliberately **no Docker, Kubernetes, managed database, Redis, Kafka, or cloud-specific runtime dependency** in the exchange core.

## Current deployment boundary

```
                    Internet
                       |
                TCP gateway (future)
                       |
              +--------+--------+
              |                 |
        Exchange node A   Exchange node B/C
        native Rust       native Rust
              |
       +------+------+
       |             |
   durable WAL    snapshots
       |
   persistent disk
```

The repository currently contains a deterministic exchange core plus a health server. The native client TCP gateway and real multi-process Raft transport are the next implementation stage. Until those are merged and certified, deployment must not be described as a networked HA exchange.

## VM requirements

The experimental baseline is intentionally boring:

- Linux x86_64
- 1 vCPU minimum for functional deployment
- 2 GB RAM recommended for the first 392-book experiments
- persistent local disk for the command journal and snapshots
- systemd
- outbound network access for administration/deployment
- no container runtime

For a three-node HA experiment, use three independent VMs in the same region. Keep the journal on persistent storage. Do not put correctness state in ephemeral RAM.

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

## Roadmap to a real cloud exchange

1. Native TCP client gateway around the existing session protocol.
2. Networked three-process Raft transport.
3. Durable Raft term/vote/log metadata.
4. Leader fencing and stale-leader rejection.
5. Real process network-partition certification.
6. Restarted-node journal/snapshot catch-up and state-fingerprint convergence.
7. Public TCP edge and leader-aware routing.
8. Journal/snapshot archival.
9. Cost-per-million-orders and VM resource benchmarks.

The acceptance criterion is not "it runs on a VM". It is: **kill a node, lose a network path, restart it, recover durable state, converge with the cluster, and prove RPO/RTO with measurements.**
