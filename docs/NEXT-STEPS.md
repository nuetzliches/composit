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
  - 9 Built-in Scanner: docker, env_files, terraform, caddyfile, workflows, prometheus, cron, mcp_config, mcp_provider
  - Deep Docker Scan: Einzelne Services mit Image, Ports, Volumes, Networks
  - Terraform HCL-Parsing: Deklarierte Resources, Module, Provider aus .tf Dateien (via hcl-rs)
  - Caddyfile-Parsing: Site-Blöcke, reverse_proxy, file_server, TLS-Directives
  - CI/CD Workflows: GitHub Actions, Forgejo, Gitea, GitLab CI — Triggers, Jobs, Runner
  - Prometheus: Scrape-Configs, Job-Namen, Alerting-Rules, Rule-Groups
  - Multi-Author Attribution: Co-Authored-By bewahrt menschlichen Autor, Agents als Flag (agent_assisted)
  - Last-Modified Attribution: Wer hat zuletzt geändert (erkennt Agent-Modifikationen)
  - 2-Phasen-Orchestrierung (Filesystem → Network)
  - Deklarative Config (composit.config.yaml): extra_patterns, Scanner toggle, Provider
  - Report-Deduplizierung, YAML/JSON/HTML Output, farbige Terminal-Ausgabe
  - Getestet gegen 17 Repos (5 bis 3399 Resources pro Repo), validiert via composit-scanner-tests

- [x] **`composit status`** — Aggregierter View aus letztem Scan-Report:
  - Liest composit-report.yaml, zeigt Resources/Attribution/Provider-Übersicht
  - `--live` Flag für Live-Provider-Erreichbarkeit

### Tech-Entscheidung

- [x] **Rust für v0.1** — Single-Binary ohne Runtime-Dependencies.
  croniq-Expertise vorhanden. Bessere Distribution (cargo install, brew).

### Scanner Status

**Implementiert (9 Scanner):**

| Scanner | Resource-Typen | Status |
|---------|---------------|--------|
| docker | docker_compose, docker_service, dockerfile | ✅ Services, Images, Ports, Volumes, Networks |
| env_files | env_file | ✅ Variable-Count |
| terraform | terraform_config, terraform_resource, terraform_module, terraform_state | ✅ HCL-Parsing via hcl-rs |
| caddyfile | caddyfile, caddy_site | ✅ Site-Blöcke, reverse_proxy, TLS |
| workflows | workflow | ✅ GitHub Actions, Forgejo, Gitea, GitLab CI |
| prometheus | prometheus_config, prometheus_rules | ✅ Scrape-Configs, Alert-Rules |
| cron | cron_job | ✅ Crontab-Einträge |
| mcp_config | mcp_server | ✅ Claude Desktop, Cursor MCP-Config |
| mcp_provider | (via API) | ✅ Remote Provider Discovery |

### Scanner Gaps (ermittelt via composit-scanner-tests, 2026-04-14)

Validiert gegen 17 öffentliche + interne Repos. Gap-Analyse zeigt Dateien
die in gescannten Repos existieren, aber von keinem Scanner erfasst werden.

**Tier 1 — Höchste Priorität** (häufig, hohes Volumen):

- [ ] **Kubernetes Manifests** (~100 Dateien in Test-Repos)
  YAML mit `apiVersion:` — Deployments, Services, ConfigMaps, Ingress.
  Resource-Typ + Namespace + Name extrahieren.
  Erkennungsmuster: `apiVersion:` in YAML (nicht Workflow/Compose/Prometheus).

- [ ] **Kustomize** (22 Dateien in Test-Repos)
  `kustomization.yaml` — Overlay-Struktur, referenzierte Bases und Resources.
  Oft zusammen mit Kubernetes Manifests.

- [ ] **Helm Charts** (5 Dateien in Test-Repos)
  `Chart.yaml` — Chart-Name, Version, Dependencies, Values.
  Zusammen mit templates/ Verzeichnis.

**Tier 2 — Mittel** (vorhanden, moderates Volumen):

- [ ] **nginx** (10 Dateien in Test-Repos)
  `nginx.conf` — Server-Blöcke, Upstreams, Locations.
  Ähnlich wie Caddyfile: Reverse-Proxy-Topologie.

- [ ] **OPA/Rego Policies** (in internen Stacks)
  Security-Policies, Zugriffsregeln.

- [ ] **Grafana Config** (2 Dateien in Test-Repos + interne Stacks)
  Dashboard-JSON, Datasource-Konfiguration.

- [ ] **Deploy Scripts** (in internen Stacks)
  Deployment-Automation: Bootstrap, Deploy, Sync.

- [ ] **DB Migrations** (in internen Stacks)
  Schema-Zustand: Anzahl Migrationen, Versionierung.

**Tier 3 — Niedrig** (selten, spezifisch):

- [ ] fly.toml (1 Datei — Fly.io Deployments)
- [ ] render.yaml (1 Datei — Render.com)
- [ ] vercel.json (2 Dateien — Vercel Deployments)
- [ ] Skaffold (1 Datei — skaffold.yaml)
- [ ] Hookaidofile (eigenes Format)
- [ ] Protobuf/gRPC Definitionen
- [ ] Tempo Tracing Config
- [ ] Traefik (traefik.yml/traefik.toml)

### Scanner-Prinzipien

- **Deklarationen scannen, nicht Runtime** — Wir lesen was im Repo steht,
  nicht was deployed ist. docker-compose.yml, nicht Docker API.
- **Nur standalone Config-Dateien** — Keine in andere Tools eingebettete
  Configs (z.B. Caddy-Labels in Docker-Compose, Caddy-Config in Ansible-Vars).
  Für eingebettete Configs → separater Scanner für das Host-Tool (z.B. Ansible).
- **Terraform als Scanner, nicht als Provider** — .tf Dateien sind das
  Arbeitsergebnis des Agents. State/Cloud-APIs sind Terraforms Domäne.

### Offen — composit status Erweiterungen

- [ ] **Live-Provider-Abfrage** — Nicht nur Report lesen, sondern aktive
  Ressourcen von croniq/hookaido/powerbrain via API abfragen
- [ ] **Drift-Detection** — Compositfile (Governance) vs. composit-report (Realität)

---

## Sprint 3: Community Launch (Woche 5-6)

### Distribution: npx-Wrapper (Zero-Install)

- [ ] **npm-Distribution** — Rust-Binary via `npx composit scan` aufrufbar,
  ohne vorherige Installation. Cross-Platform (Windows/macOS/Linux).

  **Architektur** (Pattern von biome, esbuild, turbo):
  ```
  composit                          ← Haupt-Package (JS wrapper, bin-Entry)
  ├── @composit/cli-win32-x64       ← optionalDependency (composit.exe)
  ├── @composit/cli-darwin-arm64    ← optionalDependency (macOS ARM)
  ├── @composit/cli-darwin-x64      ← optionalDependency (macOS Intel)
  ├── @composit/cli-linux-x64       ← optionalDependency (Linux x64)
  └── @composit/cli-linux-arm64     ← optionalDependency (Linux ARM)
  ```
  npm installiert nur das Platform-Package für die aktuelle Architektur.
  Das Haupt-Package enthält ein JS-Script das die richtige Binary findet
  und mit `execFileSync` durchreicht.

  **Voraussetzungen:**
  - CI-Pipeline mit Cross-Compilation (GitHub Actions Matrix oder `cross`-Crate)
  - npm-Publish in der Release-Pipeline (5-6 Packages pro Release)
  - Rust-Code ändert sich nicht — nur Packaging/Distribution

  **Parallel weiterhin:** `cargo install composit`, GitHub Releases, brew.

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

## Ecosystem-Strategie: Badge System + Scanner Hub

### Provider-Badge "Composit-Compatible"

Ein Badge-System ermöglicht Providern, ihre Composit-Kompatibilität sichtbar zu machen —
ähnlich einem OpenAPI-Badge oder "Works with Homebrew" Label.

**Mechanismus:**
1. Provider implementiert `/.well-known/composit.json` (Manifest-Spec, bereits definiert)
2. Composit stellt öffentlichen Validator bereit: `composit validate <url>`
3. Bei Erfolg: SVG-Badge (shields.io-Stil oder eigener Endpoint)

**Wert für Provider:** Sichtbarkeit im Ecosystem, Vertrauen bei Platform Engineers
("dieser Provider ist auditierbar"), spätere Premium-Badge-Tier möglich.

**Wert für Composit:** Netzwerkeffekt — jeder neue Provider stärkt die Spec als Standard.

**Wichtig:** Badge-System erst wenn Spec stabil ist (nach Sprint 4). Sonst badgen
Provider gegen ein sich änderndes Target.

### Scanner Hub — Post-MVP

Ein zentraler Hub der eigenständig Scanner updated (à la GitHub Actions Marketplace)
macht erst Sinn wenn 10+ externe Scanner existieren die wir nicht selbst pflegen.

**Jetzt:** Scanner bleiben built-in (Qualitätskontrolle), Erweiterung via
`extra_patterns` in `composit.config.yaml` reicht für Early Adopters.

### MCP für Composit Docs

Ein MCP der Spec, Schema-Definitionen und Beispiele als Tools exposed:
- Agents die Provider bauen können direkt fragen "wie implementiere ich composit.json?"
- Niedrige Hürde, hoher Signal-Wert für Spec-Adoption

### Zeitplan

| Zeitpunkt | Was |
|-----------|-----|
| Sprint 3 | Spec + Schema als RFC auf GitHub |
| Nach Reference Customer (Sprint 4) | Badge-System + `composit validate` CLI |
| Nach 5+ externen Providern | MCP für Composit Docs |
| Nach Community-Wachstum | Scanner Hub evaluieren |

---

## Kill Criteria

Composit stoppen wenn EINS davon eintritt:
- 0/15 Interviewees beschreiben den Pain als "must have" (Sprint 1)
- <50 Signups nach 30 Tagen (Sprint 3)
- Kein Team bereit als Reference Customer (Sprint 4)
- Hyperscaler launcht Open-Source Agent Control Plane mit Community-Adoption
