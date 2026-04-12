# HN Launch Draft

---

## Title Options

- **The human context window problem is solved**
- **Your context window is smaller than your LLM's — here's the fix**
- **Composit: Creator Control for Agent-Generated Ecosystems**

---

## Post

We talk a lot about context windows for LLMs. 128k, 200k, 1M tokens. But we
ignore the real bottleneck: **yours.**

An LLM with a 1M context window has perfect recall. You don't. And right now,
AI agents are generating infrastructure on your behalf — cron jobs, webhooks,
database tables, API integrations — faster than you can track.

Three weeks in, you have 40 agent-generated micro-services. You don't know which
ones are still relevant. You don't know what they cost. You don't know what breaks
if one goes down. You've lost control of something you technically own.

**Context engines exist for the AI.** RAG pipelines, vector databases, policy
engines — all designed to feed the right information to the agent at the right
time. That problem is being solved.

Nobody is solving it for the human.

---

**Composit** is an open spec and open-core tool for creator control over
agent-generated ecosystems.

The core idea: a **Compositfile** — a declarative document that describes what
exists in your ecosystem, why it exists, who created it, and what rules govern it.
Not a deployment tool (that's Terraform). Not a monitoring tool (that's Datadog).
A **visibility tool** — the missing layer between "agents build things" and
"I understand what I have."

Three protocols:

1. **Manifest Discovery** — providers publish capabilities at a well-known URL.
   Agents can discover who offers scheduling, event routing, or knowledge search
   — and under what terms. "Which tool?" now includes "which company?"

2. **Contract Trust** — after a business relationship is established, authenticated
   access unlocks concrete endpoints, SLAs, and pricing. Machine-readable B2B.

3. **Policy Interface** — creators define OPA rules that constrain what agents
   can provision. Cost caps per business case. Provider whitelists. Region
   restrictions for regulated data. The agent builds within boundaries.

The thesis: agents are right to build fast and light. The problem isn't that they
produce throwaway solutions — it's that nobody tracks the accumulation. Composit
doesn't slow agents down. It gives the creator a map of what they've built.

---

**What exists today:**

- Spec exploration + strategic docs
- Compositfile format with examples
- Public manifest schema (JSON)
- OPA policy examples (agent limits, provider approval)
- Three MCP-native reference providers that we built and maintain:
  [croniq](https://github.com/nuetzliches/croniq) (scheduling),
  [hookaido](https://github.com/nuetzliches/hookaido) (events),
  [powerbrain](https://github.com/nuetzliches/powerbrain) (knowledge)

This is early. We're publishing the thinking, not the product. If the problem
resonates, the spec comes next.

GitHub: https://github.com/nuetzliches/composit

---

## Key Talking Points (for comments)

**"How is this different from Backstage / Port?"**
Backstage is a developer portal for platform teams. Composit is creator control
for agent-generated ecosystems. Backstage catalogs services humans built.
Composit tracks what agents build on your behalf — including things you didn't
explicitly ask for.

**"How is this different from Terraform?"**
Terraform provisions infrastructure. Composit observes it. The Compositfile is
not "apply this state" — it's "here's what exists and why." Think `terraform
state list` elevated to a product, with business-case attribution and cost
tracking.

**"Why not just use Datadog / Grafana?"**
Observability tools monitor health of infrastructure. Composit tracks the
creator's understanding of their ecosystem. "Is this service healthy?" vs.
"Why does this service exist, who created it, and what happens if I remove it?"

**"Why would an agent use composit instead of just building directly?"**
It wouldn't — and that's the point. Agents keep building fast. Composit is
not in the agent's hot path. It observes what providers report and gives the
creator a consolidated view. The agent doesn't need to change.

**"Isn't this just a CMDB?"**
CMDBs are where information goes to die. They require manual maintenance and
are always out of date. Composit auto-populates from MCP-native providers —
the providers report state, composit aggregates it. The Compositfile is the
creator's intent; reality is tracked automatically.

**"MCP lock-in?"**
MCP is an open protocol (by Anthropic). Composit builds on MCP because it's
what agents already speak. If a better agent-tool protocol emerges, composit
adapts. The spec is protocol-aware, not protocol-locked.
