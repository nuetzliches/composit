# RFC 005 — Compositfile Resource Roles & Matchers

- **Status:** Draft
- **Authors:** nuetzliches/composit maintainers
- **Created:** 2026-04-24
- **Extends:** [RFC 004 — Compositfile HCL Schema v0.1](004-compositfile-schema.md)
- **Discussion:** _(link to GitHub Discussion when published)_

## Summary

Extends the `resources.allow "<type>" { … }` block (RFC 004 §Block: `resources`) with a nested `role "<name>" { … }` sub-block. A role selects a subset of resources by matcher predicates and attaches stricter per-role constraints (image pins, required ports, required networks, env-var presence, …).

This closes the gap between "you may have up to N docker_services" (coarse) and "every service must be listed verbatim" (unmaintainable) by letting operators govern resources *by role*, not *by instance*.

## Motivation

Compositfile v0.1 governs resource collections only at the type level: `max`, `allowed_images`, `allowed_types`. That is too blunt for real infrastructure:

- `allowed_images = ["postgres:*", "ghcr.io/acme/*"]` permits *any* service to use *any* of those images. It cannot express "the database service must pin `postgres:16`, but the app service must come from our registry."
- There is no way to require that a service exposes specific ports, attaches to specific networks, or reads specific env vars.
- Renaming a service counts as "no change" to governance, even if it flips roles (e.g. `postgres` → `mysql`).

A real .NET-plus-Python monolith with Ansible-driven deploys produces ~80 scanned resources across seven types. Governed only by seven `max` values, the Compositfile validates *counts*, not *correctness*: swapping an image family or changing a port exposure slides through unchallenged.

Per-instance configuration (`resource docker_service "postgres" { … }`) would fix expressiveness but turn the Compositfile into a duplicate of the scan report. Agent-driven infrastructure rename services often; hard-pinning every instance name creates churn violations without real signal.

Roles strike the middle ground: the Compositfile says *what kinds of services should exist and what rules they obey*, matchers say *which scan resources fill that kind*.

## Non-goals

- **Not** per-instance configuration. A role groups resources by pattern; it is not a named singleton.
- **Not** a replacement for `allow.allowed_images` / `allow.allowed_types`. Those remain as type-level defaults; roles tighten per subset.
- **Not** a general rule engine. Complex cross-resource logic (e.g. "if service A exists then service B must exist") stays in OPA/Rego via the existing `policy` block.
- **Not** a template system. Roles do not generate expected resources; they describe what observed resources must satisfy.

## Design

### Block: `role`

A `role` block lives inside an `allow "<type>"` block (RFC 004 §Nested block: `allow`).

```hcl
allow "docker_service" {
  max            = 40
  allowed_images = ["postgres:*", "ghcr.io/acme/*", "git.example.com/*"]

  role "database" {
    match {
      name  = ["*postgres*", "*db*"]
      image = ["postgres:*"]
    }
    image_pin      = ["postgres:16", "postgres:17"]
    must_expose    = [5432]
    must_attach_to = ["backend"]
  }

  role "api" {
    match {
      name = ["*-api", "api-*"]
    }
    image_prefix   = ["git.example.com/acme/"]
    must_set_env   = ["DATABASE_URL", "OTEL_ENDPOINT"]
  }
}
```

**Cardinality:** zero or more `role` blocks per `allow` block.

### Matchers — the `match` sub-block

The `match` block selects which resources belong to the role. All attributes are **optional**; a `match {}` with no attributes selects every resource of the parent `allow` type.

| Attribute  | Type              | Matches against |
|------------|-------------------|-----------------|
| `name`     | list of patterns  | `resource.name` |
| `image`    | list of patterns  | `resource.extra.image` (docker_service) |
| `path`     | list of patterns  | `resource.path` (glob relative to workspace root) |
| `label`    | map of patterns   | `resource.extra.labels` (future) |

**Pattern semantics:** each list entry is a glob with `*` and `?` wildcards (same semantics as `allowed_images`). Within an attribute, patterns are OR-ed (`name = ["*db*", "*postgres*"]` matches either). Across attributes, patterns are AND-ed (`name` AND `image`).

**Predicate attribute** (optional):

```hcl
match {
  name     = ["*"]
  predicate = "any"   # default: "all"
}
```

- `"all"` (default): all attribute patterns must match.
- `"any"`: at least one attribute pattern must match.

### Role constraints

Constraints are declared as top-level attributes inside the `role` block. Each one produces a distinct violation rule (see §Violation catalog below). All constraints are optional.

| Attribute         | Type            | Applies to       | Violation rule |
|-------------------|-----------------|------------------|----------------|
| `image_pin`       | list of strings | docker_service   | `role_image_not_pinned` |
| `image_prefix`    | list of strings | docker_service   | `role_image_prefix_mismatch` |
| `must_expose`     | list of ints    | docker_service   | `role_port_missing` |
| `must_attach_to`  | list of strings | docker_service   | `role_network_missing` |
| `must_set_env`    | list of strings | env_file         | `role_env_var_missing` |
| `must_have_file`  | list of globs   | any              | `role_file_missing` |
| `forbidden_env`   | list of strings | env_file         | `role_env_var_forbidden` |
| `min_count`       | integer         | any              | `role_count_below_min` |
| `max_count`       | integer         | any              | `role_count_above_max` |

Attributes that do not apply to a resource type are silently ignored for resources of the "wrong" type within that role. Rationale: roles are declared inside an `allow "<type>"` block, so the type is already scoped; the list above documents which attributes are *meaningful* for which types.

### Match semantics

**Cumulative (AND) across roles.** A resource may match more than one role; *every* matching role's constraints apply. Example:

```hcl
role "database" { match { name = ["*db*"] } must_attach_to = ["backend"] }
role "persistent" { match { image = ["postgres:*", "mysql:*"] } must_have_file = ["backups/*.sql"] }
```

A service `postgres-db` with image `postgres:16` matches both roles and must satisfy both `must_attach_to = ["backend"]` and `must_have_file = ["backups/*.sql"]`.

**Unmatched resources.** Resources that match *no* role inside their `allow` block are only subject to the `allow`-level constraints (`max`, `allowed_images`, `allowed_types`). This preserves the forward-compatibility story: adding a role does not suddenly break resources that the role does not target.

**Role-exclusive mode** (future extension, not in v0.1): an `allow` block could declare `role_required = true` to reject any resource that matches no role.

### Violation catalog

New violation rules emitted by `composit diff`. All carry the `expected`/`actual` fields introduced in the HTML diff renderer so the tabular view renders correctly.

| Rule                          | Severity | `expected` field contents              | `actual` field contents            |
|-------------------------------|----------|----------------------------------------|------------------------------------|
| `role_image_not_pinned`       | Error    | list of allowed pins, one per line     | observed image string              |
| `role_image_prefix_mismatch`  | Error    | list of allowed prefixes               | observed image string              |
| `role_port_missing`           | Error    | required port list                     | observed port list                 |
| `role_network_missing`        | Error    | required network list                  | observed network list              |
| `role_env_var_missing`        | Error    | required env var names                 | observed env var names             |
| `role_env_var_forbidden`      | Error    | `forbidden` list                       | observed env var names             |
| `role_file_missing`           | Warning  | required file glob(s)                  | `(not found)`                      |
| `role_count_below_min`        | Error    | `≥ <min>`                              | observed count                     |
| `role_count_above_max`        | Error    | `≤ <max>`                              | observed count                     |

Each violation's `details` field names the role (`role: "<name>"`) and, where applicable, the offending resource path.

### Parse-time validations (additions to RFC 004 §Parse-time validations)

| Rule | Error condition |
|------|-----------------|
| Empty role label | `role ""` |
| Duplicate role label within one `allow` block | Two `role "api"` under the same `allow` |
| Unknown predicate value | `match.predicate` is not `"all"` or `"any"` |
| `match` references unknown attribute | e.g. `match { nope = ["*"] }` — warn; forward-compat |
| Port attribute is not a positive integer | `must_expose = [-1]` |

### Interaction with `allowed_images` (type-level)

`allow.allowed_images` remains type-level and is checked *independently*. If a role's `image_pin` is stricter than `allowed_images`, both checks run — the tighter one wins because any violation, at either level, fails the diff. The two are not expected to disagree in practice; a common pattern is a broad `allowed_images` on `allow` plus a stricter `image_pin` per role.

### Migration

- **Existing Compositfiles** (without `role` blocks) continue to validate unchanged. Roles are additive.
- **`composit init`** (v0.3+) MAY emit commented-out `role` stubs for common patterns (one per detected image family), seeded from the scan. The default output remains `max`-only to avoid prescribing structure.
- **Schema file** (`schemas/composit-v0.2.hcl` or equivalent) adds the `role` block; the v0.1 schema remains available. The feature lives behind a Compositfile version bump (`workspace "<name>" { version = "0.2" … }`) once the HCL schema introduces versioning.

### Open questions

1. **Glob vs regex for `name` matching.** Glob is simpler; regex covers more cases. Vote: glob for v0.2, regex as future opt-in via `name_regex`.
2. **Role inheritance.** Should a role extend another? Probably not for v0.2 — keeps the data model flat and matchers explicit.
3. **Role-exclusive mode.** See §Match semantics above. Propose for v0.3 once role coverage patterns stabilise.
4. **Per-role cost estimation.** Could we attach a cost budget per role? Deferred — budget is a separate block today (RFC 004) and the scoping is workspace-level. Revisit when cost estimation improves.

## Annotated example

```hcl
workspace "acme-platform" {

  resources {
    max_total = 100

    allow "docker_service" {
      max = 40

      role "internal-api" {
        match { name = ["*-api", "api-*"] }
        image_prefix   = ["git.acme.example/acme/"]
        must_expose    = [8080]
        must_attach_to = ["backend"]
      }

      role "database" {
        match {
          name      = ["*postgres*", "*mssql*", "*sqlserver*"]
          image     = ["postgres:*", "mcr.microsoft.com/mssql/*"]
          predicate = "any"
        }
        image_pin = ["postgres:16", "mcr.microsoft.com/mssql/server:2022-latest"]
      }

      role "search-index" {
        match { image = ["docker.elastic.co/elasticsearch/*"] }
        image_prefix   = ["docker.elastic.co/elasticsearch/"]
        must_attach_to = ["search", "search-dev", "search-staging"]
      }

      role "monitoring" {
        match { path = ["monitoring/**"] }
        must_attach_to = ["monitoring"]
      }
    }

    allow "env_file" {
      max = 42

      role "production-env" {
        match { path = ["**/.env.production", "ansible/**/production/**"] }
        forbidden_env = ["DEBUG", "VERBOSE"]
      }
    }

    allow "workflow" {
      max = 10
    }
  }
}
```

This Compositfile says:

- There may be up to 40 docker services in total.
- Services whose name matches `*-api` / `api-*` must come from the internal registry, expose port 8080, and join the `backend` network.
- Database services (matched by name *or* image family) must pin to a supported major version.
- Search-index services must use the official Elasticsearch image family and attach to one of the environment-specific networks.
- Monitoring stack services (matched by path prefix) must attach to the `monitoring` network.
- Production-scope env files must not define `DEBUG` or `VERBOSE`.

Everything else (workflows, dockerfiles, other services) remains governed only by type-level counts — no role overhead where none is needed.

## Changelog

- **2026-04-24** — Initial draft.
