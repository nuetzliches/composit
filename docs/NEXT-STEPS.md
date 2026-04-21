# Composit — Next Steps

Stand: 2026-04-21 (Self-Hosting gelandet; Tier-1 + Tier-2 Scanner komplett)

Basiert auf Business Validation Report (7.5/10, BUILD — re-assessment 2026-04-14).
Zeitgebundene Sprints mit klarem ICP-Fokus: Platform Engineers + CTOs.

**Arbeitsmodus (explizit):** Wir validieren nicht über Interviews, sondern
über **Dogfooding + Launch-Signale**. Annahmen werden als solche markiert
und durch öffentliche Artefakte (Spec, Demo, Landing) überprüft.

---

## Offene Stränge (Stand 2026-04-20)

Konsolidiert aus allen Sprints, sortiert nach unmittelbarer Blocker-Qualität.
Erledigt: RFC 001-003 (Draft), Public/Contract-Split end-to-end inkl.
`contract_expired`, Contract-Endpoint auf nuetzliche.it, composit-landing
(ersetzt durch OSS-native Landing ohne Plausible/DNS-Arbeit).

**Blocker für Launch:**

- [x] **Live-Demo (public) bauen** — `examples/demo-drift/` (widgetshop
  workspace, 3 deterministische Errors). README + Haupt-README-Link.
- [x] **Asciinema-Recording-Script** — `docs/demo/record.sh` (35s,
  deterministisch). Eigentliche Aufnahme muss noch gemacht werden.
- [ ] **Asciinema-Recording aufnehmen + publishen** — mit obigem Script,
  Ergebnis als `composit-demo.cast` auf asciinema.org + im HN-Post einbetten.
- [ ] **Show-HN-Post** selbst (Woche-5/6 in Sprint 3).

**Nicht-Blocker, aber auf der Liste:**

- [ ] **NUETZLICHE_COMPOSIT_CONTRACT_KEYS** im Production-`.env` auf
  ext-docker-host-1 setzen und nuetzliche-site neu starten — erst
  dann antwortet `https://nuetzliche.it/contract` auf echte Keys.
  Passthrough liegt in `ext-docker-host-1/docker-compose.yml`, Doku in
  `ext-docker-host-1/.env.example`.
- [ ] **Dogfood-Runs dokumentieren** — `composit scan` + `diff`-Artefakte
  aus nuts-infra, croniq, hookaido, powerbrain als Demo-Material.
- [ ] **npx-Wrapper** für Zero-Install-Distribution (Sprint 3).
- [x] **Scanner-Tests nachziehen** — inline Unit-Tests für docker,
  env_files, cron, mcp_config, mcp_provider bestätigt; End-to-End-Suite
  in `tests/scanner_e2e.rs` deckt scan + diff auf Fixture-Workspaces ab.
- [x] **Percentage-Validierung** im Compositfile-Parser (0-100% Range,
  inkl. Missing-`%`-Suffix + Decimal-Fällen).
- [ ] **OPA Runtime-Evaluation** — Rego tatsächlich ausführen statt nur
  parsen.
- [x] **Scanner-Gaps Tier 1 — K8s Manifests, Kustomize, Helm Charts**:
  `kubernetes`-Scanner emittiert `kubernetes_manifest`, `kustomization`,
  `helm_chart`. Multi-Doc-YAML, `Kind/Name` qualifizierte Namen, skip
  für `templates/`, vendor und andere Scanner-Pfade.
- [ ] **Scanner-Benchmark** (`composit-scanner-tests`, Coverage-Metrik).

**Spec-Folgearbeiten (keine Consumer-Blocker):**

- [ ] **RFC 004 Compositfile-Spec** — HCL-Schema dokumentieren
  (workspace, provider, budget, policy, require). Abgeleitet aus
  tatsächlicher Parser-Implementierung.
- [ ] **OAuth2-Flow** — auf Roadmap. Vollausbau eigener RFC wenn ein
  zweiter Provider das fordert.
- [ ] **Multi-Tier-Contracts** (RFC 002 Open Question #3).
- [ ] **Multi-Identity pro Provider** (RFC 002 Open Question #1).
- [ ] **Voller CLI-Consume der Contract-Response** — v0.1 liest nur
  `contract.{id, issued_at, expires_at, pricing_tier}`. Endpoints,
  tools, sla, rate_limits bleiben ungenutzt bis ein Consumer sie
  fordert.

---

## Sprint 1 (revidiert): Spec + Dogfooding statt Interviews (Woche 1-2)

### ICP locked

- [x] **ICP-Entscheidung:** Platform Engineers + CTOs als zahlende Zielgruppe.
  Solo Devs = Community/Adoption Funnel, nicht Produkt-Fokus.
- [x] **Narrative schärfen** — README, STRATEGY, HN-LAUNCH auf Governance-as-Code
  umgeschrieben (Commits 7ab3c72, c05c493).

### Dogfooding statt Interviews

Anstelle von 10-15 Interviews validieren wir über eigene Stacks:

- [ ] **Dogfood-Runs auf eigenen Repos** — `composit scan` + `composit diff`
  auf nuts-infra, croniq, hookaido, powerbrain. Jeder Run muss ein
  Artefakt (Report + Diff) erzeugen, das als Demo-Material taugt.
  Gefundene echte Drifts = stärkstes internes Signal.
- [ ] **Scanner-Benchmark etablieren** — `composit-scanner-tests` (17 Repos)
  als laufende Quality-Signal-Quelle. Coverage-Metrik: Anteil erkannter
  Ressourcen pro Repo. Regression-Schutz vor jedem Release.

### Spec

- [x] **composit-report.yaml Spec v0.1 als RFC** — RFC 001 Draft,
  `schemas/composit-report-v0.1.json`.
- [ ] **Compositfile Spec als RFC** — HCL-Schema dokumentieren
  (workspace, provider, budget, policy, require). Abgeleitet aus
  tatsächlicher Parser-Implementierung. Offen als RFC 004.
- [x] **Manifest Schema finalisieren** — RFC 002 (Public/Contract-Tier
  Split) + RFC 003 (Contract-Response-Envelope) jeweils Draft, Schema
  und Reference-Implementation auf nuetzliche.it.

### Annahmen (explizit, zu widerlegen durch Signale)

| # | Annahme | Widerlegung durch |
|---|---------|-------------------|
| A1 | Platform Engineers erkennen den Pain aus Report/Blogpost | HN + Reddit Reaktionen |
| A2 | Drift-Detection ist wichtiger als Cost-Attribution | Feature-Interest-Click-Tracking auf Landing |
| A3 | Open-Spec wird als Standard gesehen, nicht als Vendor-Grab | GitHub Discussion Engagement |
| A4 | Compositfile-Syntax (HCL) ist ergonomisch genug | Dogfooding-Erfahrung + Early-Adopter-Feedback |

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

**Implementiert (13 Scanner):**

| Scanner | Resource-Typen | Status |
|---------|---------------|--------|
| docker | docker_compose, docker_service, dockerfile | ✅ Services, Images, Ports, Volumes, Networks |
| env_files | env_file | ✅ Variable-Count |
| terraform | terraform_config, terraform_resource, terraform_module, terraform_state | ✅ HCL-Parsing via hcl-rs |
| caddyfile | caddyfile, caddy_site | ✅ Site-Blöcke, reverse_proxy, TLS |
| nginx | nginx_config | ✅ Server-Blöcke, Upstreams, Proxies, SSL |
| workflows | workflow | ✅ GitHub Actions, Forgejo, Gitea, GitLab CI |
| prometheus | prometheus_config, prometheus_rules | ✅ Scrape-Configs, Alert-Rules |
| grafana | grafana_dashboard, grafana_datasource, grafana_dashboard_provider | ✅ Dashboard-JSON, Provisioning-YAML |
| cron | cron_job | ✅ Crontab-Einträge |
| kubernetes | kubernetes_manifest, kustomization, helm_chart | ✅ Multi-Doc-YAML, Kind/Name-qualifiziert, Kustomize + Helm Chart.yaml |
| opa_policy | opa_policy | ✅ Package, Rule-Count, allow/deny/violation-Entrypoints |
| mcp_config | mcp_server | ✅ Claude Desktop, Cursor MCP-Config |
| mcp_provider | (via API) | ✅ Remote Provider Discovery |

### Scanner Gaps (ermittelt via composit-scanner-tests, 2026-04-14)

Validiert gegen 17 öffentliche + interne Repos. Gap-Analyse zeigt Dateien
die in gescannten Repos existieren, aber von keinem Scanner erfasst werden.

**Tier 1 — Höchste Priorität** (häufig, hohes Volumen):

- [x] **Kubernetes Manifests** (~100 Dateien in Test-Repos)
  `kubernetes`-Scanner parsed Multi-Doc-YAML mit `apiVersion` + `kind`,
  emittiert pro Document ein `kubernetes_manifest` mit Namespace + Kind.

- [x] **Kustomize** (22 Dateien in Test-Repos)
  `kustomization.yaml`/`Kustomization` → `kustomization`-Resource mit
  resource-/base-/component-Counts und Namespace.

- [x] **Helm Charts** (5 Dateien in Test-Repos)
  `Chart.yaml` → `helm_chart`-Resource mit Chart-Name, Version,
  Dependency-Count. `templates/` wird übersprungen (Go-templated YAML
  parst kaum sauber).

**Tier 2 — Mittel** (vorhanden, moderates Volumen):

- [x] **nginx** — `nginx`-Scanner. Fingerprint-basiert (erkennt echte
  nginx-Config an `server {` / `upstream` / `proxy_pass`, nicht an der
  Datei-Endung) um andere `.conf`-Tools nicht fälschlich zu matchen.

- [x] **OPA/Rego Policies** — `opa_policy`-Scanner emittiert pro `.rego`
  ein `opa_policy` mit Package, Rule-Count und allow/deny/violation-
  Entrypoints. Orthogonal zu RFC-001 `policy`-Referenzen im Compositfile
  — findet freie Policies im Repo, nicht nur die explizit deklarierten.

- [x] **Grafana Config** — `grafana`-Scanner erkennt Dashboards über
  `schemaVersion + panels` (shape-basiert, Pfad egal) und Provisioning-
  YAML nur unter `**/provisioning/{dashboards,datasources}/*.yaml` um
  Kollision mit docker-compose/K8s-Scans zu vermeiden.

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

### Critical Path (Governance-as-Code Kern) — erledigt

- [x] **Compositfile Parser** (Commit d96ae0f) — HCL-Parsing via `hcl-rs`.
  Typisierte Governance-Struct: workspace, approved providers, budget,
  policies, require-blocks. 5 Unit-Tests.

- [x] **`composit diff`** (Commits d96ae0f, abe7e53, a5bfe91) — Vergleich
  composit-report.yaml (IST) gegen Compositfile (SOLL). Terminal + YAML/JSON
  + HTML-Output. Severity-Klassifizierung (Error/Warning/Info). 10 Unit-Tests.
  unused_provider = Warning (a5bfe91).

### Offen — Sprint 2 Restposten

- [ ] **Live-Demo (public) bauen** — separates, synthetisches Beispiel-Repo
  (oder Fixture im composit-Repo) mit Compositfile + bewusst eingebauter
  Drift. Darf keine privaten Infrastruktur-Details enthalten —
  der frühere `examples/demo/`-Ordner wurde entfernt, weil er auf
  nuts-infra-Daten basierte. Haupt-HN-Artefakt, ergänzt durch
  asciinema-Recording für den Post.

- [ ] **Scanner-Tests nachziehen** — Unit-Tests für caddyfile, terraform,
  workflows, prometheus bestehen. Offen: **docker, env_files, cron,
  mcp_config, mcp_provider**. Priorität: docker (größtes Volumen).
  Dazu mindestens ein End-to-End-Test (`composit scan` auf Fixture-Repo).

- [x] **Live-Provider-Abfrage** — `composit status --live` fetcht
  `/.well-known/composit.json`, parst das Manifest und merged
  Capabilities/Protocol auf die Provider-Liste. Description, Region
  und Compliance-Tags werden im Terminal angezeigt.

- [ ] **OPA Runtime-Evaluation** — Tatsächliches Ausführen der Rego-Regeln
  gegen report-abgeleitete Inputs. Aktueller Stand: `composit diff`
  parst .rego Dateien textuell (Package, Rules, Entrypoints), meldet
  Syntax-Issues, aber evaluiert noch nicht. Realistisches V1 braucht
  composit-spezifische Rego-Policies (z.B. "deny if docker_service.image
  endet auf :latest") — powerbrains eigene `.rego` Dateien erwarten
  Request-shaped Inputs, nicht Scan-Reports, und wären für diff-time
  Evaluation nicht sinnvoll.

- [ ] **Percentage-Validierung in Compositfile-Parser** — `alert_at: "150%"`
  wird aktuell akzeptiert. Range 0-100% prüfen.

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
- [ ] **Live-Demo vorbereiten** — `composit scan` + `diff` auf dem synthetischen
  Public-Demo-Repo (siehe Sprint 2 Restposten). Keine Privat-Infrastruktur-
  Screenshots. Asciinema-Recording für den Post.

### Landing Page (Signalquelle #1 ohne Interviews)

Ohne Interviews ist die Landing-Page der wichtigste externe Berührungspunkt.
**Scope-Entscheidung (2026-04-18):** Keine Waitlist / kein Email-Signup,
kein Analytics. composit ist OSS — es gibt keinen "Launch" zu announcen.
GitHub-Stars sind das externe Signal, alles andere ist UX.

- [ ] **Landing-Page online** (VOR HN, nicht parallel) — statische Seite
  auf `nuetzliches.github.io/composit` (GitHub Pages, `landing/` via Actions).
  Primary CTA: **★ Star on GitHub**. Quick-Start mit Copy-Buttons.
- [ ] **Feature-Voting-UX** — Klicks auf "Drift Alerts",
  "Cost Attribution", "Compliance Reports", "Multi-Agent Visibility",
  "Team Dashboard", "OPA at Commit Time". Reine Client-UX
  (localStorage-Dedup), kein Backend.
- [ ] **Signal-Benchmark** (30 Tage nach HN):

  | Metrik | Grün | Gelb | Rot |
  |---|---|---|---|
  | GitHub-Stars | ≥300 | 100–300 | <100 |

### Community

- [ ] **Posting-Plan** — r/devops, r/platformengineering, Platform Engineering Slack,
  CNCF Slack, DevOps-Meetups. Nicht nur HN.
- [ ] **croniq/hookaido/powerbrain READMEs** — Composit-Referenz ergänzen
  (nach positivem HN + Star-Signal).

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

Nach Sprint 4 die Frage beantworten (ohne Interview-Signal, nur öffentliche Signale):

- **Signal stark** (≥300 GitHub-Stars, ≥3000 Pageviews, Feature-Votes
  mit klarem Top-2, ≥1 externer Provider implementiert Spec,
  HN/Reddit-Kommentare bestätigen Pain):
  → Full Build. Spec finalisieren, CLI erweitern, Team-Tier-Exploration
  starten (jetzt MIT konkretem Beta-Produkt als Waitlist-Aufhänger).
- **Signal gemischt** (100–300 Stars, 1000–3000 Pageviews, Votes gemischt,
  kein externer Provider):
  → Pivot-Kandidat prüfen. Composit als Feature in croniq/hookaido? Oder
  als MCP-Plugin statt eigenständiges Produkt?
- **Signal schwach** (<100 Stars, <1000 Pageviews, keine externe Resonanz):
  → Spec open-sourcen, CLI maintenance-only, Fokus auf die Reference
  Providers (croniq, hookaido, powerbrain).

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

## Kill Criteria (ohne Interview-Pfad)

Composit stoppen wenn EINS davon eintritt:
- **Dogfooding negativ** (Sprint 1-2): `composit diff` findet auf eigenen
  Stacks (nuts-infra, croniq, hookaido, powerbrain) keine echten Drifts,
  die wir vorher übersehen hätten. Dann fehlt der Pain sogar bei uns.
- **HN + Stars schwach** (Sprint 3): <100 GitHub-Stars, <1000 Pageviews
  nach 30 Tagen, überwiegend "cool, aber wozu"-Kommentare statt "genau
  das brauche ich".
- **Kein externer Provider** (Sprint 4): 8 Wochen post-Launch kein einziger
  Third-Party-Provider implementiert `.well-known/composit.json`. Open-Spec-Story
  trägt dann nicht.
- **Hyperscaler Open-Source-Move**: AWS/Microsoft launcht Open-Source Agent
  Control Plane mit echter Community-Adoption.

**Wichtig:** "Gelb"-Signale (100–300 Stars, Pageviews da, Feature-Votes
ohne klaren Top-2) führen zu Positioning-/Message-Iteration, nicht zu
Kill. Kill nur bei klar negativem Signal aus mehreren Quellen.
