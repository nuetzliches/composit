# Open Questions

Tracked decisions and unresolved questions for the composit project.

---

## Q1: Identity — Build or Integrate?

**Context:** Authentik exists in nuts-infra and handles identity today.
But it's not MCP-native and not built for agent-to-provider trust.

**Options:**
- A) Build a lightweight, MCP-native identity layer (new nuetzliches project)
- B) Integrate Authentik via composit adapter
- C) Make identity a composit-native concern (like State and Cost)

**Test:** "Funktioniert neu besser als der Adapter?"

**Status:** Open.

---

## Q2: Compositfile — Static or Living Document?

**Context:** Is the Compositfile a declaration the creator writes (like Terraform),
or a living document that composit maintains by observing reality?

**Options:**
- A) Static: creator writes it, composit validates reality against it (drift detection)
- B) Dynamic: composit auto-generates it from observed provider state
- C) Hybrid: creator declares intent, composit enriches with observed state

**Leaning:** C — the creator declares business cases and policies, composit fills
in the runtime details (actual jobs, actual costs, actual health).

**Status:** Open.

---

## Q3: Manifest Discovery — DNS, HTTP, or Registry?

**Context:** How does an agent find composit-compatible providers?

**Options:**
- A) Well-known URL: `provider.example/.well-known/composit.json`
- B) DNS TXT records: `_composit.example.com`
- C) Central registry: `registry.composit.dev`
- D) All of the above, with fallback chain

**Status:** Open.

---

## Q4: What Language for composit-core?

**Context:** The existing stack is Rust (croniq), Go (hookaido), Python (powerbrain),
TypeScript (openclaw).

**Considerations:**
- CLI performance → Rust or Go
- MCP ecosystem alignment → TypeScript or Python
- Team expertise → all four are in play
- Composit is primarily a spec/metadata tool, not a high-throughput system

**Status:** Open.

---

## Q5: Composit and OpenClaw — Relationship?

**Context:** OpenClaw is a personal AI assistant (355k stars). Composit is creator
control. They are complementary but distinct.

**Question:** Is composit a feature inside OpenClaw, an OpenClaw skill, or a
standalone project that OpenClaw integrates with?

**Leaning:** Standalone. OpenClaw is one possible agent that speaks composit.
Claude Code is another. Composit must be agent-agnostic.

**Status:** Open.
