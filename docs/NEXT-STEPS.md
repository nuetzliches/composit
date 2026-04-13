# Composit — Next Steps

Stand: 2026-04-12 (post-Validation)

Basiert auf Business Validation Report (7.0/10, BUILD).
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

- [ ] **composit-report.yaml Spec v0.1 als RFC** — Formales Schema (JSON Schema),
  nicht nur Beispiele. Auf GitHub als Discussion oder PR veröffentlichen.
  Compositfile (Governance-Spec) separat als Post-MVP Draft.
- [ ] **Manifest Schema finalisieren** — composit.json v0.1 mit konkreten Feldern,
  Versionierung, Discovery-Mechanismus (.well-known URL, align mit MCP).

---

## Sprint 2: `composit status` CLI (Woche 3-4)

### Implementiert

- [x] **`composit scan`** — Rust CLI mit Plugin-ready Scanner-Architektur:
  - 6 Built-in Scanner: docker, env_files, terraform, cron, mcp_config, mcp_provider
  - Deep Docker Scan: Einzelne Services mit Image, Ports, Volumes, Networks
  - Git-blame Attribution mit Co-Authored-By-Erkennung (agent vs. human)
  - Last-Modified Attribution: Wer hat zuletzt geändert (erkennt Agent-Modifikationen)
  - 2-Phasen-Orchestrierung (Filesystem → Network)
  - Deklarative Config (composit.config.yaml): extra_patterns, Scanner toggle, Provider
  - Report-Deduplizierung, YAML/JSON/HTML Output, farbige Terminal-Ausgabe
  - Getestet gegen powerbrain (28 Resources), nuts-infra (61 Resources, 42 agent-modified)

- [x] **`composit status`** — Aggregierter View aus letztem Scan-Report:
  - Liest composit-report.yaml, zeigt Resources/Attribution/Provider-Übersicht
  - `--live` Flag für Live-Provider-Erreichbarkeit

### Tech-Entscheidung

- [x] **Rust für v0.1** — Single-Binary ohne Runtime-Dependencies.
  croniq-Expertise vorhanden. Bessere Distribution (cargo install, brew).

### Offen — Neue Scanner (priorisiert)

Scanner-Versioning: Version wird aus Docker-Image abgeleitet wenn verfügbar
(z.B. `caddy:2.8` → Caddy v2). Scanner meldet Fehler wenn Format nicht parsbar.
Keine eigene Versionserkennung im Scanner nötig.

**Tier 1 — Nächste Umsetzung** (hoher Wert, in unseren Stacks vorhanden):

- [ ] **Caddyfile** (8 Instanzen in nuts-infra, powerbrain, neurawerk, matrix-test)
  Routing-Topologie: welcher Service hängt an welcher Domain, TLS-Config.
  Domains und Upstream-Backends als Resources extrahieren.

- [ ] **CI/CD Workflows** (42 Instanzen: .forgejo/ + .github/)
  Build/Deploy-Pipelines: was wird automatisch deployed, Trigger, Ziel-Server.
  Jeder Workflow als Resource mit Trigger-Events und Steps.

- [ ] **Prometheus + Alerting** (3 Instanzen in powerbrain, hookaido-test)
  Monitoring-Targets, Scrape-Intervalle, Alert-Regeln mit Schwellwerten.
  Jedes Scrape-Target und jede Alert-Rule als Resource.

**Tier 2 — Danach:**

- [ ] **OPA/Rego Policies** (23 Instanzen in powerbrain)
  Security-Policies, Zugriffsregeln, EU AI Act Compliance.
  Jedes Policy-File als Resource mit Policy-Namen.

- [ ] **Grafana Config** (7 Instanzen in powerbrain)
  Datasources, Dashboard-Definitionen — Observability-Stack-Übersicht.

- [ ] **Deploy Scripts** (17 Instanzen in nuts-infra)
  Deployment-Automation: Bootstrap, Deploy, Sync — wer deployed wohin.

- [ ] **DB Migrations** (22 Instanzen in powerbrain)
  Schema-Zustand: Anzahl Migrationen, letzte Migration, Versionierung.

**Tier 3 — Bei Bedarf:**

- [ ] nginx.conf (3 Instanzen, nur Frontends)
- [ ] Hookaidofile (2 Instanzen, eigenes Format)
- [ ] Protobuf/gRPC Definitionen (3 Instanzen)
- [ ] Tempo Tracing Config (1 Instanz)

### Offen — composit status Erweiterungen

- [ ] **Live-Provider-Abfrage** — Nicht nur Report lesen, sondern aktive
  Ressourcen von croniq/hookaido/powerbrain via API abfragen
- [ ] **Drift-Detection** — Compositfile (Governance) vs. composit-report (Realität)

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
