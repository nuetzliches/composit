# Composit

**Creator Control for Agent-Generated Ecosystems.**

Open specification + open-core product that gives creators visibility and control
over the services, data, and relationships that AI agents build on their behalf.

---

## The Problem

AI agents solve problems fast. They spin up cron jobs, wire webhooks, provision
databases, and call APIs — in minutes. The solutions work. But nobody keeps track.

After a few weeks the creator has dozens of agent-generated micro-services,
scattered state, invisible dependencies, and no central view of what exists,
what it costs, or what breaks when something goes down.

**Context engines solve this for the AI. Nothing solves it for the human.**

Humans have the smaller context window.

---

## The Idea

Composit is two things:

1. **An open specification** — a declarative format (Compositfile) and protocols
   (Manifest Discovery, Contract Trust, Policy Interface) that describe what a
   creator's ecosystem looks like, who provides what, and under what rules.

2. **A product** — CLI, registry, and integrations that make the spec useful
   day one. Open-core: self-hosted single-creator is free, multi-creator /
   managed registry / compliance features are commercial.

The spec establishes the standard. The product bootstraps adoption.
Same playbook as MCP, Docker, Terraform.

---

## Core Thesis

> "Which tool?" includes "which company?"

An agent that needs a capability (scheduling, event routing, knowledge search)
should not just pick a library — it should discover **providers**, evaluate their
terms, and provision within the creator's policies. Composit makes that machine-
readable.

---

## Capability Categories

From the creator's control perspective — "what do I need visibility into?":

| Category           | Creator Question                        | Reference Provider |
|--------------------|-----------------------------------------|--------------------|
| **Scheduling**     | When does what run?                     | [croniq][]         |
| **Events**         | What triggers what?                     | [hookaido][]       |
| **Knowledge**      | What do my agents know?                 | [powerbrain][]     |
| **Identity**       | Who/what is allowed to do what?         | TBD                |
| **Cost**           | What does this cost me?                 | composit-native    |
| **State**          | Where does data live? Who created it?   | composit-native    |
| **Observability**  | What happened, and why?                 | composit-native    |

**Knowledge vs. State:** Knowledge (powerbrain) is about curated, policy-controlled
context *for agents*. State in composit is inventory — the creator's view of *where
data exists across the ecosystem*, regardless of whether agents consume it.

**State vs. Observability:** State = where data is now. Observability = what happened
over time (traces, logs, audit trail). Addresses the "silent ecosystem failure"
problem — agents fail silently, and the creator needs to know.

**Identity includes Secrets:** Not just who-is-allowed-what, but also which API keys
and tokens are in use, and whether they are rotated and scoped correctly.

State, Cost, and Observability are not separate infrastructure projects. They are
composit-native concerns — metadata that composit itself tracks by observing
the providers.

[croniq]: https://github.com/nuetzliches/croniq
[hookaido]: https://github.com/nuetzliches/hookaido
[powerbrain]: https://github.com/nuetzliches/powerbrain

---

## Architecture Layers

```
┌─────────────────────────────────────────────────────┐
│  Creator (Human)                                    │
│  "What exists? What costs? What needs attention?"   │
├─────────────────────────────────────────────────────┤
│  Composit Control Plane                             │
│  ├─ Compositfile      (declarative state-of-world)  │
│  ├─ Policy Engine     (OPA: what's allowed)         │
│  ├─ State Tracker     (inventory + drift detection) │
│  ├─ Cost Aggregator   (metering across providers)   │
│  └─ Manifest Registry (provider discovery)          │
├─────────────────────────────────────────────────────┤
│  Provider Layer (MCP-native, independent projects)  │
│  ├─ croniq       → scheduling capabilities          │
│  ├─ hookaido     → event routing capabilities       │
│  ├─ powerbrain   → knowledge + policy capabilities  │
│  ├─ authentik*   → identity capabilities            │
│  └─ [third-party providers via Manifest protocol]   │
├─────────────────────────────────────────────────────┤
│  Agent Layer                                        │
│  ├─ OpenClaw, Claude Code, custom agents            │
│  ├─ Agents discover providers via Manifest          │
│  ├─ Agents provision within creator policies        │
│  └─ All actions tracked by composit state tracker   │
└─────────────────────────────────────────────────────┘
```

---

## Key Concepts

### Compositfile

Declarative document describing the creator's ecosystem. Not a deployment tool
(that's docker-compose / Terraform). A **visibility tool** — what exists, why,
and how it connects.

```hcl
workspace "nuetzliche" {

  provider "croniq" {
    endpoint "https://croniq.nuetzliche.it"
    capabilities ["scheduling"]
    protocol "mcp"
  }

  provider "hookaido" {
    endpoint "https://hooks.nuetzliche.it"
    capabilities ["events"]
    protocol "mcp"
  }

  provider "powerbrain" {
    endpoint "https://mcp.nuetzliche.it/powerbrain"
    capabilities ["knowledge"]
    protocol "mcp"
    compliance ["gdpr", "eu-ai-act"]
  }

  business_case "pr-review-bot" {
    uses      ["croniq", "hookaido", "powerbrain"]
    created   "2026-03-15"
    owner     "agent:claude-code"
    approved  "sebastian"
    budget    "50 EUR/month"
  }

  policy "agent-provisioning" {
    source "policies/composit.rego"
  }
}
```

### Public Manifest (Provider Discovery)

Hosted at a well-known URL (e.g., `composit.example.com/.well-known/composit.json`).
Unauthenticated. Enables agents and creators to discover providers before any
contract exists.

```json
{
  "composit": "0.1",
  "provider": "nuetzliche",
  "capabilities": [
    {
      "type": "scheduling",
      "protocol": "mcp",
      "tools": 12,
      "description": "Distributed cron with retries, DLQ, calendar rules"
    }
  ],
  "compliance": ["gdpr", "eu-ai-act"],
  "region": "eu-central-1",
  "contract_endpoint": "https://composit.nuetzliche.it/contract"
}
```

### Contract Trust Protocol

After a business relationship is established, authenticated access unlocks:
- Concrete MCP endpoints and credentials
- Rate limits, SLAs, pricing tier
- Dedicated instance URLs
- Audit data feeds

Authentication via API key, mTLS, or OAuth token exchange.

### Policy Interface

Creators define rules (OPA/Rego) that constrain what agents can provision:

```rego
package composit.provision

# Max 5 cron jobs per business case
deny[msg] {
    input.action == "create_job"
    count(existing_jobs[input.business_case]) >= 5
    msg := sprintf("job limit reached for %s", [input.business_case])
}

# Only approved providers
deny[msg] {
    input.provider_id
    not input.provider_id in data.approved_providers
    msg := sprintf("provider %s not approved", [input.provider_id])
}

# Monthly cost cap
deny[msg] {
    input.estimated_cost > 0
    total := sum_costs(input.business_case)
    total + input.estimated_cost > data.budgets[input.business_case]
    msg := "budget exceeded"
}
```

---

## Business Model

**Open Source (composit-core):**
- Compositfile format + parser
- CLI (validate, status, diff, apply)
- Manifest Discovery protocol
- Local provider integrations
- Single-creator, self-hosted

**Commercial (composit-cloud):**
- Multi-creator / team workspaces
- Managed manifest registry
- Contract management (provider <> consumer)
- Cost aggregation + alerting
- Audit trail + compliance reporting
- Hosted manifests with SLA

The reference providers (croniq, hookaido, powerbrain) remain independent
open-source projects. They are composit providers, not composit dependencies.

---

## Prior Art & Positioning

| Tool            | What it does                     | How composit differs               |
|-----------------|----------------------------------|-------------------------------------|
| Backstage/Port  | Developer portal / service catalog | For platform teams, not creators   |
| Terraform       | Infrastructure provisioning      | Deploys infra; composit *observes* it |
| docker-compose  | Container orchestration          | Runtime composition; composit is visibility |
| MCP             | Agent ↔ tool protocol            | composit builds *on* MCP, not *beside* it |
| Datadog         | Infrastructure observability     | Monitors infra; composit tracks business cases |

Composit doesn't replace any of these. It's the layer above — answering
"what does my agent-generated ecosystem look like?" rather than "how do I
deploy/monitor individual components?"

---

## Status

**Phase: Exploration.**

This repository documents the strategic thinking behind composit.
No code yet — spec-first, then reference implementation.

---

## Origin

Born from a practical observation: when you build MCP-native infrastructure
(croniq, hookaido, powerbrain) and agents that compose them, the missing
piece is not another tool — it's the creator's ability to see and control
the whole picture.
