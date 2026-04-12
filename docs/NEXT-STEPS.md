# Composit — Next Steps

Stand: 2026-04-12 (post-Validation)

Basiert auf Business Validation Report (6.5/10, ITERATE).
Alte 4-Phasen-Roadmap (Konzept > Spec > PoC > Validieren) ersetzt durch
zeitgebundene Sprints mit klarem ICP-Fokus: Platform Engineers + CTOs.

---

## Sprint 1: Validate + Spec Draft (Woche 1-2)

### ICP locked

- [x] **ICP-Entscheidung:** Platform Engineers + CTOs als zahlende Zielgruppe.
  Solo Devs = Community/Adoption Funnel, nicht Produkt-Fokus.
- [ ] **Narrative schärfen** — README, HN-LAUNCH, STRATEGY auf Platform-Eng-Audience
  umschreiben. Weg von "Creator Control", hin zu "Agent Infrastructure Visibility."

### Interviews

- [ ] **10-15 Interviews planen** — Zielgruppe: Platform Engineers und CTOs bei
  Teams die Claude Code, Cursor, oder Devin nutzen (5-50 Devs).
  - Kernfragen: Trackt ihr was Agents erstellen? Würdet ihr für Visibility zahlen?
    Was ist der aktuelle Workaround? Wie viel Zeit geht für Drift-Audits drauf?
  - Kanäle: LinkedIn (DevOps/Platform Eng), CNCF Slack, lokale Meetups, eigenes Netzwerk
- [ ] **Interview-Leitfaden erstellen** — Mom-Test-kompatibel. Keine Leading Questions.
  Fokus auf bestehendes Verhalten, nicht hypothetische Zahlungsbereitschaft.

### Spec

- [ ] **Compositfile Spec v0.1 als RFC** — Formales Schema (JSON Schema oder HCL Grammar),
  nicht nur Beispiele. Auf GitHub als Discussion oder PR veröffentlichen.
- [ ] **Manifest Schema finalisieren** — composit.json v0.1 mit konkreten Feldern,
  Versionierung, Discovery-Mechanismus (.well-known URL, align mit MCP).

---

## Sprint 2: `composit status` CLI (Woche 3-4)

### MVP-Scope

- [ ] **`composit scan`** — Zero-Config Scanner für Agent-generierte Artefakte:
  - Lokale MCP-Configs lesen (claude_desktop_config.json, .cursor/, etc.)
  - Terraform State Files erkennen
  - docker-compose.yml / Dockerfile erkennen
  - Cron-Einträge, Webhook-Configs, .env Files
  - Output: Single-Page Inventory (Terminal + optional HTML)

- [ ] **`composit status`** — Aggregierter Zustand via MCP-Provider:
  - Verbindet sich mit croniq, hookaido, powerbrain (wenn vorhanden)
  - Zeigt: X Jobs, Y Channels, Z Knowledge Sources, geschätzte Kosten
  - Drift-Detection: Compositfile vs. Realität

### Tech-Entscheidung

- [ ] **TypeScript für v0.1** — MCP SDK ist TypeScript-native, schnellste Iteration.
  Performance-Optimierung (Rust/Go Rewrite) nur wenn nötig.
  Rationale: Geschwindigkeit ist kritischer als Performance im 6-12 Monate Window.

---

## Sprint 3: Community Launch (Woche 5-6)

### HN Launch

- [ ] **Show HN Post** — Zielgruppe: DevOps / Platform Engineers.
  Pragmatischer Titel ("Show HN: `composit scan` — see every service your AI
  agents created"). Konkretes CLI-Output zeigen, nicht nur Konzept.
- [ ] **Live-Demo vorbereiten** — `composit scan` auf echtem Projekt (nuts-infra)
  laufen lassen. Screenshot / asciinema für den Post.

### Landing Page

- [ ] **Waitlist-Page** — "See everything your AI agents built — before it breaks."
  Ziel: 200+ Signups in 30 Tagen.
  Features: Multi-team dashboard, drift alerts, compliance reports, cost tracking.
  CTA: Email-Signup für Early Access.
- [ ] **Pricing-Signal testen** — Feature-Interest-Clicks tracken:
  Was interessiert am meisten? (Multi-Creator, Compliance, Cost Tracking)

### Community

- [ ] **Posting-Plan** — r/devops, r/platformengineering, Platform Engineering Slack,
  CNCF Slack, DevOps-Meetups. Nicht nur HN.
- [ ] **croniq/hookaido/powerbrain READMEs** — Composit-Referenz ergänzen
  (nach positivem Signal aus Sprint 1 Interviews).

---

## Sprint 4: Reference Customer + Validation (Woche 7-8)

### Reference Customer

- [ ] **1 Team (10-50 Devs) als Early-Access-Partner** — Free Access gegen
  Case Study. Ihr realer Workflow zeigt ob der Pain "nice to have" oder
  "must have" ist.
- [ ] **Erste externe Provider-Integration** — Nicht croniq/hookaido/powerbrain.
  Ein Third-Party-Provider der die Composit-Spec implementiert.
  Stärkster Proof Point für die Open-Spec-Story.

### Entscheidungspunkt

Nach Sprint 4 die Frage beantworten:

- **Signal stark (Stars, Signups, Interview-Feedback, Reference Customer):**
  → Full Build. Spec finalisieren, CLI erweitern, Cloud-Tier starten.
- **Signal gemischt:** → Pivot-Kandidat prüfen. Ist composit besser als
  Feature in croniq/hookaido? Oder als MCP-Plugin statt eigenständiges Produkt?
- **Signal schwach:** → Spec open-sourcen, CLI maintenance-only, Fokus auf
  die Reference Providers (croniq, hookaido, powerbrain).

---

## Kill Criteria

Composit stoppen wenn EINS davon eintritt:
- 0/15 Interviewees beschreiben den Pain als "must have" (Sprint 1)
- <50 Signups nach 30 Tagen (Sprint 3)
- Kein Team bereit als Reference Customer (Sprint 4)
- Hyperscaler launcht Open-Source Agent Control Plane mit Community-Adoption
