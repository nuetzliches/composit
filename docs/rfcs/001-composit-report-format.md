# RFC 001 — composit-report v0.1 Format

- **Status:** Draft
- **Authors:** nuetzliches/composit maintainers
- **Created:** 2026-04-18
- **Schema:** [`schemas/composit-report-v0.1.json`](../../schemas/composit-report-v0.1.json)
- **Discussion:** _(link to GitHub Discussion when published)_

## Summary

Defines the canonical on-disk format for `composit-report.yaml` — the
IS-state inventory produced by `composit scan`. Establishes:

1. A formal JSON Schema (Draft 2020-12) for validation and tooling
2. Stable top-level structure: `workspace`, `generated`, `scanner_version`,
   `providers[]`, `resources[]`, `summary`
3. A forward-compatibility rule: scanners MAY add unknown fields to
   individual resources without breaking older consumers

## Motivation

`composit diff` compares an IS-state report against a SHOULD-state
Compositfile. For the comparison to be meaningful across scanner versions
and across foreign implementations (other CLIs, CI plugins, IDE
integrations), the report format needs a published, versioned contract.

Today the format is defined implicitly by the serde types in
`src/core/types.rs`. That's fine internally but not a contract that
external tooling can target. This RFC promotes the implicit shape to an
explicit JSON Schema so third parties can emit or consume reports with
confidence.

## Non-goals

- **Not** a specification of *what* scanners should detect. The canonical
  resource-type list is maintained separately and allowed to grow.
- **Not** the Compositfile format. That is a separate RFC (future work).
- **Not** a network protocol. Reports are local files first; transport is
  the consumer's concern.

## Design

### Top-level shape

```yaml
workspace: <string>             # required
generated: <ISO-8601 UTC>       # required
scanner_version: <semver>       # required
providers: []                   # required, may be empty
resources: []                   # required, may be empty
summary: { ... }                # required
```

All top-level fields are required. `additionalProperties: false` at the
root keeps the format predictable for consumers. Growth happens inside
`resources[]` via type-specific fields (see below), not at the root.

### Resource shape

Every resource carries two required fields:

- `type` — canonical resource type (e.g. `docker_service`, `workflow`)
- `detected_by` — scanner ID that produced it

All other fields are optional but typed:

- `name`, `path`, `provider`, `created`, `created_by`, `estimated_cost`

**Forward compatibility**: `additionalProperties: true` is deliberate on
the Resource object. A `docker_service` resource carries `image`, `ports`,
`volumes`, `networks`; a `workflow` carries `jobs`, `runs_on`, `triggers`.
Rather than building a sealed per-type schema (which would force a
schema release for every new scanner), we keep the extras open-ended.

Consumers that want stronger typing MAY pin a resource-type vocabulary
out-of-band (a future RFC can formalise this if demand emerges).

### Attribution

`created_by` uses a small prefix grammar: `human:<name>` or `agent:<id>`.
This keeps downstream filtering trivial (`starts_with("agent:")`) while
leaving room for new prefixes (`bot:`, `team:`) later.

Resources where no git history is available omit `created_by`; the
summary counts them as `auto_detected`.

### Summary

The summary is computed, not authoritative. It exists to make reports
scannable without parsing the resource list. Consumers that need precise
numbers should compute from `resources[]` directly.

`estimated_monthly_cost` is the aggregate of per-resource
`estimated_cost` values. v0.1 assumes a single currency (EUR) across the
report; mixed-currency reports are an explicit future concern.

### Versioning

This RFC is **v0.1**. Compatibility rules:

- **Minor-version bumps** (v0.2, v0.3) MAY add new required top-level
  fields, but MUST keep existing fields compatible with v0.1 consumers.
- **Major-version bumps** (v1.0) are breaking. A v1.0 release will close
  out the forward-compat rule on resources and publish a per-type
  vocabulary.
- Scanner emits `scanner_version`, NOT schema version. Consumers looking
  for a schema version MAY inspect `$schema` in the generated file, or
  infer from the `scanner_version` mapping published in CHANGELOG.

## Validation example

```bash
# Using jsonschema-cli (pip install jsonschema)
composit scan --output json > report.json
jsonschema -i report.json schemas/composit-report-v0.1.json

# Or using ajv
npx -p ajv-cli ajv validate \
  -s schemas/composit-report-v0.1.json \
  -d report.json
```

A CI job SHOULD validate the canonical example
(`examples/composit-report.yaml`) against the schema to catch schema
drift in PRs.

## Open questions

1. **Report versioning discovery** — Should we mandate a top-level
   `version: "0.1"` field instead of inferring from `scanner_version`?
   Pro: explicit, easy for consumers. Con: couples file format to CLI
   release cadence.

2. **Resource-type vocabulary** — Should we reserve an `x-`
   prefix for experimental types (like HTTP headers) or rely on
   scanner IDs to disambiguate?

3. **Localised costs** — Multi-currency support is deferred. When does
   that become necessary?

4. **Streaming consumption** — For very large workspaces, should we
   offer an NDJSON format alongside YAML/JSON? (Out of scope for v0.1.)

## Reference implementation

- Types: `src/core/types.rs`
- Producer: `composit scan` (writes YAML by default, `--output json`
  available)
- Consumer: `composit diff` + `composit status`
- Example: `examples/composit-report.yaml`

## Changelog

- **2026-04-18** — Initial draft (v0.1).
