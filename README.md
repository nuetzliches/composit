# Composit

**The missing inventory for what AI agents build.**

Open specification + open-core product that gives platform teams visibility and
control over agent-generated infrastructure — across all agents, all providers,
all environments.

---

## The Problem

AI agents solve problems fast. They spin up cron jobs, wire webhooks, provision
databases, and call APIs — in minutes. The solutions work. But nobody keeps track.

After a few weeks, your team's agents have created dozens of services outside
your IaC pipeline. Your platform engineer discovers them during drift audits.
Your CTO discovers them on the cloud bill.

The numbers are real:
- Only 25% of CIOs report full visibility into all agents operating in their org
- 223 shadow-AI incidents per month at the average enterprise
- 78% of IT leaders report unexpected costs from AI agent usage
- One `terraform destroy` from an AI agent wiped 2.5 years of production data

**Context engines exist for the AI.** RAG pipelines, vector databases, policy
engines — all designed to feed the right information to the agent. That problem
is being solved.

Nobody is solving it for the human. Humans have the smaller context window.

---

## The Idea

Composit is two things:

1. **An open specification** — a declarative format (Compositfile) and protocols
   (Manifest Discovery, Contract Trust, Policy Interface) that describe what an
   ecosystem looks like, who provides what, and under what rules.

2. **A product** — CLI, registry, and integrations that make the spec useful
   day one. Open-core: self-hosted is free, team features are commercial.

The spec establishes the standard. The product bootstraps adoption.
Same playbook as MCP, Docker, Terraform.

---

## Core Thesis

> "Which tool?" includes "which company?"

An agent that needs a capability (scheduling, event routing, knowledge search)
should not just pick a library — it should discover **providers**, evaluate their
terms, and provision within the team's policies. Composit makes that machine-
readable.

---

## Capability Categories

From the platform team's perspective — "what do we need visibility into?":

| Category           | Question                                | Reference Provider |
|--------------------|-----------------------------------------|--------------------|
| **Scheduling**     | When does what run?                     | [croniq][]         |
| **Events**         | What triggers what?                     | [hookaido][]       |
| **Knowledge**      | What do agents know?                    | [powerbrain][]     |
| **Identity**       | Who/what is allowed to do what?         | TBD                |
| **Cost**           | What does this cost?                    | composit-native    |
| **State**          | Where does data live? Who created it?   | composit-native    |
| **Observability**  | What happened, and why?                 | composit-native    |

State, Cost, and Observability are composit-native concerns — metadata that
composit tracks by observing the providers. Not separate infrastructure projects.

[croniq]: https://github.com/nuetzliches/croniq
[hookaido]: https://github.com/nuetzliches/hookaido
[powerbrain]: https://github.com/nuetzliches/powerbrain

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Platform Team / CTO                                │
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
│  └─ [third-party providers via Manifest protocol]   │
├─────────────────────────────────────────────────────┤
│  Agent Layer                                        │
│  ├─ Claude Code, Cursor, Devin, custom agents       │
│  ├─ Agents discover providers via Manifest          │
│  ├─ Agents provision within team policies           │
│  └─ All actions tracked by composit state tracker   │
└─────────────────────────────────────────────────────┘
```

---

## Key Concepts

### Compositfile

Declarative document describing the ecosystem. Not a deployment tool — a
**visibility tool**. What exists, why, and how it connects.

→ See [`examples/Compositfile`](examples/Compositfile) for the full format.

### Public Manifest (Provider Discovery)

Hosted at `provider.example/.well-known/composit.json`. Unauthenticated.
Enables agents and teams to discover providers before any contract exists.

→ See [`examples/composit-manifest.json`](examples/composit-manifest.json).

### Policy Interface

OPA/Rego rules that constrain what agents can provision: cost caps, provider
whitelists, region restrictions, job limits per business case.

→ See [`examples/policies/`](examples/policies/).

### Contract Trust Protocol

After a business relationship is established, authenticated access unlocks
concrete MCP endpoints, credentials, SLAs, and pricing.

---

## Business Model

| Tier | Target | Features |
|------|--------|----------|
| **Free CLI** (composit-core) | Solo devs, small teams | `composit scan`, `composit status`, Compositfile parser, local provider integrations |
| **Team** ($29-99/mo) | Platform engineers, 5-50 dev teams | Multi-team dashboard, drift alerts, cost attribution, Slack/Teams integration |
| **Enterprise** ($199+/mo) | Compliance-driven orgs | Audit trail, SOC2/GDPR reporting, managed manifest registry, SSO, SLA |

The reference providers (croniq, hookaido, powerbrain) remain independent
open-source projects. They are composit providers, not composit dependencies.

---

## Prior Art & Positioning

| Tool            | What it does                     | How composit differs                   |
|-----------------|----------------------------------|----------------------------------------|
| Backstage/Port  | Developer portal / service catalog | No AI-agent awareness, no auto-discovery |
| Terraform       | Infrastructure provisioning      | Only tracks declared resources; blind to ad-hoc agent actions |
| Datadog         | Infrastructure observability     | Monitors health; composit tracks business cases + attribution |
| MCP             | Agent-tool protocol              | composit builds *on* MCP, not *beside* it |
| Port            | IDP ($800M valuation)            | Traditional catalog; pivoting to agents but proprietary |
| AWS Agent Registry | Agent inventory (preview)     | AWS-only, proprietary. Composit is cloud-agnostic + open spec. |

---

## Status

**Phase: Spec Draft + CLI PoC.**

Validation complete (6.5/10, ITERATE — April 2026). Building toward first
working CLI (`composit scan`) and published spec draft (Compositfile v0.1).

→ [Next Steps](docs/NEXT-STEPS.md) | [Strategy](docs/STRATEGY.md) | [Open Questions](docs/OPEN-QUESTIONS.md)

---

## Origin

Born from a practical observation: when you build MCP-native infrastructure
(croniq, hookaido, powerbrain) and agents that compose them, the missing
piece is not another tool — it's the platform team's ability to see and
control the whole picture.
