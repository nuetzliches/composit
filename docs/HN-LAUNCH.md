# HN Launch Draft

Stand: 2026-04-14 (post-Positioning: Governance-as-Code)

---

## Title Options

Governance-Angle:
- **Show HN: Governance-as-code for AI-generated infrastructure**
- **Show HN: `composit diff` — does your infrastructure match your governance?**
- **Show HN: AI agents are building your infra. Here's what doesn't match your contracts.**

Pragmatisch:
- **Show HN: `composit scan` + `composit diff` — inventory your infra, detect governance drift**

---

## Post

Infrastructure governance was broken before AI. Most teams can't answer:
"What services do we run, and do they match what we declared?"

AI makes this urgent. Agents provision cron jobs, webhooks, databases, API
integrations — in minutes. The solutions work. But they accumulate outside
any governance framework.

The numbers: only 25% of CIOs report full visibility into agents in their org.
223 shadow-AI incidents per month at the average enterprise. 78% of IT leaders
report unexpected costs from AI agent usage.

Every company will use AI. The question is how they keep control.

---

**Composit** is governance-as-code for infrastructure.

Two artifacts, one comparison:

1. **`composit scan`** → the IS-state. Point it at a repo, get a machine-readable
   inventory: Docker services, Terraform resources, Caddyfiles, CI/CD workflows,
   Prometheus configs — plus who created what (AI agent vs. human, via git-blame).

2. **`Compositfile`** → the SHOULD-state. Declare your governance: approved
   providers, budget constraints, policies. Version it. Review it in PRs.

3. **`composit diff`** → the gap. Unapproved providers, budget violations,
   missing resources, governance drift.

Not a deployment tool (that's Terraform). Not a monitoring tool (that's Datadog).
A **governance tool** — the missing layer between "infrastructure exists" and
"it matches what we declared."

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

- `composit scan` CLI (Rust, zero-config, 9 built-in scanners)
- `composit status` (aggregated view of last scan)
- `composit diff` (IS-vs-SHOULD, HTML/YAML/JSON output with severity)
- composit-report.yaml format (v0.1) + Compositfile governance spec (draft)
- Public manifest schema (JSON, well-known URL discovery)
- OPA policy examples (agent limits, provider approval)
- Three MCP-native reference providers we built:
  [croniq](https://github.com/nuetzliches/croniq) (scheduling),
  [hookaido](https://github.com/nuetzliches/hookaido) (events),
  [powerbrain](https://github.com/nuetzliches/powerbrain) (knowledge)

**Known gaps** (honest): No Kubernetes / Helm / nginx scanner yet.
OPA rules can be referenced in the Compositfile but aren't evaluated at
runtime yet. Live provider API queries are scaffolded, not wired.

GitHub: https://github.com/nuetzliches/composit

---

## Key Talking Points (for comments)

**"Who is this for?"**
Platform engineers and CTOs at teams where 3+ developers use AI coding agents.
If you have agents provisioning infrastructure and no central inventory of what
they created — this is for you. Solo devs are welcome too (the CLI is free),
but the product is built for teams.

**"How is this different from Backstage / Port?"**
Backstage catalogs services. Composit governs infrastructure. Backstage answers
"what do we have?" Composit answers "does what we have match what we declared?"
The Compositfile is governance-as-code — version-controlled, reviewable, diffable.

**"How is this different from Terraform?"**
Terraform provisions infrastructure. Composit observes it. The `composit-report.yaml`
is not "apply this state" — it's "here's what exists and why." Think `terraform
state list` elevated to a product, with attribution and cost tracking. The
Compositfile (governance layer) then defines what *should* be true — like
`.terraform-policy` but for agent-created infrastructure. Critically: Terraform
only knows about declared resources. Agents create things outside Terraform
all the time.

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
aggregates it. The composit-report is reality; the Compositfile is intent.
Drift detection compares the two.

**"Why should I trust an open spec from a solo developer?"**
Fair question. The spec is designed to be contributed to a foundation (like
OpenTelemetry moved to CNCF) if it gains traction. The value of an open spec
is that even competitors building on it validates the standard. If Port or
Backstage adopt the composit report format or Compositfile spec, composit wins.

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
