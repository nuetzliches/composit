# Composit — Next Steps

Stand: 2026-04-12

---

## Phase 1: Konzept schärfen (vor Code)

- [ ] **ICP definieren** — Wer fühlt den Schmerz zuerst? Kandidaten: Solo-Founder mit 5-15 Agent-Services, Agentur-CTOs, AI-forward Startups (5-20 Devs). Einen primären ICP auswählen.
- [ ] **"Silent Ecosystem Failure" als Kernnarrative** — README und HN-LAUNCH umschreiben. Weg von "agents build fast", hin zu "your business cases degrade silently."
- [ ] **Compositfile-Design-Entscheidung** — Auto-generiert (composit beobachtet Provider) vs. hand-geschrieben (Creator deklariert). Vermutlich hybrid: composit beobachtet, Creator annotiert.
- [ ] **Companion-Model-Framing** — In STRATEGY.md explizit machen: composit ist die Antwort auf die offene Frage aus croniq/hookaido-Validierung.
- [ ] **Monetarisierungs-These** — Composit als Enterprise-Layer dokumentieren. RBAC, Audit, Cost-Tracking leben in composit, nicht in den Einzelprojekten.

## Phase 2: Spec entwerfen

- [ ] **Manifest-Schema finalisieren** — composit.json v0.1 spezifizieren (Felder, Versionierung, Discovery-Mechanismus)
- [ ] **Policy Interface definieren** — Minimales OPA-Interface, das ein Provider implementieren muss
- [ ] **Contract Protocol skizzieren** — Auth-Flow (API-Key, mTLS, Token-Exchange), was nach Vertragsschluss exponiert wird
- [ ] **Compositfile-Format** — Basierend auf Design-Entscheidung aus Phase 1

## Phase 3: Proof of Concept

- [ ] **composit CLI** — `composit status` zeigt aggregierten Zustand von croniq + hookaido + powerbrain via MCP
- [ ] **Live-Demo mit nuts-infra** — int.baumeister als erstes reales Composit-Workspace
- [ ] **Sprach-Entscheidung** — Rust, Go, oder TypeScript für composit-core

## Phase 4: Validieren

- [ ] **Show HN** — Konzept + CLI-Demo
- [ ] **5 echte Gespräche** — Creators/CTOs die Agent-generierte Infrastruktur betreiben
- [ ] **croniq/hookaido/powerbrain READMEs** — Composit-Referenz ergänzen (nach positivem Signal)
