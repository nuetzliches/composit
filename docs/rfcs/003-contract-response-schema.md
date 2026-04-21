# RFC 003 — Contract Manifest: Response Schema

- **Status:** Draft
- **Authors:** nuetzliches/composit maintainers
- **Created:** 2026-04-20
- **Schema (v0.1):** `schemas/composit-contract-response-v0.1.json`
- **Depends on:** RFC 002 (Provider Manifest: Public + Contract Tiers)

## Summary

Formalizes the JSON response shape returned by a provider's authenticated
contract URL — the other half of the two-tier model introduced in RFC
002. The public manifest already advertises `contracts[].url`; this RFC
defines what a client can expect to parse at that URL once a valid
credential is attached.

The response is **personalized per identity** (RFC 002 §Content tiers),
so the schema describes the allowed *shape* of a contract response, not
its values. v0.1 requires four fields inside a single `contract` object
and leaves the rest optional so providers can grow their responses
without a schema bump.

## Motivation

Two concrete gaps block useful contract-tier behaviour today:

1. **`contract_expired` is a dead wire.** `composit diff` declares the
   rule (`src/commands/diff.rs` ≈ line 257) but has nothing to compare
   against because the contract response body is unspecified. Without a
   standard field, the scanner can't tell whether a contract is still
   valid.
2. **Provider implementers have no target to aim at.** RFC 002 shipped
   an illustrative example but no schema. Anyone building the "second
   composit-compatible provider" can't answer "what do I return from
   my contract URL?" by reading a document.

RFC 003 closes both gaps by pinning a minimum response envelope while
preserving room for provider-specific extensions.

## Non-goals

- **Not a pricing protocol.** `contract.pricing_tier` is a label (e.g.
  `"free"`, `"team"`, `"enterprise"`). Actual prices, billing cycles,
  invoicing details stay out-of-band.
- **Not a tool-call protocol.** `capabilities[*].endpoint` tells a
  client *where* the capability lives; how to talk to it (MCP, HTTP,
  gRPC) is owned by the capability's own protocol.
- **Not multi-tier negotiation.** RFC 002 Open Question #3 (multiple
  parallel contracts for free/team/enterprise) is left open. A single
  contract response represents the tier the identity already holds.

## Design

### Response envelope

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

Top-level required fields: `composit`, `contract`. `capabilities` and
`sla` are optional but widely expected.

### `contract` object — the governance surface

This object is what `composit diff` reasons about. v0.1 required
fields:

| Field | Type | Meaning |
|-------|------|---------|
| `id` | string | Stable identifier for this contract-identity pair. Free-form; providers typically encode issue year, provider name, and an internal sequence. Opaque to the consumer. |
| `provider` | string | Canonical provider name — MUST match the `provider.name` of the public manifest that pointed here. Guards against misrouted responses. |
| `issued_at` | string (ISO 8601, UTC) | When this contract relationship started. Informational; no rule depends on it in v0.1. |
| `expires_at` | string (ISO 8601, UTC) | When the contract becomes invalid. `composit diff` MUST emit `contract_expired` (Error) when this timestamp is in the past. |

Optional:

| Field | Type | Meaning |
|-------|------|---------|
| `pricing_tier` | string | Short label identifying the identity's tier (`free`, `team`, `enterprise`, provider-defined). Surfaces in reports for operational visibility; no rule depends on it in v0.1. |

All timestamp strings are ISO 8601 with an explicit `Z` offset (UTC).
`+00:00` is accepted; other offsets are allowed by the schema but
discouraged — providers SHOULD normalize to UTC so `composit diff` on
different workstations draws the same conclusion.

### `capabilities[]` — what the identity can actually call

Extends the public-tier `PublicCapability` shape (from the RFC 002
schema) with concrete call-time metadata. Required fields: `type` (same
semantics as public). Providers SHOULD include `product` and `protocol`
when they were advertised publicly, so a consumer can correlate entries
across tiers by `(type, product)` pair.

Additional v0.1 fields, all optional:

| Field | Type | Meaning |
|-------|------|---------|
| `endpoint` | string (URI) | Concrete base URL the identity should call. Personalized: a shared-cluster caller and a dedicated-instance caller see different URLs at the same contract URL. |
| `tools` | integer ≥ 0 | Size of the tool inventory the identity can use. Providers MAY publish the full list in future versions; v0.1 sticks to a count. |
| `rate_limit` | object | See below. |
| `regions` | array of string | Regions the identity is allowed to route through. Same notation as the public manifest's `region` field. |

`rate_limit` shape (all optional, at least one required if the field is
present):

```json
{ "requests_per_minute": 120, "requests_per_hour": 5000, "burst": 20 }
```

This is a *declaration* the identity can rely on — not an enforcement
contract. Enforcement is a provider-side concern (RFC 003 is about
governance visibility, not runtime policing).

### `sla` — optional service-level claim

```json
{
  "uptime_pct": 99.5,
  "incident_contact": "sre@provider.example",
  "response_time_ms_p99": 800
}
```

All fields optional. Consumers surface these in `composit status
--live` but don't make governance decisions on them in v0.1.

### Unknown fields

The schema sets `additionalProperties: true` at every level. Providers
MAY embed vendor-specific keys without a schema bump. Consumers MUST
ignore fields they don't recognize — this is the same contract the
MCP and OpenAPI ecosystems use.

Providers that need structured extension SHOULD prefix keys with `x-`,
matching the convention from RFC 001 §Extension fields.

### Expiry semantics

`contract_expired` fires when `contract.expires_at` parses as a valid
ISO-8601 timestamp *and* that timestamp is before the machine's current
UTC clock. The rule does **not** fire when:

- `expires_at` cannot be parsed (that's `invalid_contract_body`).
- The provider omits `expires_at` — violates the v0.1 required-fields
  rule; surfaced as `invalid_contract_body` by the scanner.

The scanner clock is authoritative. This is a deliberate simplification
— governance verdicts should be reproducible from a checked-in report,
so we don't negotiate clock drift with the provider. A provider wanting
to assert their own "now" can do so via a custom extension; v0.1
doesn't read it.

### Personalization & caching

A contract URL returns different JSON for different authenticated
callers. The schema describes the *shape* of any caller's response; the
contents are identity-dependent. Consumers MUST treat responses as
non-cacheable by default.

Providers SHOULD send `Cache-Control: private, no-store` to avoid
intermediary caches conflating identities. Consumers SHOULD refetch on
every `composit scan` unless a future RFC introduces an explicit TTL
hint (RFC 002 Open Question #2 tracks this).

### Mismatch detection

When the contract response arrives, the scanner validates:

1. `composit` version parses (semver-ish).
2. `contract.provider` equals the public manifest's `provider.name`.
3. `contract.expires_at` parses as ISO 8601.

Any failure records `auth_error = "invalid_contract_body"` on the
provider entry and leaves `auth_mode = Public`. `composit diff` then
surfaces this as `contract_unreachable` (Warning) — distinct from an
auth failure because the credential worked but the body was wrong.

### Conformance

**Provider v0.1 conformance:**

- MUST return HTTP 200 with a body matching this schema to valid
  authenticated requests.
- MUST return HTTP 401 or 403 to missing / invalid credentials.
- MUST include `contract.{id, provider, issued_at, expires_at}`.
- SHOULD include a `capabilities[]` array mirroring the public
  manifest's capability types.
- SHOULD set `Cache-Control: private, no-store`.

**Consumer v0.1 conformance:**

- MUST parse `contract.expires_at` and emit `contract_expired` when
  past.
- MUST tolerate unknown fields at every level.
- MUST match `contract.provider` against the public manifest's
  `provider.name`; mismatch is `invalid_contract_body`.
- MAY ignore `capabilities[]`, `sla`, `rate_limit` in v0.1 (composit
  CLI does — see "Relationship to composit CLI v0.1" below).

### Relationship to composit CLI v0.1

This RFC is intentionally wider than what `composit` v0.1 consumes.
The CLI in this release reads only the `contract` object (bookkeeping)
and uses `contract.expires_at` for the diff rule. `capabilities[]`,
`sla`, and `rate_limit` are specified here so external provider
implementers know the target; the CLI will grow into them in a later
minor version.

Providers MAY publish the fuller shape today — unknown fields are
ignored per §"Unknown fields".

### Relationship to RFC 001

RFC 001 (`composit-report.yaml`) is updated additively:

- `providers[]` entries gain an optional `contract` object carrying
  the v0.1 bookkeeping fields (`id`, `issued_at`, `expires_at`,
  `pricing_tier`). Optional, `additionalProperties: false` stays.
- `providers[]` entries gain optional `auth_mode` and `auth_error`
  fields (already emitted by the Rust types; the schema just catches
  up).

RFC 001 schema version stays at v0.1 (additive changes only). A
changelog entry records the addition.

### Relationship to RFC 002

RFC 002 §"Contract Manifest (informative, v0.1)" is superseded in
substance (this RFC is now the normative reference) but kept in place
as historical context. The public-manifest schema and the Compositfile
`auth` block are untouched.

## Open questions

1. **Full tool inventory.** v0.1 publishes a count. Should v0.2 embed
   the full list (names + schemas) so an agent can choose which tools
   to enable without calling `tools/list`? The MCP ecosystem already
   provides that call-time; duplicating in the contract response risks
   drift. Tentative answer: stay with a count in v0.1; revisit if
   consumers actually ask for it.

2. **Contract clock.** v0.1 pins expiry verdicts to the scanner's
   clock. In strictly-synced environments that's fine; in offline CI
   it could diverge. A provider-supplied `server_time` field would let
   consumers detect skew. Deferred — no reported pain.

3. **Revocation before expiry.** A contract can be revoked mid-term
   without waiting for `expires_at`. Today that surfaces as `401` on
   the next contract fetch (`contract_unauthorized`). Should contracts
   carry a `revoked: true` flag so a long-cached response can reflect
   revocation without a refetch? Waiting on actual need.

4. **Multi-contract responses.** If a single identity legitimately
   holds two contracts against the same provider (dev + prod tenants),
   does the URL return one response or an array? RFC 003 v0.1 assumes
   one URL, one response. Multi-identity sugar is tracked in RFC 002
   Open Question #1.

5. **Response-scoped schema versioning.** The response carries
   `"composit": "0.1.0"` — same field as the public manifest. That
   conflates "which spec version produced this response" with "which
   public-manifest version the provider claims conformance with".
   Tentatively fine; revisit if a provider needs to decouple.

## Reference implementation

- **Schema:** `schemas/composit-contract-response-v0.1.json` (shipped
  in this RFC's PR as Draft; promoted to Proposed when the first
  external provider implements it).
- **Example response:** `examples/composit-contract.example.json`.
- **CLI changes** (this iteration):
  - `src/core/types.rs`: add `ContractInfo` struct, append to
    `Provider`.
  - `src/scanners/mcp_provider.rs`: parse the contract body inside
    `upgrade_to_contract`; record `ContractInfo` on success, set
    `auth_error = "invalid_contract_body"` on parse failure.
  - `src/commands/diff.rs`: emit `contract_expired` when
    `contract.expires_at < now`. Route `"invalid_contract_body"` to
    `contract_unreachable` (Warning) in the public-tier branch.
  - `schemas/composit-report-v0.1.json`: additively document
    `auth_mode`, `auth_error`, and `contract` on provider entries.
- **Reference provider:** `nuetzliche.it/contract` is the first
  provider to implement v0.1 of this schema (env-var-map-backed API
  keys, single response template parameterized per identity).

## Changelog

- **2026-04-20** — Initial draft (v0.1). Required: `contract.{id,
  provider, issued_at, expires_at}`; extensive optional surface; CLI
  v0.1 consumes the `contract` object only.
