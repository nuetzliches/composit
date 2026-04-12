# Knowledge vs. State — Why the Distinction Matters

## The Question

If powerbrain handles enterprise knowledge with policy controls, vector search,
and audit trails — isn't "state" already covered?

## The Answer: No. Different Layer, Different Purpose.

### Knowledge (powerbrain)

Powerbrain answers: **"What should this agent know right now?"**

- Curated, indexed, policy-controlled READ access
- Data flows IN (ingestion) and OUT (search results for agents)
- Powerbrain knows its own state perfectly (collections, chunks, access logs)
- It does NOT know about data that lives outside its domain

```
Powerbrain knows:
  "Collection pb_code has 2,400 chunks, classification: internal,
   last ingest: yesterday, access policy: engineering-only"

Powerbrain does NOT know:
  "An agent created a SQLite DB in a container last week.
   3 cron jobs use it as backend. Nobody makes backups.
   The container has no health check."
```

### State (composit-native)

Composit State answers: **"Where does data live across my ecosystem?"**

- Inventory: what databases, files, caches, queues exist
- Topology: which services depend on which data stores
- Provenance: who created it (agent? human?), when, why
- Risk: backup status, redundancy, access controls
- Drift: does reality (composit-report) match what the Compositfile declares?

This is not a storage layer. Composit doesn't store data — it tracks
**metadata about where data lives and who depends on it**.

## Relationship

```
             ┌─────────────────────────┐
             │   Composit State Layer  │  ← Creator's view
             │   (inventory, topology, │     "What exists?"
             │    provenance, risk)    │
             └────────┬────────────────┘
                      │ observes
        ┌─────────────┼──────────────┐
        ▼             ▼              ▼
   ┌─────────┐  ┌──────────┐  ┌──────────┐
   │ Qdrant  │  │ Postgres │  │ Agent DB │  ← Actual data stores
   │(powerbr)│  │(hookaido)│  │(unknown) │
   └─────────┘  └──────────┘  └──────────┘
        ▲
        │ curates + serves
   ┌─────────────────────┐
   │ Powerbrain          │  ← Agent's view
   │ (knowledge search)  │    "What should I know?"
   └─────────────────────┘
```

Knowledge is a **subset** of state. Powerbrain manages curated read-access
for agents. Composit State tracks the full data topology for the creator.

## Practical Implication

Powerbrain is complete in its domain. It doesn't need state-tracking features.
State awareness belongs in composit itself — it's a cross-cutting concern
that observes ALL providers, not just the knowledge layer.
