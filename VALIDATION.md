# Composit — Business Idea Validation Report

**Date:** 2026-04-12
**Method:** Business Model Canvas + Mom Test + Live Market Research
**Score:** 6.5/10 — ITERATE with high urgency

---

## 1. Executive Summary

**Composit adressiert ein reales und wachsendes Problem**, aber der Markt bewegt sich schnell und von oben. Grosse Player (AWS Agent Registry, Microsoft Agent Governance Toolkit, Zenity, StackGen) greifen das Problem bereits an — allerdings primaer fuer Enterprises und aus Security-Perspektive. Composit's Differenzierung (open spec, creator-centric statt enterprise-centric, MCP-native) ist potenziell wertvoll, aber das Zeitfenster ist eng. Die staerkste Chance liegt im Open-Source-Community-Play als "OpenTelemetry fuer Agent-Oekosysteme" — wenn es schnell genug kommt.

---

## 2. Idea Extract

- **Idea Summary:** Composit is an open specification + open-core product that gives creators visibility and control over the services, data, and relationships that AI agents build on their behalf. It provides a declarative format (Compositfile), discovery protocols, and policy enforcement so humans can see and govern their agent-generated ecosystems.

- **Target User Persona:** Technical creators and developers who actively use AI coding agents (Claude Code, Cursor, Copilot Workspace, custom agents) that autonomously provision infrastructure — cron jobs, webhooks, databases, APIs. They're productive but increasingly anxious about losing track of what exists, what it costs, and what breaks.

- **Problem Statement:** When AI agents autonomously provision services and infrastructure, creators lose visibility and control over their growing ecosystem of micro-services, dependencies, and costs.

---

## 3. Key Insights

### Problem Reality: REAL — High Intensity

The evidence is strong:

- **223 Shadow-AI-Incidents per month** on average at enterprises; top quartile: over 2,100/month
- **Only 25% of CIOs** have full visibility into all agents in production
- **48.9% of organizations** are completely blind to machine-to-machine traffic
- **88% of organizations** reported confirmed or suspected AI agent security incidents
- Real cost incidents: 127 forgotten test databases at $180/month each ($22,860/month wasted); developers with $500-2,000/month agent API costs
- MCP ecosystem exploding: **6,400+ registered servers**, 97M monthly SDK downloads
- "Shadow MCP" as new risk: unapproved MCP servers maintaining connections to internal tools outside governance
- 4+ competing token-tracking tools (Splitrail, VibeUsage, Agent Stats, Coding Agent Usage Tracker) emerged in months — strong market signal, but all stop at LLM costs, none track provisioned infrastructure
- Claude Code's own docs build in auto-expiry after 7 days — an implicit admission the problem exists

### User Behavior: What are people doing today?

- **Manual Notion/Spreadsheet inventories** (outdated within days)
- **AWS Config + Custom Scripts** for drift detection (~4h/week for platform engineers)
- **`grep` through agent conversation logs** (indie hackers)
- **Cloud dashboard hopping** across multiple providers
- **Nothing** — many only notice the problem at the next bill or outage

### Market Gaps: Where do existing solutions fail?

1. **No solution is creator-centric** — all target enterprise security/compliance teams
2. **No open spec** — proprietary platforms, no interoperable format
3. **No MCP-native approach** — existing tools treat MCP as one of many vectors
4. **No bottom-up developer tool** — everything is top-down enterprise SaaS
5. **No business case tracking** — existing tools see resources, not *why* they exist

### Important Nuance

The loudest complaints currently come from the **enterprise side** (shadow AI, governance) and from **developers about token costs**. The specific Composit use case — "what infrastructure did my agent build?" — is **not yet grassroots-driven**, but rather vendor-driven. This suggests the problem is **emerging but not yet at critical mass** for indie developers.

---

## 4. Business Model Canvas

| Block | Status | Details |
|-------|--------|---------|
| **Customer Segments** | Assumption | **Primary:** Solo-devs & small teams with AI agents (Claude Code, Cursor). **Secondary:** Platform engineers at AI-native startups. **Tertiary:** Compliance-driven mid-size companies. Risk: Primary segment has little budget. |
| **Value Proposition** | Assumption | "See & control your agent-generated ecosystem." Differentiation: Open Spec, creator-centric, MCP-native, bottom-up not top-down. |
| **Channels** | Assumption | GitHub Stars -> HN Launch -> Dev Community (Discord, Reddit) -> Conference Talks -> Content Marketing. The Docker/Terraform playbook. |
| **Customer Relationships** | Validated | Self-serve OSS -> Community -> Enterprise Sales. Proven pattern. |
| **Revenue Streams** | Assumption | Free CLI + Paid Cloud ($29-99/creator/mo teams, $199+/mo enterprise). Compliance/Audit features as upsell. |
| **Key Resources** | Assumption | Spec design expertise, CLI engineering, MCP ecosystem knowledge, community. Existing: croniq, hookaido, powerbrain as reference providers. |
| **Key Activities** | Assumption | Finalize spec, build CLI, grow community, onboard providers, integrate with AI agent platforms. |
| **Key Partners** | Assumption | Anthropic (Claude Code), MCP ecosystem, OPA community, cloud provider APIs. |
| **Cost Structure** | Low Risk | Primarily engineering labor. Infra minimal. OSS reduces marketing costs. Main risk: time investment before revenue. |

**Canvas Assessment:** 7 of 9 blocks are assumptions. The business model pattern (open-core) is proven, but **customer segment specificity** and **willingness to pay** are the critical unknowns.

---

## 5. Problem Validation (Mom Test Interviews)

### Interview 1: "Marcus" — Freelance AI-Augmented Developer

Marcus uses Claude Code daily for client projects. Over 3 months, his agents set up 14 cron jobs across 3 VPS instances, several webhook endpoints, and two Supabase projects. He discovered the sprawl when a $47 Supabase bill arrived — he'd forgotten an agent had provisioned a database for a test.

- **Current workaround:** A Notion page where he manually logs what agents create. Updated "when he remembers." Admits it's often weeks behind.
- **Money spent on problem:** ~2h/month reconciling cloud bills + the surprise costs themselves ($50-150/month in forgotten services).
- **Emotional intensity:** Medium-high. "It's not an emergency, but I have this constant low-grade anxiety that something's running I forgot about."
- **Would he pay?** Behavior suggests yes — he already spends time on manual tracking. But the amount is unclear. A $10-20/mo tool, maybe. He'd try a free CLI first.

### Interview 2: "Sarah" — Platform Engineer at AI-Native Startup (15 people)

Sarah's team adopted Claude Code and Cursor company-wide. Within 6 weeks, developers had agents spin up Lambda functions, SQS queues, and DynamoDB tables that weren't in Terraform state. She now runs weekly "drift audits" comparing AWS resource lists to their IaC.

- **Current workaround:** AWS Config + custom scripts to detect unmanaged resources. Takes ~4h/week of her time.
- **Money spent:** Her salary time ($300+/week on this task) + AWS Config costs.
- **Emotional intensity:** High. "I'm the one who gets paged at 3am when something breaks, and I can't fix what I don't know exists."
- **Would she pay?** Her company already pays for Backstage and Datadog. A tool that specifically tracks agent-created resources would fill a real gap — if it integrates with their existing stack.

### Interview 3: "Tomas" — Solo Maker / Indie Hacker

Tomas builds SaaS micro-products using AI agents. He has 8 small products running. He thinks he has "maybe 30?" services total but isn't sure. Uses Hetzner, Cloudflare Workers, and Supabase.

- **Current workaround:** `grep -r` through agent conversation logs. Checks cloud dashboards manually.
- **Emotional intensity:** Low-medium. "Honestly it works fine until something breaks. Then I spend a whole day figuring out what connects to what."
- **Would he pay?** Unlikely for a SaaS. Might use a free CLI tool. "If it was a single command that showed me everything, I'd use it daily."

### Interview 4: "Priya" — Engineering Manager, Mid-Size Company

Priya's team of 12 engineers uses AI agents. She's worried about compliance (SOC2 audit coming) — auditors will ask "what services process customer data?" and she can't fully answer because agents provisioned some services outside the IaC pipeline.

- **Current workaround:** Manual inventory spreadsheet + asking engineers to self-report what their agents created.
- **Emotional intensity:** High. The compliance deadline creates urgency.
- **Would she pay?** Yes — compliance tooling budgets exist. $200-500/mo is reasonable if it solves the audit problem.

### Interview Summary

| Signal | Finding |
|--------|---------|
| Problem is real | Yes — across all personas, agent sprawl creates confusion |
| Current spend on workarounds | $0-300+/week depending on role and scale |
| Emotional driver | Anxiety (solo devs), operational burden (platform eng), compliance fear (managers) |
| Willingness to pay | Split: solo devs want free/cheap tools, enterprises would pay for compliance |
| Key insight | The compliance/audit angle may be the strongest monetization path |

**Note:** These are simulated interviews based on real-world behavioral patterns from market research, not actual conversations. Real interviews are a critical next step.

---

## 6. Market Research

### Track A: Pain Points

| # | Pain Point | Frequency | Intensity |
|---|-----------|-----------|-----------|
| 1 | **No visibility into agent-created resources** | Very high | HIGH |
| 2 | **Unexpected costs from forgotten agent services** | High | HIGH |
| 3 | **Shadow MCP / Shadow AI from unapproved agent actions** | High | HIGH |
| 4 | **Compliance risk — auditors ask what exists** | Medium | MEDIUM |
| 5 | **No central inventory of agent ecosystem** | Medium | MEDIUM |

### Track B: Competitor Landscape

#### Developer Portals / Service Catalogs

| Competitor | Pricing | Target | Key Strength | Key Weakness (vs Composit) |
|-----------|---------|--------|-------------|---------------------------|
| **Backstage** (Spotify, OSS) | Free (heavy eng investment) | Large eng orgs (500+) | Massive ecosystem, CNCF graduated | Zero awareness of agent-created resources |
| **Port** ($800M valuation) | From $30/seat/mo | Mid-to-large eng orgs | $100M raise, "AI agent command center", MCP-compatible | Focused on dev self-service, not tracking what agents create |
| **Cortex** | Free tier; ~$25/dev/mo paid | Mid-market eng teams | Service ownership + scorecards | No agent-created resource tracking |
| **OpsLevel** | Per-seat, custom | Enterprise platform teams | MCP server for metadata, AI catalog enrichment | Catalog assumes human operators |

#### AI-Powered Infrastructure Management

| Competitor | Pricing | Target | Key Strength | Key Weakness (vs Composit) |
|-----------|---------|--------|-------------|---------------------------|
| **StackGen** | Per-resource + $15-60/user/mo | Enterprise platform teams | 7 specialized agents, multi-toolchain | IS the agent, not a governance layer |
| **Pulumi Neo** | Pulumi Cloud pricing | IaC-native teams | Deep state management, drift detection | Only tracks Pulumi-managed state |
| **Terraform/OpenTofu** | Free to enterprise | Broad IaC users | Industry standard, massive ecosystem | Per-workspace silos, no agent attribution |

#### AI Agent Governance / Security

| Competitor | Pricing | Target | Key Strength | Key Weakness (vs Composit) |
|-----------|---------|--------|-------------|---------------------------|
| **Zenity** | Enterprise custom | Fortune 500 security teams | Full-lifecycle agent security | Security-focused, not infra catalog |
| **Credo AI** | Enterprise custom | GRC/compliance teams | Regulatory compliance (EU AI Act, NIST) | Governs models/apps, not their infra |
| **AvePoint AgentPulse** | Enterprise pricing | M365/Google orgs | Agent registry + cost control | Scoped to M365/Google only |
| **Microsoft Agent Governance Toolkit** | Free (MIT) | Security-conscious builders | All 10 OWASP risks, multi-language | Runtime security only, no inventory |
| **Ceros (Beyond Identity)** | Public preview, free tier | Claude Code users | MCP visibility, runtime policies, audit | Enterprise-focused |

#### Hyperscalers

| Competitor | Status | Key Weakness (vs Composit) |
|-----------|--------|---------------------------|
| **AWS Agent Registry** (Bedrock AgentCore) | Preview April 2026 | AWS-only, no open spec |
| **Microsoft Agent 365** | Announced Ignite 2025 | Microsoft ecosystem lock-in |

**Critical observation:** No competitor combines: Open Spec + Creator-centric + MCP-native + Business-Case Tracking. But the market is converging fast.

#### Five Clear Market Gaps — All in Composit's Scope

| Gap | Status |
|-----|--------|
| **Attribution:** Who/what created this resource and why? | Nobody solves this |
| **Cross-Agent Inventory:** One catalog across all provisioning methods | Nobody solves this |
| **Cost Attribution:** Costs per agent session/agent type | Nobody solves this |
| **Pre-Provisioning Policy:** What are agents allowed to create? | Partially MS Toolkit |
| **Open Spec for Agent-Resource Metadata** | Nobody — OpenTelemetry analogy |

### Track C: Demand Signals

| Signal | Strength | Evidence |
|--------|----------|---------|
| **Category has a name** | HIGH | Forrester formally evaluating "Agent Control Plane" market NOW (April 2026) |
| **Market size** | HIGH | Autonomous AI agent market: $8.5B by 2026, $35B by 2030 |
| **VC activity** | HIGH | $2.66B in agentic AI funding Q1 2026 alone (142.6% rise vs 2025) |
| **Active communities** | HIGH | HN posts on "Infrastructure Gap in Agentic AI", MCP sprawl discussions, Reddit threads on agent costs |
| **Conference activity** | HIGH | AI Agent Conference 2026 (NYC), LangChain Interrupt 2026 (SF), RSAC 2026 agent security tracks |
| **Search interest** | MEDIUM | "AI agent governance", "MCP registry" growing; "composit" as term not yet existent |
| **Willingness to pay** | MEDIUM | Enterprise: yes (compliance budgets). Solo devs: primarily free/OSS. Gap in the middle. |
| **MCP ecosystem growth** | HIGH | 6,400+ servers, 97M downloads/month, 2026 roadmap includes .well-known discovery |
| **Ad presence** | LOW | Content marketing stage, not paid acquisition — consistent with forming market |

**Timing Assessment: Now — but the window is closing.**

The problem is real today. MCP ecosystem is growing exponentially. But large players are already moving. Forrester explicitly flags three standards gaps: incomplete instrumentation, absent portable agent identity, and missing cross-plane governance schemas. Whoever fills these with an open spec wins the Docker-equivalent position.

In 12-18 months, AWS/Microsoft could offer native solutions. The next 6-12 months determine whether hyperscalers lock in proprietary approaches or an open standard emerges.

---

## 7. Signal Scoring

### Strong Signals

- People already paying for inferior alternatives (manual tracking, custom scripts, AWS Config)
- High emotional intensity: anxiety (solo devs), operational burden (platform eng), compliance fear (managers)
- Repeated, specific complaints across multiple sources (HN, Reddit, vendor content, analyst reports)
- Growing search interest and community activity
- VC funding at record levels in adjacent space
- Forrester creating formal market evaluation = procurement budgets will follow

### Weak Signals

- Infrastructure sprawl complaints are more enterprise-vendor-driven than grassroots-developer-driven
- Solo developers may not pay for dedicated tooling
- "Agent-created infrastructure tracking" specifically has few first-person developer stories

### Red Flags

- Microsoft Agent Governance Toolkit (MIT license) could become de facto standard
- Port ($800M, $100M fresh capital) could pivot into this exact space
- Hyperscalers (AWS, Microsoft) shipping competing features natively
- Open spec adoption requires critical mass — without early adopters among agent platforms, it stays niche

---

## 8. Synthesis and Verdict

**Problem Strength: High**
The problem is real, growing, and measurable in dollars and time. 88% of organizations report AI agent security incidents. Only 25% have full visibility. Cost incidents are documented and visceral.

**Solution Fit: Clear**
Composit addresses all 5 identified market gaps directly. The open-spec approach is differentiated and strategically valuable.

**Market Readiness: Now**
Forrester is evaluating the category. AWS and Microsoft shipped competing products. $2.66B in Q1 2026 funding. Timing is right but urgent.

---

## 9. Validation Score: 6.5 / 10

Mixed-to-strong signals. The problem is validated; execution speed and positioning determine success.

| Dimension | Score | Rationale |
|-----------|-------|-----------|
| Problem Strength | 9/10 | Clearly real, growing, measurable in $ and time |
| Solution Fit | 7/10 | Good approach, addresses all identified gaps |
| Market Readiness | 7/10 | Forrester evaluation = market forming actively |
| Competitive Pressure | 7/10 | No direct competitor yet, but Port ($800M) + MS Toolkit moving |
| Monetization | 4/10 | Primary segment (creators) has little budget; enterprise needs sales |
| Differentiation | 7/10 | Only Open Spec + Creator-centric + MCP-native approach |
| Urgency | 9/10 | 6-12 month window for open standard position |

---

## 10. Recommendation: ITERATE -> BUILD in 4 Weeks

The core idea is strong, but the go-to-market strategy needs sharpening. The problem is validated — the question is not *whether* but *how* and *for whom exactly*.

**Why ITERATE, not BUILD:** Spec-first without code ("no code yet") is risky in a market moving this fast. Until the spec is ready, established players could close the gap.

**Why not DROP:** The problem is too real and the differentiation (open spec, creator-centric, MCP-native) too unique.

**Key strategic shift:** The research shows that the Open-Spec position is the most valuable asset — not the CLI product alone. The analogy is OpenTelemetry, not Docker.

---

## 11. Next Actions

### Immediate (Week 1-2)

1. **Publish Draft Spec** — Compositfile format + Manifest Discovery as RFC on GitHub. Does not need to be perfect, needs to exist. Gather community feedback.

2. **Segment Decision.** Who pays first?
   - **Option A:** Platform engineers at AI-native startups (5-50 people). Budget exists, pain is acute, compliance pressure coming.
   - **Option B:** MCP power users / indie hackers. No budget, but community growth -> OSS adoption -> enterprise pull.
   - Recommendation: **Option B as entry, Option A as monetization.**

3. **5 Real User Interviews** (Mom Test). Talk to:
   - 2 Claude Code heavy users (r/ClaudeAI, Claude Discord)
   - 2 Platform engineers at AI-native startups
   - 1 Compliance/security person managing SOC2/GDPR with AI agents

### Short-term (Week 3-4)

4. **`composit scan` CLI** — reads local MCP configs, cloud provider APIs, shows ecosystem inventory. Proof-of-concept that the spec works. Comparable to `docker ps` for agent ecosystems.

5. **Landing Page + Waitlist** with the question: "How many agent-created services are running in your infrastructure right now?" — Response distribution = demand signal.

### Medium-term (Week 5-8)

6. **First External Provider Integration** (not just own croniq/hookaido/powerbrain). Demonstrate the spec is vendor-neutral.

7. **HN Launch** with working CLI + draft spec.

8. **Pricing Validation.** Test three price points:
   - Free: CLI + local scan
   - $29/mo: Team dashboard + drift alerts
   - $199/mo: Compliance reporting + audit trail

### Ongoing

9. **Monitor Port closely.** $800M valuation, $100M fresh capital, explicit "agents as first-class citizens" strategy. If they pivot harder into agent-created resource tracking, they have distribution and capital to dominate.

10. **Monitor MCP 2026 Roadmap.** Includes `.well-known` discovery and enterprise features. Composit should be complementary, not competing.

11. **Monitor Microsoft Agent Governance Toolkit.** If it moves to a foundation and gains community traction, it could become the de facto standard. Options: position against OR build on top.

---

## 12. Risks and Assumptions

### Most Dangerous Assumptions

1. **"Creators" are a paying segment.** Solo devs and small teams have little budget for tooling. Revenue likely comes from team/enterprise segment first.

2. **Open spec gets adopted.** A standard only works with critical mass. Without early adopters among agent platforms (Anthropic, Cursor), the spec stays niche.

3. **MCP remains the dominant agent-tool protocol.** Composit builds on MCP. If a competing protocol wins, relevance shrinks.

4. **Bottom-up beats top-down.** Enterprise competitors (Zenity, AWS) have sales teams and budget. The Docker playbook works, but not always.

### What would need to be true for this to fail?

- MCP ecosystem stagnates or is displaced by proprietary alternatives
- AWS/Microsoft offer native ecosystem visibility for free
- Developers solve the problem with simple scripts and don't need a dedicated tool
- No agent platform partner (Anthropic, Cursor) integrates Composit

### What could be wrong about this analysis?

- Simulated interviews are hypotheses, not real conversations
- Enterprise demand could be much larger than assumed (compliance pressure)
- The open-spec approach could create its own category that competitors don't serve

---

## Sources

### Pain Points & Market Data
- Beam.ai: AI Agent Sprawl as Shadow IT
- NETSCOUT: Shadow AI Creates Zombie Infrastructure
- InformationWeek: Controlling AI Agent Costs
- Reco.ai: AI Agent Sprawl Statistics
- The New Stack: Hidden Agentic Technical Debt
- DEV Community: "You're Already in AI Control Debt"

### Competitors
- StackGen: Autonomous Infrastructure Platform (stackgen.com)
- Port: $100M raise for AI agent hub (siliconangle.com)
- Zenity: AI Agent Security Platform (zenity.io)
- Credo AI: AI Governance (credo.ai)
- AvePoint AgentPulse: Agentic AI Governance
- Microsoft Agent Governance Toolkit (github.com/microsoft/agent-governance-toolkit)
- Ceros by Beyond Identity: AI Trust Layer
- OpsLevel: AI-powered service catalog
- MCP Registry (registry.modelcontextprotocol.io)

### Demand Signals
- Forrester: Agent Control Plane Market Evaluation (April 2026)
- Deloitte: AI Agent Orchestration Predictions
- Crunchbase: Q1 2026 Record-Breaking AI Funding
- Tracxn: Agentic AI Funding Data
- Gartner: Predicts 2026 — AI Agents Reshape I&O
- MCP 2026 Roadmap (blog.modelcontextprotocol.io)
- AI Agent Conference 2026 (agentconference.com)

### Cost Incidents
- Wisp CMS: AI Bot Unexpected Billing Case Study
- MindStudio: AI Token Management / Budget Management
- Anthropic/The Register: Claude Code Usage Limits
- Hacker News: Claude Code Users Hitting Limits

---

*Generated 2026-04-12 via Business Idea Validation Skill (Claude Code)*
