# HN Launch Draft

Stand: 2026-04-12 (post-Validation, retargeted to Platform Engineers)

---

## Title Options

Pragmatisch (Platform Eng Audience):
- **Show HN: `composit scan` — see every service your AI agents created**
- **Show HN: Your agents built 47 services last month. Here's the inventory**
- **Show HN: An open spec for tracking what AI agents build in your infrastructure**

Philosophisch (breitere Audience):
- **The human context window problem**
- **Your context window is smaller than your LLM's — here's the fix**

---

## Post

AI agents write code fast. But they also provision infrastructure fast — cron
jobs, webhooks, databases, API integrations. And nobody keeps track.

If you're a platform engineer running drift audits, comparing AWS resources to
Terraform state, wondering who created that service on port 8443 — you know
this pain. If you're a CTO who found out about a $22K/month bill from forgotten
test databases your agents spun up — you know it too.

The numbers back it up: only 25% of CIOs have full visibility into agents
operating in their org. The average enterprise sees 223 shadow-AI incidents
per month. One AI agent ran `terraform destroy` on production because of a
missing state file — wiping 2.5 years of data.

**Context engines exist for the AI.** RAG, vector databases, policy engines —
all designed to give agents the right information. That problem is solved.

Nobody is solving it for the human. Humans have the smaller context window.

---

**Composit** is an open spec and open-core CLI for visibility over
agent-generated infrastructure.

The core: a **Compositfile** — a declarative document that describes what
exists in your ecosystem, why it exists, who created it, and what rules govern it.
Not a deployment tool (that's Terraform). Not a monitoring tool (that's Datadog).
A **visibility tool** — the missing layer between "agents build things" and
"the platform team understands what exists."

`composit scan` is the entry point. Zero config. Point it at a project and it
inventories agent-created artifacts: MCP configs, Terraform state, Docker files,
cron entries, webhook configs. Single-page output.

Three protocols (the spec):

1. **Manifest Discovery** — providers publish capabilities at a well-known URL.
   Agents discover who offers scheduling, event routing, knowledge search
   — and under what terms. "Which tool?" now includes "which company?"

2. **Contract Trust** — after a business relationship is established, authenticated
   access unlocks concrete endpoints, SLAs, and pricing. Machine-readable B2B.

3. **Policy Interface** — teams define OPA rules that constrain what agents
   can provision. Cost caps per business case. Provider whitelists. Region
   restrictions for regulated data.

The thesis: agents are right to build fast. The problem isn't that they produce
throwaway solutions — it's that nobody tracks the accumulation. Composit doesn't
slow agents down. It gives the platform team a map of what they've built.

---

**What exists today:**

- `composit scan` CLI (TypeScript, zero-config)
- Compositfile spec draft (v0.1)
- Public manifest schema (JSON, well-known URL discovery)
- OPA policy examples (agent limits, provider approval)
- Three MCP-native reference providers we built:
  [croniq](https://github.com/nuetzliches/croniq) (scheduling),
  [hookaido](https://github.com/nuetzliches/hookaido) (events),
  [powerbrain](https://github.com/nuetzliches/powerbrain) (knowledge)

GitHub: https://github.com/nuetzliches/composit

---

## Key Talking Points (for comments)

**"Who is this for?"**
Platform engineers and CTOs at teams where 3+ developers use AI coding agents.
If you have agents provisioning infrastructure and no central inventory of what
they created — this is for you. Solo devs are welcome too (the CLI is free),
but the product is built for teams.

**"How is this different from Backstage / Port?"**
Backstage catalogs services humans built. Composit tracks what agents build
on your behalf — including things you didn't explicitly ask for. Backstage is
a developer portal. Composit is an agent-infrastructure inventory.

**"How is this different from Terraform?"**
Terraform provisions infrastructure. Composit observes it. The Compositfile is
not "apply this state" — it's "here's what exists and why." Think `terraform
state list` elevated to a product, with business-case attribution and cost
tracking. Critically: Terraform only knows about declared resources. Agents
create things outside Terraform all the time.

**"Why not just use Datadog / Grafana?"**
Observability tools monitor health. Composit tracks attribution. "Is this
service healthy?" vs. "Why does this service exist, who created it, what
does it cost, and what happens if I remove it?"

**"Why would an agent use composit instead of just building directly?"**
It wouldn't — and that's the point. Agents keep building fast. Composit is
not in the agent's hot path. It observes what providers report and gives the
platform team a consolidated view. The agent doesn't need to change.

**"Isn't this just a CMDB?"**
CMDBs require manual maintenance and are always out of date. Composit
auto-populates from MCP-native providers. The providers report state, composit
aggregates it. The Compositfile is intent; reality is tracked automatically.

**"Why should I trust an open spec from a solo developer?"**
Fair question. The spec is designed to be contributed to a foundation (like
OpenTelemetry moved to CNCF) if it gains traction. The value of an open spec
is that even competitors building on it validates the standard. If Port or
Backstage adopt the Compositfile format, composit wins.

**"MCP lock-in?"**
MCP is an open protocol (by Anthropic). Composit builds on MCP because it's
what agents already speak. If a better agent-tool protocol emerges, composit
adapts. The spec is protocol-aware, not protocol-locked.

---

## Community Launch Plan

| Channel | Timing | Content |
|---------|--------|---------|
| **Hacker News** (Show HN) | Day 1 | Post + CLI demo |
| **r/devops** | Day 1-2 | Cross-post, focus on drift-audit angle |
| **r/platformengineering** | Day 1-2 | Focus on agent sprawl visibility |
| **Platform Engineering Slack** | Day 2-3 | Discussion thread |
| **CNCF Slack** | Day 2-3 | #developer-experience channel |
| **DevOps Meetups** (local) | Week 2+ | Lightning talk / demo |
| **croniq/hookaido/powerbrain READMEs** | After positive signal | Add composit reference |
