# Composit — Roadmap

Public-facing list of what's open and what's deliberately not. For closed
design questions see [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md); for the
formal specs see [`rfcs/`](rfcs/).

This file is the contributor's "where can I help" entry point — each
item is small enough to scope in a single PR unless noted.

---

## Scanner principles

The scanner design choices that decide whether a given file is in scope:

- **Declarations, not runtime.** We read what the repo declares
  (`docker-compose.yml`), not what is actually running (Docker API).
  Runtime is the deployment tool's job; composit gives you the
  paper trail.
- **Standalone config files only.** Configs embedded in other tools
  (Caddy labels inside Docker Compose, OPA rules inside Ansible vars)
  are the host tool's territory — they get covered by a scanner for
  the host tool, not by the embedded-format scanner.
- **Terraform is a scanner target, not a provider.** `.tf` files are an
  agent's declared work product. Remote state and cloud APIs belong to
  Terraform itself.

Anything that contradicts these principles is a design discussion, not
a drive-by PR.

---

## Open CLI work

- **OPA runtime evaluation.** Today the `opa_policy` scanner parses
  `.rego` (package, rule heads, entrypoints) and `composit diff` reports
  `policy_parsed`. Actual rule evaluation against scan-derived inputs
  is the next milestone. Needs a composit-specific input shape (e.g.
  "deny if `docker_service.image` ends with `:latest`") rather than
  the request-shaped inputs most existing Rego libraries expect.

- **npx wrapper.** Zero-install distribution via
  `npx @composit/cli scan`. Pattern from biome / esbuild / turbo:
  a meta package plus one optional-dependency platform package per
  target (linux-x64, darwin-arm64, etc.). Blocker is the CI
  cross-compilation matrix, not the Rust.

- **Compositfile RFC (RFC 004).** The HCL schema is currently the
  parser; the RFC documents it so third parties can write Compositfiles
  with confidence. Covers `workspace`, `provider`, `budget`, `policy`,
  `require`, `allow`, `scan`.

- **Scanner benchmark.** `composit-scanner-tests` runs composit against
  a curated set of public repos and measures resource-coverage per
  scanner. Surfaces regressions (a scanner silently stops detecting
  Helm charts after a refactor) and drives the gap list below.

---

## Scanner gaps

Sorted by how often the file appears in the sampled repos.

**Tier 2 (remaining)**
- **Deploy scripts** — bespoke bootstrap/deploy/sync scripts. Hard to
  generalise; realistic approach is pattern matching on shell script
  names under `scripts/` or `deploy/`.
- **DB migrations** — schema-state counter (how many migrations,
  which framework). Alembic, sqlx, Flyway, Prisma.

**Tier 3 (low volume, specific)**
- `fly.toml` — Fly.io deployment.
- `render.yaml` — Render.com deployment.
- `vercel.json` — Vercel deployment config.
- `skaffold.yaml` — Skaffold K8s dev loop.
- `traefik.yml` / `traefik.toml` — Traefik reverse-proxy config.
- Protobuf / gRPC definitions — service surface.
- Tempo tracing config.

Each of these is roughly one file to add under `src/scanners/`, a
fixture under `tests/fixtures/`, and one entry in the E2E test file.
The `nginx` and `opa_policy` scanners are good references.

---

## Spec follow-ups

- **OAuth2 flow.** Currently `trust = "contract"` only accepts
  `auth.type = "api-key"`. OAuth2 is reserved in the parser but
  rejected — we turn it on when a second provider needs it, and
  codify the flow as its own RFC.

- **Multi-tier contracts.** RFC 002 Open Question #3 — can a provider
  publish several paid tiers (free/basic/pro) in one manifest, each
  with its own endpoints and SLAs?

- **Multi-identity per provider.** RFC 002 Open Question #1 — a team
  that holds multiple contracts with the same provider (different
  billing entities, dev vs. prod) may need one manifest to support
  several credentials simultaneously.

- **Full contract-response consumption.** `composit status --live`
  currently reads only `contract.{id, issued_at, expires_at, pricing_tier}`
  from the contract response. `endpoints`, `tools`, `sla`, `rate_limits`
  are defined in RFC 003 but not yet surfaced in the CLI.

---

## Deliberately out of scope

Patterns we keep saying no to, for reference:

- **Deployment.** Composit reads declarations; deploying them is
  Terraform / Pulumi / ArgoCD / Helm territory.
- **Inter-agent messaging.** Event routing lives in hookaido; direct
  A2A patterns are an emerging space we watch, not solve.
- **Service mesh / networking.** Routing, mTLS, traffic shaping are
  below composit's capability layer.
- **Proprietary scanner registry.** Scanners stay built-in until
  10+ external scanners exist; extension via
  `scan { extra_patterns { … } }` in the Compositfile covers the
  long tail.

---

## Contributing

If you pick up a roadmap item: open a discussion first for anything
that touches the scanner contract or the Compositfile schema, otherwise
just send a PR. The `self-scan.sh` gate in CI fails on governance
drift, so changes that add new resource types or rules also need to
keep the project's own Compositfile green.
