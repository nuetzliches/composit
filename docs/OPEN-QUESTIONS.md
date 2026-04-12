# Open Questions

Tracked decisions and unresolved questions for the composit project.
Updated: 2026-04-12 (post-Validation)

---

## Q1: Identity — Build or Integrate?

**Context:** Authentik exists in nuts-infra and handles identity today.
But it's not MCP-native and not built for agent-to-provider trust.

**Options:**
- A) Build a lightweight, MCP-native identity layer (new nuetzliches project)
- B) Integrate Authentik via composit adapter
- C) Make identity a composit-native concern (like State and Cost)

**Test:** "Funktioniert neu besser als der Adapter?"

**Validation Update (2026-04-12):** Identity ist nicht der erste Pain Point.
Die Validation zeigt: Attribution ("wer hat das erstellt?"), Cross-Agent Inventory,
und Cost Attribution sind die dringendsten Lücken. Identity auf post-MVP
deprioritisieren. Für MVP reicht API-Key-basierter Zugang.

**Status:** Deprioritized (post-MVP). Entscheidung erst nach CLI v0.1.

---

## Q2: Compositfile — Static or Living Document?

**Context:** Is the Compositfile a declaration the creator writes (like Terraform),
or a living document that composit maintains by observing reality?

**Options:**
- A) Static: creator writes it, composit validates reality against it (drift detection)
- B) Dynamic: composit auto-generates it from observed provider state
- C) Hybrid: creator declares intent, composit enriches with observed state

**Validation Update (2026-04-12):** Leaning C bestätigt. `composit scan` als
erstes Produkt ist implizit ein "beobachte Realität"-Ansatz. Der Scanner
generiert ein Inventory aus dem was er findet. Creator-Intent (Business Cases,
Policies, Budgets) kommt später als Annotationen über die beobachtete Realität.

**Status:** Decided — Option C (Hybrid). Scanner first, Annotations second.

---

## Q3: Manifest Discovery — DNS, HTTP, or Registry?

**Context:** How does an agent find composit-compatible providers?

**Options:**
- A) Well-known URL: `provider.example/.well-known/composit.json`
- B) DNS TXT records: `_composit.example.com`
- C) Central registry: `registry.composit.dev`
- D) All of the above, with fallback chain

**Validation Update (2026-04-12):** MCP 2026 Roadmap beinhaltet `.well-known`
Discovery nativ. Composit sollte sich daran alignen (Option A) für die
Open-Source-Version. Central Registry (Option C) ist das natürliche
Commercial-Upsell für composit-cloud (managed registry with SLA).

**Status:** Decided — A (well-known URL) für OSS, C (Registry) als Commercial.

---

## Q4: What Language for composit-core?

**Context:** The existing stack is Rust (croniq), Go (hookaido), Python (powerbrain),
TypeScript (openclaw).

**Considerations:**
- CLI performance → Rust or Go
- MCP ecosystem alignment → TypeScript or Python
- Team expertise → all four are in play
- Composit is primarily a spec/metadata tool, not a high-throughput system

**Validation Update (2026-04-12):** Validation zeigt Geschwindigkeit ist
kritischer als Performance. Das 6-12 Monate Window vor Hyperscaler-Lock-in
erfordert schnellste Iteration. TypeScript ist: MCP SDK native, schnellste
Prototyping-Geschwindigkeit, npm-Ecosystem für CLI-Tooling (commander, ora, etc.).
Rust/Go Rewrite später wenn Performance-Bottleneck wird.

**Status:** Decided — TypeScript für v0.1. Revisit nach MVP wenn nötig.

---

## Q5: Composit and OpenClaw — Relationship?

**Context:** OpenClaw is a personal AI assistant. Composit is infrastructure
visibility. They are complementary but distinct.

**Question:** Is composit a feature inside OpenClaw, an OpenClaw skill, or a
standalone project that OpenClaw integrates with?

**Validation Update (2026-04-12):** Standalone bestätigt. Cross-Agent Visibility
ist der zentrale Moat gegen "Feature not Product" Risiko. Wenn composit nur
mit OpenClaw funktioniert, wird es ein Feature. Composit muss mit Claude Code,
Cursor, Devin, und beliebigen anderen Agents funktionieren.

**Status:** Decided — Standalone. OpenClaw is one of many possible agents.
