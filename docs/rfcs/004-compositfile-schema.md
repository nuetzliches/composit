# RFC 004 — Compositfile HCL Schema v0.1

- **Status:** Draft
- **Authors:** nuetzliches/composit maintainers
- **Created:** 2026-04-22
- **Discussion:** _(link to GitHub Discussion when published)_

## Summary

Defines the canonical schema for the `Compositfile` — the HCL governance
document that expresses an operator's SHOULD-state. Establishes:

1. A formal, block-level schema for every top-level and nested block
2. Attribute types, cardinality, default values, and validation rules
3. Trust/auth invariants that the parser enforces at load time
4. An annotated, full-featured reference example

This RFC is the companion to RFC 001 (composit-report format). RFC 001
documents the IS-state produced by `composit scan`; this RFC documents
the SHOULD-state that `composit diff` validates against.

## Motivation

The Compositfile schema is currently defined implicitly by the Rust parser
in `src/core/compositfile.rs`. That's fine internally but gives third
parties — IDE plugins, CI helpers, schema validators, other CLI
implementations — no stable contract to target.

Documenting the schema as an RFC:

- Lets tool authors validate Compositfiles before running the CLI.
- Gives authors a single authoritative reference instead of reading Rust.
- Establishes a versioning baseline so breaking changes are deliberate and
  visible.

## Non-goals

- **Not** the composit-report format. That is RFC 001.
- **Not** the provider manifest format. That is RFC 002.
- **Not** the OPA/Rego policy language. Rego is the policy language; this
  RFC only covers how a policy _file_ is referenced from a Compositfile.
- **Not** a JSON Schema. The format is HCL; a JSON Schema overlay is a
  future concern.
- **Not** a network protocol or transport specification.

## Design

### File location and name

The parser looks for a file literally named `Compositfile` (no extension)
in the scan root. This is the only supported name in v0.1. Alternative
paths (e.g. `composit/Compositfile`) are a future extension.

### Top-level structure

A Compositfile contains exactly one `workspace` block at the top level.
Everything else is nested inside it.

```hcl
workspace "<name>" {
  # zero or more: provider, budget, policy, resources, scan
}
```

The `workspace` label is a free-form string that identifies the
infrastructure context (e.g. a team, a product, or a deployment
environment). It appears in `composit diff` output and in the generated
report's `workspace` field.

**Required:** yes — the parser rejects a Compositfile with no `workspace`
block.

---

### Block: `provider`

Declares an approved AI provider. Any provider found in a scan report that
is not listed in the Compositfile is flagged as an `unapproved_provider`
violation by `composit diff`.

```hcl
provider "<name>" {
  manifest   = "<url>"
  trust      = "public" | "contract"
  compliance = ["<tag>", ...]     # optional
  auth {                          # required when trust = "contract"
    type = "api-key"
    env  = "<ENV_VAR_NAME>"       # optional
  }
}
```

**Cardinality:** zero or more `provider` blocks per `workspace`.

#### Attributes

| Attribute    | Type             | Required | Description |
|--------------|------------------|----------|-------------|
| `manifest`   | string (URL)     | yes      | URL of the provider's `.well-known/composit.json` manifest. |
| `trust`      | string (enum)    | yes      | Trust level. Valid values: `"public"`, `"contract"`. See below. |
| `compliance` | list of strings  | no       | Compliance tags asserted by this provider (e.g. `"gdpr"`, `"eu-ai-act"`). No controlled vocabulary in v0.1; values are stored and surfaced in reports. |

#### Trust levels

| Value        | Meaning |
|--------------|---------|
| `"public"`   | Only the unauthenticated `.well-known/composit.json` manifest is read. No credential is required or expected. `auth` block MUST NOT be present. |
| `"contract"` | composit fetches both the public manifest and an authenticated contract manifest. `auth` block is required. |

Mixing `trust = "public"` with an `auth` block is a parse-time error.
Declaring `trust = "contract"` without an `auth` block is a parse-time
error.

#### Nested block: `auth`

Required when `trust = "contract"`. Describes how composit should obtain
a credential at scan time.

| Attribute | Type          | Required | Description |
|-----------|---------------|----------|-------------|
| `type`    | string (enum) | yes      | Auth method. Valid in v0.1: `"api-key"`. `"oauth2"` is on the RFC 002 roadmap but rejected by the parser. |
| `env`     | string        | no       | Name of the environment variable that holds the credential. When omitted, composit cannot authenticate; `composit diff` surfaces a `contract_auth_missing` diagnostic. |

composit never reads a secret from the tracked file itself. `env` is a
level of indirection: the credential lives in the environment, not the
repository.

---

### Block: `budget`

Declares a spending limit for a named scope. `composit diff` compares
`max_monthly` against `estimated_monthly_cost` in the scan report.

```hcl
budget "<scope>" {
  max_monthly = "<amount>"
  alert_at    = "<percentage>%"   # optional
}
```

**Cardinality:** zero or more per `workspace`.

#### Attributes

| Attribute     | Type          | Required | Description |
|---------------|---------------|----------|-------------|
| `max_monthly` | string        | yes      | Maximum allowed monthly cost. Free-form in v0.1 (e.g. `"500 EUR"`, `"$200"`). Compared as a string label against the report. |
| `alert_at`    | string (`N%`) | no       | Threshold at which composit emits a budget alert. Must be a percentage between `0%` and `100%` inclusive (e.g. `"80%"`). The parser rejects values outside this range or without the `%` suffix at load time. |

Budget scopes are free-form labels (e.g. `"workspace"`, `"per-agent-session"`).
Multiple budget blocks with different scopes are all validated independently.

---

### Block: `policy`

References an OPA/Rego policy file. `composit diff` loads the file, parses
it, and evaluates `deny` and `allow` rules against the scan report.

```hcl
policy "<name>" {
  source      = "<path/to/policy.rego>"
  description = "<human-readable description>"   # optional
}
```

**Cardinality:** zero or more per `workspace`.

#### Attributes

| Attribute     | Type   | Required | Description |
|---------------|--------|----------|-------------|
| `source`      | string | yes      | Path to the Rego policy file, relative to the Compositfile's directory. |
| `description` | string | no       | Human-readable description shown in `composit diff` output. |

#### Policy evaluation semantics

When `composit diff` loads a policy it:

1. Parses the Rego source and detects whether the package exports a
   `deny` partial set rule and/or a default `allow` boolean.
2. Serializes the full scan report as JSON and sets it as `input`.
3. If `deny` exists: evaluates `data.<package>.deny`; each string in the
   result set becomes a `policy_violation` error.
4. If only `allow` exists: evaluates the query as a boolean; `false`
   becomes a `policy_not_allowed` error.
5. If neither pattern is present: records a `policy_parsed` info
   diagnostic (the policy was loaded but has no enforceable rules).

The input shape available to Rego policies mirrors the scan report:

```rego
input.workspace     # string — workspace name
input.resources     # array of resource objects
input.summary       # summary object
```

Each resource in `input.resources` has a `type` field (e.g. `"docker_service"`)
and flattened scanner-specific fields as top-level keys (e.g. `input.resources[i].image`).

---

### Block: `resources`

Declares constraints on what resource types are allowed to exist and in
what quantities.

```hcl
resources {
  max_total = <integer>   # optional

  allow "<resource_type>" {
    max            = <integer>    # optional
    allowed_images = ["<img>"]   # optional
    allowed_types  = ["<type>"]  # optional
  }

  require "<resource_type>" {
    min = <integer>   # optional, default 1
  }
}
```

**Cardinality:** zero or one `resources` block per `workspace`.

#### Attributes

| Attribute   | Type    | Required | Description |
|-------------|---------|----------|-------------|
| `max_total` | integer | no       | Hard cap on the total number of resources across all types. |

#### Nested block: `allow`

Declares a resource type as explicitly permitted. The label is the
canonical resource type string (e.g. `"docker_service"`, `"workflow"`).

**Allowlist semantics:** if at least one `allow` block is present, any
resource type found in the scan report that is _not_ listed is a
`resource_type_not_allowed` violation. If no `allow` blocks are present,
all types are implicitly permitted.

| Attribute        | Type            | Required | Description |
|------------------|-----------------|----------|-------------|
| `max`            | integer         | no       | Maximum number of resources of this type. Exceeding it produces a `resource_limit_exceeded` violation. |
| `allowed_images` | list of strings | no       | For `docker_service` resources: a list of permitted image name prefixes. Resources with images not matching any prefix produce a `disallowed_image` violation. |
| `allowed_types`  | list of strings | no       | For resource types that carry a sub-type field: permitted sub-type values. |

**Cardinality:** zero or more per `resources` block.

#### Nested block: `require`

Declares that a resource type must have at least `min` instances in the
scan report. A missing or under-represented type produces a
`required_resource_missing` violation.

| Attribute | Type    | Required | Description |
|-----------|---------|----------|-------------|
| `min`     | integer | no       | Minimum required count. Defaults to `1`. |

**Cardinality:** zero or more per `resources` block.

---

### Block: `scan`

Configures scanner behaviour: which paths to skip, which custom file
patterns to surface, and which built-in scanners to disable. An empty
`scan { }` block is a valid no-op. Omitting `scan` entirely is equivalent.

```hcl
scan {
  exclude = ["<path>", ...]   # optional

  extra_patterns {
    type        = "<resource_type>"
    glob        = "<glob>"
    description = "<text>"    # optional
  }

  scanners {
    <scanner_id> = false      # disable a built-in scanner
    <scanner_id> = true       # explicitly re-enable (redundant, valid)
  }
}
```

**Cardinality:** zero or one per `workspace`.

#### Attribute: `exclude`

A list of path patterns to skip during filesystem traversal. Evaluated
relative to the scan root (the directory containing the Compositfile).

- Bare directory names (e.g. `"tests/fixtures"`) match everything under
  that directory (`tests/fixtures/**`).
- Patterns containing glob metacharacters (`*`, `?`, `[`) are used
  verbatim (e.g. `"**/*.generated.yaml"`).

| Attribute | Type            | Required | Description |
|-----------|-----------------|----------|-------------|
| `exclude` | list of strings | no       | Path patterns to exclude from the scan. |

#### Nested block: `extra_patterns`

Defines a custom file pattern that composit should surface as an ad-hoc
resource type. Useful for domain-specific files that no built-in scanner
handles.

| Attribute     | Type   | Required | Description |
|---------------|--------|----------|-------------|
| `type`        | string | yes      | The resource type string to assign matched files (e.g. `"terraform_module"`). |
| `glob`        | string | yes      | A glob pattern (e.g. `"modules/**/*.tf"`) matched against paths relative to the scan root. |
| `description` | string | no       | Human-readable description of what this pattern finds. |

**Cardinality:** zero or more per `scan` block.

#### Nested block: `scanners`

A key-value map from scanner ID to boolean that overrides the default
enabled state of built-in scanners. Missing keys default to `true`
(enabled). Attributes must be boolean literals.

```hcl
scanners {
  prometheus = false   # disable the Prometheus scanner
  docker     = true    # explicit re-enable (same as omitting)
}
```

**Cardinality:** zero or one per `scan` block.

---

### Parse-time validations

The parser enforces the following rules and produces a fatal error on
violation:

| Rule | Error condition |
|------|-----------------|
| Missing `workspace` block | No `workspace` block at the top level. |
| Missing `manifest` | `provider` block has no `manifest` attribute. |
| Missing `trust` | `provider` block has no `trust` attribute. |
| Contract without auth | `trust = "contract"` and no `auth` block. |
| Public with auth | `trust = "public"` and an `auth` block present. |
| Unknown auth type | `auth.type` is not `"api-key"` (v0.1). |
| Missing budget amount | `budget` block has no `max_monthly`. |
| Invalid `alert_at` | Not a number followed by `%`, or outside `0–100`. |
| Missing policy source | `policy` block has no `source`. |
| Missing `extra_patterns.type` | `extra_patterns` block has no `type`. |
| Missing `extra_patterns.glob` | `extra_patterns` block has no `glob`. |

Unknown block types inside `workspace` or `scan` emit a warning to stderr
but do not fail the parse. This forward-compatibility rule lets newer CLI
versions add blocks while older ones gracefully degrade.

---

### Annotated full example

```hcl
# Compositfile — production workspace governance
#
# IS-state: generated by `composit scan`  → composit-report.yaml
# SHOULD-state: this file                 → `composit diff` compares them

workspace "production" {

  # --- Scan Tuning ---
  # Exclude generated and test directories from the inventory.
  # Custom patterns surface domain-specific files as first-class resources.

  scan {
    exclude = [
      "tests/fixtures",
      "examples",
      "**/*.generated.yaml",
    ]

    extra_patterns {
      type        = "terraform_module"
      glob        = "modules/**/*.tf"
      description = "Internal Terraform modules"
    }

    scanners {
      prometheus = false   # no Prometheus in this workspace
    }
  }

  # --- Providers ---
  # Approved AI providers. Unlisted providers found in the scan are violations.
  # trust="public":   no credential needed; public manifest only.
  # trust="contract": auth block required; composit fetches the contract manifest.

  provider "acme-llm" {
    manifest   = "https://acme.example.com/.well-known/composit.json"
    trust      = "contract"
    compliance = ["gdpr", "eu-ai-act"]
    auth {
      type = "api-key"
      env  = "ACME_COMPOSIT_KEY"
    }
  }

  provider "open-models" {
    manifest = "https://openmodels.example.com/.well-known/composit.json"
    trust    = "public"
  }

  # --- Budgets ---
  # Compared against estimated_monthly_cost in the scan report.

  budget "workspace" {
    max_monthly = "500 EUR"
    alert_at    = "80%"
  }

  budget "per-session" {
    max_monthly = "50 EUR"
    # no alert_at — only hard cap enforced
  }

  # --- Resource Constraints ---
  # At least one `allow` block → allowlist mode: unlisted types are violations.
  # No `allow` blocks           → permissive mode: all types implicitly allowed.

  resources {
    max_total = 200

    allow "docker_compose" {
      max = 10
    }

    allow "docker_service" {
      max            = 50
      allowed_images = ["ghcr.io/acme/", "docker.io/library/"]
    }

    allow "workflow" {
      max = 20
    }

    allow "terraform_config" {
      max = 10
    }

    allow "terraform_module" {
      max = 30
    }

    # CI workflow must always be present.
    require "workflow" {
      min = 1
    }

    # At least one Docker Compose file must exist.
    require "docker_compose" {
      min = 1
    }
  }

  # --- Policies ---
  # OPA/Rego policies evaluated against the scan report by `composit diff`.

  policy "image-pinning" {
    source      = "policies/image-pinning.rego"
    description = "All Docker images must use a pinned tag (not :latest)"
  }

  policy "data-residency" {
    source      = "policies/data-residency.rego"
    description = "All data must remain in EU regions"
  }
}
```

---

## Open questions

1. **Multiple workspace blocks** — Should a single Compositfile be allowed
   to declare more than one `workspace` block (e.g. for monorepos with
   multiple deployment targets)? The current parser requires exactly one.
   Pro: single-file multi-target governance. Con: complicates `composit
   diff --workspace` selection.

2. **Schema version declaration** — Should the Compositfile carry an
   explicit `schema_version` attribute (e.g. at the root or inside
   `workspace`)? Currently there is no version field; schema identity is
   inferred from the CLI version. This mirrors the report format question
   in RFC 001 § Open questions.

3. **`auth.type = "oauth2"`** — OAuth2 is reserved on the RFC 002 roadmap
   but the parser rejects it today. When should it be promoted, and what
   additional attributes does it require (`token_url`, `client_id`,
   `client_secret_env`)?

4. **Allowlist granularity for `allowed_images`** — Currently a prefix
   match. Should it support exact pinning (`ghcr.io/acme/api@sha256:…`)
   or full glob syntax for more expressive policies?

5. **`max_monthly` currency** — The field is a free-form string. Multi-
   currency workspaces cannot currently be cross-compared. Should the
   format adopt a canonical `"<number> <ISO-4217>"` syntax and parse it?

6. **`scan.extra_patterns` ordering** — Multiple `extra_patterns` blocks
   with overlapping globs produce one resource per matched file per
   pattern. Should we document explicit deduplication or priority rules?

7. **Conditional blocks** — Can governance rules vary by branch, tag, or
   environment? The current schema has no conditional syntax. A future
   extension might borrow HCL's `dynamic` block or introduce an `env`
   label on `budget`/`resources`.

## Reference implementation

- Parser: [`src/core/compositfile.rs`](../../src/core/compositfile.rs)
- Governance types: [`src/core/governance.rs`](../../src/core/governance.rs)
- Consumer: `composit diff` (`src/commands/diff.rs`)
- Canonical examples: [`examples/Compositfile`](../../examples/Compositfile),
  [`Compositfile`](../../Compositfile) (the CLI project's own governance)
- OPA evaluation: [`src/core/opa_eval.rs`](../../src/core/opa_eval.rs)

## Changelog

- **2026-04-22** — Initial draft (v0.1).
