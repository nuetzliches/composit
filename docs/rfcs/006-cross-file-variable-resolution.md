# RFC 006 — Cross-File Variable Resolution

- **Status:** Draft
- **Authors:** nuetzliches/composit maintainers
- **Created:** 2026-04-24
- **Related:** [RFC 001 — composit-report v0.1 format](001-composit-report-format.md), [RFC 005 — Compositfile resource roles](005-compositfile-resource-roles.md)
- **Discussion:** _(link to GitHub Discussion when published)_

## Summary

Teaches `composit scan` and `composit diff` to resolve environment-variable
references across files. A `docker-compose.yml` that writes
`ports: ["${NEOAI_API_PORT:-2404}:2404"]` and a sibling `.env` that sets
`NEOAI_API_PORT=5090` today surface as two disjoint resources. After this
RFC, the report carries both the raw expression and a resolved form, and
role constraints operate on the resolved form.

## Motivation

Scanner output today is literal. Consider this excerpt from a real scan:

```yaml
- type: docker_service
  name: neoai-api
  image: git.example/acme/soapneo-neo-api:${NEO_API_TAG:-latest}
  ports: ["127.0.0.1:5090:8080"]
```

The `image` tag is stuck at `${NEO_API_TAG:-latest}` — a literal string —
even though the same scan emits an `env_file` that defines `NEO_API_TAG=0.4.2`.
RFC 005 role checks (`image_pin = ["acme/*:0.4.x"]`) can't match because
they see the unexpanded form.

Three downstream features want resolved values:

1. **Role `image_pin`** — matches the real running image, not the template.
2. **Cost estimation** — needs the resolved image to price a registry pull.
3. **Drift diffs** — users expect to see `0.4.2 → 0.4.3`, not
   `${NEO_API_TAG:-0.4.2} → ${NEO_API_TAG:-0.4.3}`.

## Non-goals

- **Not** a general compose variable substitution engine. We support the
  subset actually used in docker-compose: `${VAR}`, `${VAR:-default}`,
  `${VAR:?error}`, `${VAR:+alt}` (the four forms Compose 2.0+ supports).
  Full shell interpolation is out of scope.
- **Not** runtime environment variables. We only read variables declared
  in `.env` files discovered by the scan. The shell's `export` state at
  scan time is ignored — reports must be reproducible.
- **Not** macro expansion in arbitrary file types. v0.1 targets
  `docker_service` fields (`image`, `ports`, `environment` values). Other
  resource types add on demand.
- **Not** secret resolution. If a `.env` file contains a token, we still
  don't read the value — the scanner emits key names only (RFC 001
  §env_file). Only *values that appear in an env-file key and are used as
  plain configuration* are resolvable.

## Design

### Matching `.env` files to compose files

Docker Compose's default rule: an `.env` file in the same directory as
the `docker-compose.yml` is auto-loaded. We mirror that default:

```
./deploy/neoai-stack/
├── docker-compose.yml        # references ${NEO_API_TAG}
└── .env                      # sets NEO_API_TAG=0.4.2
```

Resolution order (most specific wins):

1. `<compose-dir>/.env` — the canonical default.
2. `<compose-dir>/.env.<scan-mode-hint>` — only if the user supplies a
   scan-mode hint (`--env-file-hint production`). v0.1 does not implement
   the hint; placeholder for a future CLI flag.

If no matching env file exists, variables with a `:-default` fallback
resolve to the default; variables without one resolve to `null` and
surface as `unresolved_variable` info in the diff.

### Expression grammar

```
expr      ::= "${" name [ modifier ] "}"
name      ::= [A-Za-z_][A-Za-z0-9_]*
modifier  ::= ":-" default       # use default if unset or empty
            | ":?" error         # error if unset or empty
            | ":+" alt           # use alt if set and non-empty
default   ::= any-char-except-"}"
```

No nested `${}` in v0.1.

### Report format changes (additive)

A resolvable field gets a companion field with a `resolved_` prefix. The
original remains intact:

```yaml
- type: docker_service
  name: neoai-api
  image: git.example/acme/soapneo-neo-api:${NEO_API_TAG:-latest}
  resolved_image: git.example/acme/soapneo-neo-api:0.4.2
  ports: ["${NEOAI_API_PORT:-2404}:2404"]
  resolved_ports: ["5090:2404"]
```

For fields that already nest (e.g. ports is an array), `resolved_*` is an
array of the same length. Indices line up: `resolved_ports[0]` resolves
`ports[0]`.

A new top-level `resolution` block summarises the pass:

```yaml
resolution:
  env_files_used:
    - ./deploy/neoai-stack/.env
    - ./AdvoNeo/.env
  unresolved:
    - resource_path: ./docker-compose.yml
      field: image
      variable: NO_SUCH_VAR
      reason: "not in any .env, no default"
```

### Diff behaviour

Role matchers and constraints MUST prefer the resolved form when present:

```hcl
role "database" {
  match {
    image = ["postgres:*"]
  }
  image_pin = ["postgres:16"]
}
```

Given `image = "postgres:${PG_TAG:-latest}"` and `.env` sets `PG_TAG=16`:

| Field matcher reads | Without RFC 006 | With RFC 006           |
|---------------------|-----------------|-------------------------|
| `image`             | `postgres:${PG_TAG:-latest}` (matches `postgres:*`) | same |
| `image_pin`         | `${PG_TAG:-latest}` (doesn't match `postgres:16`) | `postgres:16` ✓ |

When `resolved_*` is absent (variable couldn't be resolved), the matcher
falls back to the raw field.

### New violations

| Rule                      | Severity | Fires when                                  |
|---------------------------|----------|---------------------------------------------|
| `unresolved_variable`     | Info     | a compose field references an undefined var without default |
| `resolution_inconsistency`| Warning  | two .env files in scope declare the same key with different values |

These are emitted by `composit scan` and threaded through the report so
`composit diff` can surface them without re-reading the workspace.

### CLI surface (no change for v0.1)

`composit scan` performs resolution automatically. No new flag.

Future `--no-resolution` can disable it if an implementer wants raw-only
output (useful for third-party tools that do their own substitution).

## Implementation sketch

1. After all filesystem scanners run (`core/registry.rs::run_all`), collect
   `env_file` resources and their full key/value content (new: emit
   `values` behind an opt-in flag in the env_files scanner; secrets-safe
   only when the Compositfile marks a path as `resolvable`).
2. For each `docker_service`, identify the nearest compose directory and
   look up its `.env` file in the collected map.
3. Apply substitution to `image`, each element of `ports`, and each value
   of `environment` that's a `${…}` expression.
4. Emit `resolved_image`, `resolved_ports`, `resolved_environment`, and
   record the source `.env` paths in the report's new `resolution` block.
5. In the role matcher (`src/commands/diff.rs::role_matches`), read
   `resolved_image` first, fall back to `image`. Same for every
   constraint checker.

## Secrets caveat

Values read from `.env` files leave the disk and enter the composit
report. That's a behaviour change: today the env_files scanner stores
*keys only*. Two safeguards:

1. **Opt-in per path.** Only `.env` files matched by the Compositfile
   `scan.resolvable` glob block are eligible. Default: none, so existing
   workspaces see no secret exposure by accident.
2. **Redacted values for obvious secrets.** Keys matching `*_KEY`,
   `*_SECRET`, `*_TOKEN`, `*_PASSWORD` (case-insensitive) are replaced
   with `"<redacted>"` before writing to the report. Resolution still
   runs — the substituted form just reads `<redacted>` in the value.

Operators who want aggressive resolution can extend the redaction list in
the Compositfile:

```hcl
scan {
  resolvable = [".env", "ansible/**/production/.env"]
  redact     = ["*_KEY", "*_SECRET", "*_TOKEN", "*_PASSWORD", "DATABASE_URL"]
}
```

## Open questions

1. **Ansible Jinja expansion.** `{{ var }}` in `.j2` templates has the
   same shape as the problem here but different semantics (variables may
   come from inventories, group_vars, role defaults). RFC 006 v0.1 does
   *not* cover Jinja; a follow-up RFC should.
2. **Arrays in environment values.** Compose lets `environment:` be a list
   of `KEY=${VAR}` strings *or* a mapping. Both need resolution. v0.1
   handles both; document carefully in the implementation.
3. **Interpolation in image `build:` contexts.** Build args use their own
   mechanism. Not touched in v0.1.

## Migration

Additive RFC. Existing `composit-report.yaml` files stay valid. Consumers
that don't read `resolved_*` keep their current behaviour.

## Changelog

- **2026-04-24** — Initial draft.
