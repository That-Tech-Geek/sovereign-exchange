# Vercel Deployment Architecture

## Purpose

Vercel hosts the Sovereign Exchange web cockpit and its stateless API gateway. The authoritative matcher remains the native Rust process.

This split is deliberate. Vercel Functions can scale to multiple execution instances, while the exchange contract requires exactly one authoritative single-writer matching owner. A serverless function must therefore never become an independent matcher replica.

## Runtime topology

```
Browser
   |
   v
Vercel
  React/Vite cockpit
   |
   +--> /api/health
   +--> /api/orders
   +--> /api/book/:instrument
   +--> /api/trades/:instrument
             |
             v
     EXCHANGE_ENGINE_URL
             |
             v
   Native Rust authoritative engine
       single writer
       canonical sequencing
       CLOB
       risk / ledger
       durable journal
       replay / recovery
```

The Vercel API layer is a transport adapter only. It does not assign exchange sequence numbers, mutate books, perform matching, write authoritative state, or invent fallback orders.

## Contract preserved

The Vercel boundary does not change:

- 196 sovereign maximum.
- Spot + future topology, 392 instrument capacity.
- One matching owner.
- Canonical exchange sequencing.
- Price/time priority.
- Account + instrument + client-order-ID identity.
- Typed rejection semantics.
- Durable journal and deterministic replay remain native-engine responsibilities.
- Firestore, GitHub, Vercel, analytics, and archival systems remain outside the matching critical path.

## Important deployment rule

Do **not** point `EXCHANGE_ENGINE_URL` at another Vercel Function that contains a second matcher. Doing so would create multiple potential authorities and violate the production contract.

The Vercel project is therefore an exchange control-plane / cockpit deployment, not a claim that Vercel's autoscaled function fleet is a single exchange process.

## Environment

Set:

```text
EXCHANGE_ENGINE_URL=https://<authoritative-engine-host>
```

The API returns HTTP 503 with `authoritative: false` when the variable is missing or the engine is unreachable. It never silently switches to a fake matching implementation.

## 250 MB constraint

The historical 250 MB Vercel Function package limit is not the exchange memory contract. The native engine's current five-million-slot pool is intentionally not allocated inside a stateless Vercel gateway.

If a Vercel-hosted Rust/container matcher is evaluated later, it must first prove single-writer ownership, state durability, failover fencing, and memory/latency budgets before it can replace the native authoritative deployment.

## Local development

Run the cockpit with:

```bash
npm install
npm run build
```

Run the authoritative engine separately and point the Vercel API layer at its HTTP endpoint.

## Release gate

A Vercel release must pass the repository's Rust certification/recovery tests, frontend type/build checks, and the Vercel architecture contract scanner before it can be considered deployable.
