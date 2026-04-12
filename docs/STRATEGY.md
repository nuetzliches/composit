# Composit — Strategy Notes

## Red Thread

The core problem is not infrastructure composition — it's **creator control**.

Agents generate solutions faster than humans can track them. The creator needs
a way to maintain oversight: what exists, why, what it costs, and what needs
attention. Everything else (spec, CLI, registry) serves this purpose.

---

## Why Agents Don't Adopt "Proper" Infrastructure

An agent solving "send me open PRs every morning" will write 15 lines of
`node-cron`, not deploy croniq. This is correct behavior — the agent optimizes
for shortest path to solution, not best infrastructure.

The problem emerges through **accumulation**: 40 throwaway solutions, each in
its own container, each with its own retry logic (or none), each invisible to
the creator. This is an entropy problem, not an infrastructure problem.

The answer is not teaching agents to prefer heavyweight tools. The answer is
that agents build **into** an ecosystem that composit provides — registering a
job in croniq via MCP is lighter than building a custom scheduler, once the
platform exists.

**"The agent builds lighter because the platform is allowed to be heavy."**

---

## "Which Tool?" Includes "Which Company?"

Today's agent flow:
```
Agent needs capability → Agent knows tools → Agent uses tool
```

What's missing:
```
Agent needs capability → Who offers this? → On what terms?
  → Does it fit my policies? → Then use it
```

This is a marketplace problem without a marketplace UI. The agent IS the UI.
It queries the composit manifest, matches against creator policies, and
suggests — or provisions directly.

The composit manifest is like a `package.json` for infrastructure relationships.
Not "which npm packages do I use" but "which provider capabilities are part of
my ecosystem, and under what terms."

---

## Capability Categories

Framed from the creator's perspective — "what do I need control over?":

### Covered by existing nuetzliches projects:
- **Scheduling** → croniq (when does what run?)
- **Events** → hookaido (what triggers what?)
- **Knowledge** → powerbrain (what do agents know?)

### Composit-native (not separate projects):
- **State** — inventory of where data lives across the ecosystem.
  Not a storage layer. Metadata + topology tracking.
  Knowledge (powerbrain) is a subset: curated read-access for agents.
  State is the creator's view of ALL data, including agent-generated DBs,
  files, caches that powerbrain doesn't manage.
- **Cost** — metering across providers. The feature that makes composit
  a business tool, not a nerd tool. "Your agents provisioned 47 services
  this month, estimated cost: X EUR."

### Gap — needs strategic decision:
- **Identity** — who/what is allowed to do what. Authentik exists in the
  infrastructure (nuts-infra) but is not MCP-native. Decision needed:
  build a composit-native identity layer, or integrate Authentik via adapter?
  Apply the "funktioniert neu besser?" test.

---

## Spec vs. Product: Hybrid Model

**Spec (open source, always):**
- Compositfile format specification
- Manifest Discovery protocol
- Contract Trust protocol  
- Policy Interface schema (OPA-compatible)

**Product (open core):**

| Open Source (composit-core)     | Commercial (composit-cloud)        |
|---------------------------------|------------------------------------|
| CLI: parse, validate, diff      | Multi-creator workspaces           |
| Local provider integrations     | Managed manifest registry          |
| Self-hosted, single-creator     | Contract management                |
| Manifest Discovery (static)     | Cost aggregation + alerting        |
|                                 | Audit trail + compliance           |
|                                 | Hosted manifests with SLA          |

croniq, hookaido, powerbrain remain independent OS projects.
They are composit providers, not composit dependencies.

---

## Minimum Viable Spec

Three things a third party needs to build a composit provider:

1. **Capability Declaration** — "I offer scheduling, via MCP, with these tools."
   The public manifest. Machine-readable, versioned, statically hostable.

2. **Trust Handshake** — "I trust you, you trust me, here are the terms."
   API key, mTLS, or token exchange. The contract protocol.

3. **Policy Interface** — "The creator has rules you must respect."
   Not the rules themselves (those stay with the creator), but the interface
   through which a provider says "I accept policy checks" or "I deliver
   these audit data points."

Everything beyond this (CLI, dashboard, agent SDK) is product, not spec.

---

## Prior Art

The pattern (open spec + product to bootstrap) has precedent:
- MCP: Anthropic wrote the spec AND built the first implementation in Claude
- Docker: pushed the container spec AND shipped the product
- Terraform: HCL spec + product
- Kubernetes: CNCF spec + Google's reference implementation

Pure specs without products are PDFs. Specs with reference implementations
that solve real problems attract adoption.
