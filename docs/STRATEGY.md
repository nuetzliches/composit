# Composit — Strategy Notes

## Red Thread

The core problem is **governance over infrastructure — IS-state vs. SHOULD-state.**

Infrastructure governance was broken before AI. Most teams don't have a
machine-readable answer to "what do we run, and does it match what we declared?"
AI agents make this urgent: they provision infrastructure faster than humans can
track, outside any governance framework.

Composit is Governance-as-Code:
- `composit scan` → **IS-state** (what exists)
- `Compositfile` → **SHOULD-state** (what's declared: providers, budgets, policies)
- `composit diff` → **drift** (the gap between reality and governance)

AI-attribution (who created what: agent vs. human) is a **reporting feature**,
not the core. The IS-vs-SHOULD comparison works regardless of who created what.
But the AI angle makes governance urgent and marketable — "every company will
use AI; the question is how they keep control."

The word "creator" remains in the spec context (the person who owns the
Compositfile governance rules and reviews composit reports). The business
narrative leads with **platform teams and CTOs** — the people who feel
governance pain most acutely and have budget to solve it.

---

## ICP — Wer zahlt?

Validation (7.5/10, BUILD — April 2026, re-validated 2026-04-14) hat die
Zielgruppen-Hierarchie geklärt:

### Primary Paying ICP: Platform Engineers / DevOps

- Frustration: 5/5 (höchste aller Segmente)
- Bestehender Budget-Kontext: Backstage ($375K-$750K/yr TCO), Datadog, IaC-Tools
- Konkreter Pain: Drift-Audits (4h+/Woche), Agent-generierte Ressourcen außerhalb
  des IaC-Pipelines, "terraform destroy auf Prod" durch AI-Agents
- Compliance-Druck (SOC2, GDPR, EU AI Act Aug 2026) schafft Procurement-Urgency
- **Sie sind die Buyer.** Product-Roadmap optimiert für diese Persona.

### Secondary Paying ICP: CTOs / Engineering Managers

- Frustration: 4/5
- Teams von 5-50 Devs, AI-nativ
- Pain: "Was hat der Agent gemacht? Welche Permissions hat er? Was kostet das?"
- Budget-Rahmen: $200-500/mo für Tooling
- Compliance-Deadline-Druck (EU AI Act) macht es zum Must-Have statt Nice-to-Have

### Community ICP (Free Tier): Solo Devs / Indie Hackers

- Frustration: 3/5 — sie fühlen den Pain, aber zahlen nicht dafür
- Price-sensitive, suchen eher "besseres Agent-Verhalten" als Governance-Tools
- **Wert für composit:** GitHub-Stars, HN-Traction, Word-of-Mouth, Spec-Adoption
- Die Free CLI muss sie gut bedienen. Aber die Produkt-Roadmap wird nicht für
  sie optimiert.

### Konsequenz für die Narrative

"Creator Control" → "Agent Infrastructure Visibility for Platform Teams"

Die README und HN-Launch sprechen Platform Engineers und CTOs an.
Solo Devs finden composit über die Open-Source-CLI und Community-Channels.

---

## Validation Findings (Kurzfassung)

**Score: 7.5/10 — BUILD** (April 2026, re-validated 2026-04-14)

**Key market numbers (from initial validation, April 2026):**
- 223 shadow-AI incidents/month on average at enterprises; top quartile: 2,100+/month
- Only 25% of CIOs have full visibility into agents in production
- 48.9% of organizations completely blind to machine-to-machine traffic
- 88% of organizations reported confirmed or suspected AI agent security incidents
- 78% of IT leaders report unexpected costs from AI agent usage

| Dimension           | Score | Signal  | Notiz |
|---------------------|-------|---------|-------|
| Problem Severity    | 8     | Stark   | 78% Shadow AI, Drift #1 Pain (HashiCorp Survey), 35 CVEs aus AI-Code (März 2026) |
| Existing Spending   | 7     | Stark   | $900M+ in adjacent Tools 2025. Spacelift $250/mo, Backstage TCO $375-750K/yr |
| Market Momentum     | 9     | Stark   | IaC 28% CAGR, Platform Eng Budgets verdoppeln sich 2026, FinOps #1 = Governance |
| Competitive Gap     | 8     | Stark   | Kein Tool kombiniert Cross-Tool Scanning + Governance Spec + AI Attribution |
| Monetization Clarity| 5     | Mittel  | Klares Freemium-Modell, aber CLI-Monetarisierung unbewiesen. Keine zahlenden Kunden |

Score-Änderung vs. Erstvalidierung (7.0 → 7.5): Problem Severity und Existing
Spending höher bewertet basierend auf aktualisierter Marktforschung (IBM 2025
Breach Report, FinOps 2026 Report, Spacelift/Firefly Traction). Monetization
Clarity bleibt konservativ bei 5 — keine reale Zahlungsbereitschaft validiert.

**Stärken:** Problem dokumentiert und quantifiziert. $900M+ Funding in adjacent
Space bestätigt Kategorie. Kein Produkt besetzt composit's Position (Cross-Tool
Governance-as-Code). Market Momentum explosiv.

**Schwächen:** Grassroots-Demand dünn. Budget existiert in adjacent Categories,
nicht spezifisch für Governance-as-Code. Keine realen User-Interviews durchgeführt.
Solo-Founder-Execution-Risk bei ambitioniertem 8-Wochen-Plan.

**Zeitfenster:** 6-12 Monate bevor Port ($800M) oder Hyperscaler die Position
mit proprietary Lösungen besetzen.

→ Vollständiger Report: `nuts-skills/results/composit/validation/2026-04-14_composit-governance-as-code.html`

---

## Feature vs. Product Risk

Das größte strategische Risiko: composit wird ein Feature in einem größeren
Produkt, bevor es als eigenständiges Produkt Traktion bekommt.

### Risiko 1: IDPs adden Agent-Discovery

Port ($800M Valuation, $100M frisches Kapital) pivotiert explizit Richtung
"agents as first-class citizens". Backstage (~89% IDP-Marktanteil) könnte
Agent-Awareness als Plugin nachliefern.

### Risiko 2: Agent-Plattformen tracken selbst

Claude Code, Cursor, Devin könnten Built-in Infrastructure Tracking liefern.
Das wäre aber immer siloed — Claude Code trackt nur was Claude Code baut.

### Risiko 3: Hyperscaler Lock-in

AWS Agent Registry (Preview), Microsoft Agent Governance Toolkit —
proprietäre Ansätze, die den offenen Standard-Raum besetzen könnten.

### Defense-Strategie

1. **Open Spec als Standard** — OpenTelemetry-Modell statt Docker-Modell.
   Wenn das composit-report-Format oder die Compositfile-Spec adoptiert wird,
   validieren selbst Competitors den Standard. Spec kann perspektivisch in
   eine Foundation wandern.

2. **Heterogenität als Differentiator** — composit trackt über Agents,
   Clouds und On-Prem-Stacks hinweg. Hyperscaler-Angebote werden strukturell
   auf ihr Ökosystem optimiert (AWS + partners first). Der Fit für Teams
   mit gemischten Stacks (lokal + mehrere Clouds + mehrere Agent-Plattformen)
   ist Composits Pflicht-Terrain.

   **Ehrliche Einordnung:** AWS/Microsoft können durchaus 60-70% des
   Enterprise-Markts bedienen. Composits adressierbarer Markt ist
   "heterogene Stacks + Platform-Eng-Teams, die Vendor-Neutralität wollen" —
   nicht "alle".

3. **Geschwindigkeit** — 6-12 Monate Window. Spec draft + Working CLI
   vor den Hyperscalern veröffentlichen. Solo-Founder-Tempo ist Risiko;
   Scope-Disziplin und Dogfooding kompensieren, bis externe Beiträge skalieren.

---

## Why Agents Don't Adopt "Proper" Infrastructure

An agent solving "send me open PRs every morning" will write 15 lines of
`node-cron`, not deploy croniq. This is correct behavior — the agent optimizes
for shortest path to solution, not best infrastructure.

The problem emerges through **accumulation**: 40 throwaway solutions, each in
its own container, each with its own retry logic (or none), each invisible to
the team. This is an entropy problem, not an infrastructure problem.

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
It queries the composit manifest, matches against team policies, and
suggests — or provisions directly.

The composit manifest is like a `package.json` for infrastructure relationships.
Not "which npm packages do I use" but "which provider capabilities are part of
my ecosystem, and under what terms."

---

## Capability Categories

Framed from the platform team's perspective — "what do I need control over?":

### Herleitung

Initial bottom-up aus bestehenden nuetzliches-Projekten (croniq, hookaido,
powerbrain) abgeleitet, dann systematisch gegen CNCF/Cloud-Native Patterns
validiert (OpenTelemetry, FinOps, SPIFFE, Service Catalog). Ergebnis: eine
Category hinzugefügt (Observability), drei bewusst ausgeschlossen.

### Covered by existing nuetzliches projects:
- **Scheduling** → croniq (when does what run?)
- **Events** → hookaido (what triggers what?)
- **Knowledge** → powerbrain (what do agents know?)

### Composit-native (not separate projects):
- **State** — inventory of where data lives across the ecosystem.
  Not a storage layer. Metadata + topology tracking.
  Knowledge (powerbrain) is a subset: curated read-access for agents.
  State is the team's view of ALL data, including agent-generated DBs,
  files, caches that powerbrain doesn't manage.
- **Cost** — metering across providers. The feature that makes composit
  a business tool, not a nerd tool. "Your agents provisioned 47 services
  this month, estimated cost: X EUR."
- **Observability** — what happened, when, and why. Traces, logs, audit trail
  across the agent ecosystem. Distinct from State (State = where data is now;
  Observability = what happened over time). Addresses the "silent ecosystem
  failure" pain point: a scheduling component hasn't run in 3 days, a webhook
  channel has 47 messages in the DLQ — and nobody noticed.

### Gap — needs strategic decision:
- **Identity** — who/what is allowed to do what. Includes secrets management
  (which API keys/tokens do agents use, and are they rotated/scoped correctly?).
  Authentik exists in the infrastructure (nuts-infra) but is not MCP-native.
  Decision needed: build a composit-native identity layer, or integrate
  Authentik via adapter? Apply the "funktioniert neu besser?" test.

### Bewusst nicht included (mit Begründung):
- **Lifecycle/Deployment** — composit ist Control Plane, nicht Deployment Plane.
  State trackt *was* läuft. Wie es deployed wurde, ist Sache des CI/CD-Systems
  oder des Agents selbst. Composit übernimmt keine Helm/ArgoCD-Funktion.
- **Inter-Agent Communication** — Events (hookaido) deckt Event-Routing ab.
  Direkte Agent-to-Agent Communication (A2A-Patterns) ist ein emergentes Feld.
  Beobachten, ob Events als Category ausreicht oder ob A2A eine eigene
  Category erfordert, sobald der Markt reift.
- **Networking/Service Mesh** — composit operiert auf der Capability-Ebene,
  nicht auf der Netzwerk-Ebene. Routing, mTLS, Traffic Shaping sind
  Infrastruktur-Concerns unterhalb von composit.

---

## Spec vs. Product: Hybrid Model

**Spec (open source, always):**
- composit-report.yaml format specification
- Compositfile governance specification (post-MVP)
- Manifest Discovery protocol
- Contract Trust protocol  
- Policy Interface schema (OPA-compatible)

**Product (open core):**

| Open Source (composit-core)     | Commercial (self-hosted or cloud)  |
|---------------------------------|------------------------------------|
| CLI: scan, status, report gen   | Multi-team workspaces           |
| `composit serve` (local dash)   | Managed manifest registry          |
| Local provider integrations     | Contract management                |
| Manifest Discovery (static)     | Cost aggregation + alerting        |
|                                 | Audit trail + compliance           |
|                                 | SSO, SLA, hosted manifests         |

**Deployment options:** Teams choose self-hosted (Docker/Helm) or composit-cloud.
The product is the same — cloud just adds managed infrastructure and SLA.
This mirrors the Backstage/GitLab model: open-core, self-hosted first.

croniq, hookaido, powerbrain remain independent OS projects.
They are composit providers, not composit dependencies.

---

## Minimum Viable Spec

Three things a third party needs to build a composit provider:

1. **Capability Declaration** — "I offer scheduling, via MCP, with these tools."
   The public manifest. Machine-readable, versioned, statically hostable.

2. **Trust Handshake** — "I trust you, you trust me, here are the terms."
   API key, mTLS, or token exchange. The contract protocol.

3. **Policy Interface** — "The team has rules you must respect."
   Not the rules themselves (those stay with the team), but the interface
   through which a provider says "I accept policy checks" or "I deliver
   these audit data points."

Everything beyond this (CLI, dashboard, agent SDK) is product, not spec.

---

## Competitive Landscape

### Direkte Wettbewerber / Adjacent Players

| Player              | Was sie tun                        | Composit-Differenzierung              |
|---------------------|------------------------------------|---------------------------------------|
| Backstage/Port      | Developer Portal / Service Catalog | Kein Agent-Awareness, kein Auto-Discovery |
| Terraform/Pulumi    | IaC — deklariert + provisioniert   | Nur declared Resources; blind für ad-hoc Agent-Aktionen |
| Port (getport.io)   | IDP, $800M Valuation               | Pivotiert zu Agents — beobachten. Kein Open Spec. |
| Gravitee            | MCP Proxy, Agent-level IAM         | Traffic/Security-Fokus, nicht "was wurde gebaut" |
| Pillar Security     | AI Asset Discovery                 | Security-only; kein Cost/Dependency Mapping |
| Cortex.io           | Service Catalog, Scorecards        | Teuer ($25-65/user/mo), 6+ Monate Deploy, kein AI |
| AWS Agent Registry  | Preview — agent inventory          | Proprietary, AWS-only. Composit ist cloud-agnostic. |
| MS Agent Governance | Enterprise Agent Governance        | Azure-locked, top-down. Composit ist bottom-up + open spec. |

**Niemand kombiniert:** Open Spec + Agent-nativ + MCP-native + Cross-Agent +
Business-Case-Tracking + Cost Attribution.

### Prior Art (Open Spec + Product)

The pattern (open spec + product to bootstrap) has precedent:
- MCP: Anthropic wrote the spec AND built the first implementation in Claude
- Docker: pushed the container spec AND shipped the product
- Terraform: HCL spec + product
- Kubernetes: CNCF spec + Google's reference implementation

Pure specs without products are PDFs. Specs with reference implementations
that solve real problems attract adoption.

---

## Provider Interchangeability

croniq, hookaido, powerbrain are reference providers — they prove the spec
works. But composit's value proposition requires that ANY provider can fill
a capability slot, not just ours.

### What's in place:
- Manifest Discovery: providers publish capabilities at a well-known URL
- Contract Trust Protocol: standardized handshake for trust establishment
- Policy Engine: team controls which providers are approved (provider-approval.rego)

### What's missing for real interchangeability:

1. **Capability Interface Spec** — what MUST a "scheduling" provider support?
   Minimum tool surface, required metadata, expected behavior. Without this,
   "I offer scheduling" is a claim, not a contract.

2. **Conformance Tests** — how does a provider prove it fulfills a capability?
   A test suite that a third-party scheduling provider can run against itself.
   Like Kubernetes conformance tests.

3. **Migration Path** — how does a team switch from provider A to provider B?
   State export, contract transfer, zero-downtime cutover. Without this,
   interchangeability is theoretical.

### Validation milestone:
First external provider integration (not croniq/hookaido/powerbrain) that
demonstrates the spec is vendor-neutral. This is the single strongest
proof point for composit's open-spec story.
