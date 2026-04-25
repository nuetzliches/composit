# Composit — Roadmap

Public-facing list of what's open and what's deliberately not. For
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

- **Version-sync automation.** Releases currently require manually
  bumping `Cargo.toml`, the npm `package.json`, and the brew formula
  template in lockstep. `cargo-release` with a release hook that
  rewrites the matching lines would collapse this into a single
  `cargo release 0.3.0` invocation.

- **Scanner benchmark.** A reproducible coverage benchmark across a
  curated set of public repos — per-scanner resource counts and
  regression alerts when a scanner silently stops detecting something
  after a refactor. Skeleton exists internally; publishing the
  harness so anyone can run it is the open work.

---

## Scanner gaps

No active backlog — the Tier 2 and Tier 3 scanners (deploy scripts,
DB migrations, `fly.toml`, `render.yaml`, `vercel.json`, `skaffold.yaml`,
`traefik.yml`, protobuf, tempo) all shipped with v0.2.0.

New scanner ideas welcome. Each scanner is roughly one file under
`src/scanners/`, a fixture under `tests/fixtures/`, and one entry in
`tests/scanner_e2e.rs`. The `nginx` and `opa_policy` scanners are the
leanest references.

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
