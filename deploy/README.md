# Cloud deployment stack

This directory is the deployment boundary around the deterministic exchange core.

## Current topology

```
                    Cloud Load Balancer / API edge
                              |
                              v
                     +------------------+
                     | exchange pod     |
                     | Rust matching    |
                     | + health server  |
                     +--------+---------+
                              |
                              v
                   PersistentVolume / WAL
```

The hot matching path remains isolated from databases and cloud APIs. The command journal is the durable recovery boundary.

## Local

```bash
docker compose -f deploy/docker-compose.yml up --build
```

Health is exposed on port 8080.

## Kubernetes

```bash
kubectl apply -f deploy/k8s/exchange.yaml
kubectl apply -f deploy/k8s/network-policy.yaml
```

The first cloud deployment intentionally runs **one** exchange replica. The current consensus implementation is not yet a multi-process network cluster, so deploying three pods as if they were HA would be misleading.

## Production stack roadmap

1. Network gateway for the native session protocol on a dedicated TCP endpoint.
2. Three real exchange processes with durable Raft state and node-to-node transport.
3. Leader fencing, quorum loss and network-partition certification.
4. Restarted-node catch-up and state-fingerprint convergence.
5. External market-data fanout and sequence recovery.
6. Immutable journal/snapshot archival to object storage.
7. Metrics/tracing/logging collection.
8. Cloud load balancer, DNS, TLS and operator/admin plane.
9. Capacity/load testing against the full network path.

The exchange core must remain the source of truth; cloud services are control-plane, durability, observability and edge infrastructure, not matching-path dependencies.
