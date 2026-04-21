# RFC 002 — Provider Manifest: Public + Contract Tiers

- **Status:** Draft
- **Authors:** nuetzliches/composit maintainers
- **Created:** 2026-04-19
- **Schema (public, v0.1):** `schemas/composit-provider-manifest-v0.1.json`
- **Schema (contract):** _pending — personalized per identity; informative example only in this RFC_
- **Supersedes:** the implicit "one public manifest" model used in
  `examples/composit-manifest.json` before 2026-04-19.

## Summary

Splits the provider manifest into two explicit tiers:

1. **Public Manifest** — `/.well-known/composit.json`. Unauthenticated,
   cacheable, minimal. Contains provider identity, capability *types*,
   compliance claims, and a pointer (`contracts[]`) to contract
   endpoints.
2. **Contract Manifest** — URL advertised by the public manifest,
   authenticated, personalized per identity. Contains concrete
   endpoints, tool inventories, SLAs, rate limits, and pricing
   relevant to *the caller*.

v1 auth: **API key** (header-based). OAuth2 is a declared roadmap
upgrade, not a v1 requirement.

## Motivation

The working assumption until now was that one public manifest
describes everything a provider offers. Running `composit` against the
`nuetzliche` stack exposed the problem:

- `nuetzliche.it/.well-known/composit.json` declares capability slots
  for croniq, hookaido, powerbrain.
- The actual product endpoints (`mcp.nuetzliche.it`,
  `hooks.nuetzliche.it`, …) sit behind an Authentik
  gate — they only work once a business relationship is established.
- The current manifest therefore *leaks* information — concrete
  endpoints, tool counts, descriptions — that a prospect can read but
  cannot actually use, and leaks them regardless of whether any
  relationship exists.
- And worse, it doesn't tell a consumer *how* to establish that
  relationship. There is no declared "contract URL", no stated auth
  method, no next step.

Authenticated access is the point of the Contract Trust idea:

> Contract Trust — after a business relationship is established,
> authenticated access unlocks concrete endpoints, SLAs, and pricing.
> Machine-readable B2B.

This RFC fills in the protocol that turns that idea into something a
client and a provider can implement against.

## Non-goals

- **Not a payment or billing protocol.** Pricing *appears* in the
  contract manifest (so a programmatic client can surface it) but the
  payment rail is out of scope.
- **Not an authn / authz server.** This RFC specifies how a provider
  *declares* that auth is required and which method it uses. It does
  not prescribe how identities are issued, revoked, or rotated. v1
  relies on out-of-band API-key provisioning; OAuth2 is a future
  refinement.
- **Not a multi-identity Compositfile spec.** A team that holds several
  identities against the same provider (dev + prod tenants, staging)
  repeats the provider block today. Multi-identity syntactic sugar is
  tracked in the Open Questions section but not normative.

## Design

### Two URLs, not two responses

Three patterns were evaluated:

- **(a)** Same URL, always public, 401 at the product endpoints.
  Rejected: doesn't solve the leak problem — concrete endpoints still
  live in the public document.
- **(b)** Same URL, auth-sensitive content.
  Rejected: one URL with two contracts breaks caching, makes the
  JSON-Schema ambiguous, and hides the tier split from consumers.
- **(c)** Two URLs — public at `/.well-known/composit.json`, contract
  at a provider-chosen URL. **Selected.**

Rationale for (c):

- Clean cache semantics (the public URL is publicly cacheable; the
  contract URL is not).
- JSON-Schema per tier is unambiguous.
- Matches the OAuth2 pattern (public `.well-known/oauth-*` → protected
  token/userinfo endpoints) that anyone doing B2B auth already knows.
- Gives providers a natural extension point (`contracts[]`, not
  `contract`) if they later publish several contract tiers — e.g. a
  free evaluation contract and a paid production contract.

### Content tiers

#### Public Manifest — what MAY live here

- **Provider identity:** `name`, `description`, `website`, `contact`.
- **Compliance claims:** `compliance: ["gdpr", "eu-ai-act", ...]`,
  `region`, `self_hosted`. These are marketing claims the provider
  *wants* the world to see.
- **Capability types:** `capabilities[*]` with `type`, `product`,
  `protocol`. No endpoints, no tool counts, no descriptions beyond
  the product name.
- **Contract pointers:** `contracts[]`, each with `url` and a minimal
  `auth` descriptor so a client knows *what kind of credentials* it
  needs before contacting sales.
- **Extensions:** `x-` prefixed fields (see RFC 001).

#### Public Manifest — what MUST NOT live here

- Concrete endpoint URLs (`mcp.example.com/...`).
- Tool counts, tool names, tool schemas.
- Full capability descriptions beyond one sentence identifying the
  product.
- SLA numbers, rate limits, pricing tiers.
- OAuth scopes that reveal internal structure.
- Anything a competitor could scrape to profile the provider's surface.

#### Contract Manifest — what it contains

- **Everything the consumer needs to actually use the provider**
  against their negotiated terms:
  - Concrete `endpoints` per capability (MCP URL, HTTP base URL, …)
  - Full `tools` inventory per capability
  - `sla`: latency target, uptime, incident contact
  - `rate_limits`: per-endpoint, per-tool, per-window
  - `pricing_tier` identifier (not the numbers themselves if those
    are out-of-band)
  - `regions`: which the identity may use
  - Auth-specific metadata (scopes granted, token TTL, refresh URL)

- **Personalized per identity.** The same contract URL returns
  different JSON for different authenticated callers:
  - Customer A on the free tier sees the shared cluster endpoint
    with 100 req/min.
  - Customer B on enterprise sees a dedicated instance endpoint
    with 10 000 req/min and a named incident contact.
  - An expired API key returns 401 with a revocation reason.

  The schema therefore describes the *shape* of any contract
  response; the content is identity-dependent.

### Public Manifest v0.1 (schema)

Full schema at
`schemas/composit-provider-manifest-v0.1.json`. Summary:

```json
{
  "composit": "0.1.0",
  "provider": {
    "name": "nuetzliche",
    "description": "MCP-native infrastructure for AI-driven ecosystems",
    "website": "https://nuetzliche.it",
    "contact": "composit@nuetzliche.it"
  },
  "capabilities": [
    { "type": "scheduling", "product": "croniq",     "protocol": "mcp" },
    { "type": "events",     "product": "hookaido",   "protocol": "mcp" },
    { "type": "knowledge",  "product": "powerbrain", "protocol": "mcp" }
  ],
  "compliance": ["gdpr", "eu-ai-act"],
  "region": "eu-central-1",
  "self_hosted": true,
  "contracts": [
    {
      "url": "https://nuetzliche.it/contract",
      "auth": { "type": "api-key", "header": "X-Composit-Api-Key" }
    }
  ],
  "discovery": {
    "well_known": "https://nuetzliche.it/.well-known/composit.json"
  }
}
```

Note what is **absent** versus today's manifest:

- No `tools`, `description`, `repo`, `license` on capabilities.
- No concrete product hostnames.

Those survive in **the product's own public manifest** (e.g.
`croniq.dev/.well-known/composit.json`), which is still useful for
open-source discovery — but not mixed with the org-level claim.

### Contract Manifest (informative, v0.1)

No formal schema in this RFC because the response shape is
identity-personalized. An illustrative response for an authenticated
caller:

```json
{
  "composit": "0.1.0",
  "contract": {
    "id": "c-2026-nuetzliche-42",
    "provider": "nuetzliche",
    "issued_at": "2026-04-01T00:00:00Z",
    "expires_at": "2027-04-01T00:00:00Z",
    "pricing_tier": "team"
  },
  "capabilities": [
    {
      "type": "scheduling",
      "product": "croniq",
      "protocol": "mcp",
      "endpoint": "https://mcp.nuetzliche.it/croniq",
      "tools": 12,
      "rate_limit": { "requests_per_minute": 120 },
      "regions": ["eu-central-1"]
    }
  ],
  "sla": {
    "uptime_pct": 99.5,
    "incident_contact": "sre@nuetzliche.it"
  }
}
```

The same URL, called by a different identity (different API key),
would return a different body — for example another `endpoint`
pointing at a dedicated instance, different `rate_limit`, a higher
`uptime_pct`.

A formal Contract Manifest schema is expected in RFC 003 once we have
two providers implementing it and can generalize. v1 clients should
treat unknown fields as ignored (`additionalProperties: true`).

### Auth — v1 and roadmap

**v1: API key, header-based.**

The public manifest declares:

```json
"auth": { "type": "api-key", "header": "X-Composit-Api-Key" }
```

Clients attach the key on every request to the contract URL (and to
whatever endpoints the contract subsequently advertises — those MAY
require a different key, specified by the contract response).

Provisioning is out-of-band — typically via a provider's signup or
sales flow. The `provider.contact` field in the public manifest is
the canonical "how do I start?" pointer.

**Roadmap: OAuth2 client-credentials flow.**

Expected shape (non-normative, pending RFC 003):

```json
"auth": {
  "type": "oauth2",
  "discovery_url": "https://provider.example/.well-known/oauth-authorization-server",
  "grant_types": ["client_credentials"],
  "scopes_required": ["composit:contract.read"]
}
```

OAuth2 brings token expiration, scoped access, and a standard
registration flow. Deferred to a later RFC because (a) API keys solve
the v1 problem, (b) MCP itself is still working out its OAuth story —
we want to align, not front-run.

**Explicitly out of scope for this RFC:**
mTLS, SPIFFE/SPIRE, signed requests, per-request attestation. All are
valid for specific deployments but none are a v1 ecosystem minimum.

### Compositfile extensions

Current shape:

```hcl
provider "powerbrain" {
  manifest = "https://nuetzliche.it/.well-known/composit.json"
  trust    = "contract"
}
```

`trust = "contract"` becomes meaningful: it signals that composit
SHOULD follow the `contracts[]` pointer from the public manifest. To
do so it needs credentials, declared alongside the provider:

```hcl
provider "powerbrain" {
  manifest = "https://nuetzliche.it/.well-known/composit.json"
  trust    = "contract"

  auth {
    type = "api-key"
    env  = "NUETZLICHE_COMPOSIT_KEY"
  }
}
```

Rules:

- The `auth` block is REQUIRED when `trust = "contract"`.
- The `env` attribute names an environment variable — composit never
  reads or writes a secret from the Compositfile itself. That file
  is committed; secrets are not.
- `auth.type` MUST match the `auth.type` the public manifest
  advertises, otherwise `composit diff` errors with
  `auth_method_mismatch`.
- If `auth.env` is unset at scan time, composit falls back to
  public-only behaviour and emits a `contract_auth_missing` info
  diagnostic — not an error, because CI pipelines that intentionally
  run offline shouldn't fail on it.

### scan/diff behaviour

**`composit scan`:**

1. Fetch the public manifest at the `manifest` URL.
2. If the Compositfile declared `trust = "contract"` *and* the auth
   credential is available:
   a. Fetch each `contracts[*].url` with the configured auth.
   b. Merge the contract response into the scanned provider entry,
      preserving public fields and adding `endpoints`, `tools`,
      `rate_limit`, etc.
3. Record the scan mode (public-only vs contract) in the produced
   `composit-report.yaml` under a new `auth_mode` field on each
   provider entry: `"public" | "contract" | "unreachable"`.
4. If a contract fetch fails (401, timeout, schema mismatch), the
   provider is kept at `"public"` and a warning surfaces in the
   scanner output.

**`composit diff`:**

New rule identifiers under the `providers` category:

- `contract_auth_missing` (Info) — provider declares `trust = "contract"`
  but the configured env var is unset; scan ran public-only.
- `contract_auth_mismatch` (Warning) — provider's public manifest
  advertises `auth.type = "api-key"`, but the Compositfile names a
  different type. Fix one side.
- `contract_unreachable` (Warning) — credentials present, contract URL
  failed. Distinct from plain 401 because the intent is right but the
  endpoint is broken.
- `contract_unauthorized` (Error) — credentials present, contract URL
  returned 401. The API key is stale, revoked, or wrong. This is a
  real governance violation: we *think* we have a contract and the
  provider disagrees.
- `contract_expired` (Error) — contract fetched successfully, but
  `contract.expires_at` is in the past. Governance must renew.

### Public manifest v0.1 migration path

Today's `public/.well-known/composit.json` on `nuetzliche.it`
(committed yesterday) is richer than v0.1 allows — it carries
`tools`, `description`, `repo`, `license`. Migration:

1. Keep those fields **in the product's own** `.well-known/composit.json`
   (croniq, hookaido, powerbrain publish them on their own hosts).
2. Replace the org manifest entries with the minimal v0.1 shape.
3. Add a `contracts[]` array pointing at `https://nuetzliche.it/contract`
   (or wherever the contract endpoint lands).

The aggregator script in `nuetzliche.it/scripts/generate-composit-manifest.mjs`
needs one adjustment:
- Swap `ENRICH_FIELDS` from `[type, product, protocol, tools, description, repo, license]`
  to `[type, product, protocol]`.
- Remote product manifests may still carry rich fields; the aggregator
  just refuses to copy them into the org tier.

### Relationship to RFC 001

RFC 001 (composit-report format) is untouched in normative shape.
Two additive clarifications:

- The `providers[]` entry in a report gains an optional
  `auth_mode: "public" | "contract" | "unreachable"` field. Optional
  and `additionalProperties` is already open on Provider; documented
  in the RFC 001 changelog.
- `examples/composit-manifest.json` is retitled a **Public** manifest
  example. A companion `examples/composit-contract.example.json`
  will ship once RFC 003 lands.

## Open questions

1. **Multi-identity per provider.** A team with dev + prod identities
   against the same provider repeats the whole `provider` block today.
   Is that acceptable (simple, explicit) or should the Compositfile
   grow a list of `auth` blocks per provider? Deferred — first see
   how many teams actually hit this.

2. **Contract cache lifetime.** Contracts change rarely but
   occasionally (rate-limit upgrade, endpoint migration). Should the
   contract response carry a `cache_ttl` hint that `composit scan`
   respects? Or is "refetch on every scan" good enough? Tentative
   answer: refetch every scan, add `cache_ttl` when scan volume
   becomes a measured problem.

3. **Tier visibility in the public manifest.** May a provider list
   multiple `contracts[]` entries corresponding to different
   tiers ("free", "team", "enterprise") so prospects can see the
   menu before signup? Or does that leak commercial structure?
   Recommendation: allow it, make it optional. A minimalist provider
   declares one contract URL; a marketing-forward provider declares
   three.

4. **Contract discovery from outside the Compositfile.** Composit's
   CLI always reads Compositfile to know which provider to contact.
   What about ad-hoc `composit inspect https://provider.example`
   flows? Left for a later Inspect/Adopt RFC — v1 assumes the
   Compositfile is the entry point.

5. **Extension prefix for identity fields.** Today's `nuetzliche`
   manifest uses `x-manifest-url` in the base config (build-time
   aggregation hint, not wire-format). Should that be moved to an
   `x-composit-aggregation.source_url` nested field so extensions
   group cleanly? Cosmetic — track with RFC 001 v0.2.

## Reference implementation

- **Public manifest:** `examples/composit-manifest.json` (to be
  updated in the same PR that lands this RFC if approved).
- **Schema:** `schemas/composit-provider-manifest-v0.1.json` (shipped
  in the RFC PR as Draft; promoted to Proposed when the first
  external provider implements it).
- **Aggregator script:** `nuetzliche.it/scripts/generate-composit-manifest.mjs`
  — already supports per-capability overlay; will drop the
  enrichment of rich fields as part of the migration.
- **CLI changes** (composit):
  - `src/core/types.rs`: add `auth_mode` to `Provider`.
  - `src/core/compositfile.rs`: parse `auth { type, env }` sub-block.
  - `src/scanners/mcp_provider.rs`: implement contract fetch path
    conditional on `trust = "contract"` + credential presence.
  - `src/commands/diff.rs`: new rules `contract_auth_missing`,
    `contract_auth_mismatch`, `contract_unreachable`,
    `contract_unauthorized`, `contract_expired`.

Implementation sequencing: RFC approval → schema merged → aggregator
migration → CLI changes. The CLI can land its plumbing behind an
opt-in `auth` block without breaking existing Compositfiles that omit
it.

## Changelog

- **2026-04-19** — Initial draft (v0.1). Two-URL split, API-key v1,
  OAuth2 on roadmap.
