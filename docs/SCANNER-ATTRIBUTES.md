# Scanner Attribute Contract

This document is the per-resource-type reference for attributes emitted by
composit's built-in scanners. It exists because RFC 005 role matching and
the diff checker read these attributes directly — if a scanner renames a
key or forgets to emit one, rules silently no-op.

Treat this as a **contract**: once a key is documented here, changing it is
a breaking change and requires coordinated updates to the RFCs and diff
checker.

## Base fields (every resource)

Every `Resource` carries these top-level fields regardless of type:

| Field          | Type          | Required | Source                                   |
|----------------|---------------|----------|------------------------------------------|
| `type`         | string        | yes      | scanner — the resource_type              |
| `name`         | string \| null| no       | scanner — service/resource/file name     |
| `path`         | string \| null| no       | scanner — `./<rel>` forward-slash path   |
| `provider`     | string \| null| no       | rarely used today                        |
| `created`      | string \| null| no       | git-attributed first-commit date         |
| `created_by`   | string \| null| no       | `human:<email>` / `agent:<name>`          |
| `detected_by`  | string        | yes      | scanner id                               |
| `estimated_cost`| string \| null| no      | rarely used today                        |

The `extra: Map<string, any>` map is flattened into the JSON/YAML output at
the top level (so `extra["image"]` renders as `image:` alongside `name:`).

All file paths are normalised to forward slashes at the orchestrator
boundary (`src/core/registry.rs`) — scanners may emit backslashes; they
are rewritten before the report leaves the CLI.

## Resource types

The **Role-readable** column lists the attributes RFC 005 role matchers
and constraints currently consume. If a role rule names an attribute that
isn't in this column for a given type, the rule will always pass (silently)
on that type.

### `docker_compose`

Emitted by `docker` scanner for each `docker-compose*.yml` / `compose*.yml`
variant discovered.

| Key                   | Type          | Notes                                |
|-----------------------|---------------|--------------------------------------|
| `services`            | integer       | count of services in the file        |
| `networks`            | string[]      | defined top-level network names      |
| `volumes`             | string[]      | defined top-level volume names       |

**Role-readable:** `networks`, `volumes`.

### `docker_service`

Emitted by `docker` scanner for each service inside a compose file.

| Key            | Type          | Notes                                      |
|----------------|---------------|--------------------------------------------|
| `image`        | string        | raw image spec (may include `${VAR}`)       |
| `build`        | string        | `build:` shorthand or context path          |
| `ports`        | string[]      | raw port mappings (`"8080:80"` form)         |
| `networks`     | string[]      | networks the service attaches to            |
| `compose_file` | string        | relative path to the owning compose file    |
| `depends_on`   | string[]      | declared dependencies                        |

**Role-readable:** `image`, `ports` (container-side port extracted by the
role matcher), `networks`.

### `dockerfile`

Emitted by `docker` scanner for standalone Dockerfiles.

No `extra` keys today.

### `env_file`

Emitted by `env_files` scanner for `.env*` files.

| Key           | Type          | Notes                                         |
|---------------|---------------|-----------------------------------------------|
| `variables`   | integer       | count of non-comment assignment lines          |
| `keys`        | string[]      | variable names (no values — never secrets)     |

**Role-readable:** `keys` (used by `must_set_env` / `forbidden_env`), also
`path` for match predicates.

### `workflow`

Emitted by `workflows` scanner for GitHub Actions / Forgejo workflows.

| Key        | Type     | Notes                                        |
|------------|----------|----------------------------------------------|
| `platform` | string   | `github-actions` / `forgejo`                  |
| `triggers` | string[] | `on:` keys — `push`, `pull_request`, etc.     |
| `jobs`     | integer  | number of jobs                                |
| `runs_on`  | string   | first job's runner label (heuristic)          |

### `prometheus_config` / `prometheus_rules`

Emitted by `prometheus` scanner for `prometheus.yml` and rules files.

| Key              | Type     | Type-scope                        |
|------------------|----------|-----------------------------------|
| `scrape_configs` | integer  | `prometheus_config`                |
| `job_names`      | string[] | `prometheus_config`                |
| `remote_read`    | boolean  | `prometheus_config`                |
| `remote_write`   | boolean  | `prometheus_config`                |
| `rules`          | integer  | `prometheus_rules`                 |
| `groups`         | integer  | `prometheus_rules`                 |
| `alerting`       | boolean  | `prometheus_rules`                 |

### `grafana_dashboard` / `grafana_datasource`

| Key             | Type    | Scope               |
|-----------------|---------|---------------------|
| `uid`           | string  | `grafana_dashboard` |
| `folder`        | string  | `grafana_dashboard` |
| `ds_type`       | string  | `grafana_datasource`|
| `version`       | string  | both                |

### `kubernetes_manifest`

| Key           | Type    | Notes                                     |
|---------------|---------|-------------------------------------------|
| `kind`        | string  | `Deployment` / `Service` / …               |
| `namespace`   | string  | `metadata.namespace`, falls back to default |
| `api_version` | string  | `apiVersion:`                              |
| `app`         | string  | common `app.kubernetes.io/name` label      |

### `terraform_config` / `terraform_module` / `terraform_resource` / `terraform_state`

| Key               | Type         | Scope                |
|-------------------|--------------|----------------------|
| `resources`       | integer      | `terraform_config`   |
| `modules`         | integer      | `terraform_config`   |
| `outputs`         | integer      | `terraform_config`   |
| `provider_list`   | string[]     | `terraform_config`   |
| `resource_type`   | string       | `terraform_resource` |
| `source`          | string       | `terraform_module`    |
| `version`         | string       | `terraform_module`    |

**Role-readable:** `resource_type` for RFC 004 `allowed_types`.

### `caddyfile`

| Key             | Type        |
|-----------------|-------------|
| `sites`         | integer     |
| `domains`       | string[]    |
| `reverse_proxy` | string      |
| `file_server`   | boolean     |
| `tls`           | boolean     |

### `nginx_config`

| Key     | Type    | Notes |
|---------|---------|-------|
| `sites` | integer | `server {}` blocks |

### `cron`

| Key        | Type   | Notes                         |
|------------|--------|-------------------------------|
| `schedule` | string | raw crontab schedule expr      |

### `mcp_config` / `mcp_server` / `mcp_provider` / `mcp_capability`

MCP discovery is a multi-step pipeline; see each scanner for details.
Role matching is not yet used for MCP types.

### `fly_toml` / `render_yaml` / `vercel_json` / `skaffold` / `traefik`

Platform-config scanners. Each emits a light summary plus the platform name:

| Key        | Type   | Notes |
|------------|--------|-------|
| `services` | integer | where applicable |

### `opa_policy`

| Key            | Type        | Notes                        |
|----------------|-------------|------------------------------|
| `managed_resources` | integer | references in the Rego source |

### `db_migration`

| Key           | Type     | Notes                          |
|---------------|----------|--------------------------------|
| `version`     | string   | leading id of the migration    |
| `description` | string   | migration name/description      |

### `deploy_script`

No `extra` keys today — the script is flagged by name only.

### `proto`

| Key       | Type    |
|-----------|---------|
| `services`| integer |

### `tempo`

Single-file config; no structured `extra`.

### `ansible_playbook`

Emitted by `ansible` scanner for YAML files that look like playbooks.

| Key       | Type     | Notes                                       |
|-----------|----------|---------------------------------------------|
| `plays`   | integer  | count of plays with a `hosts:` key          |
| `imports` | integer  | count of `import_playbook:` entries         |
| `tasks`   | integer  | total tasks across all plays                |
| `roles`   | string[] | role names referenced from the playbook     |

### `ansible_inventory`

Emitted by `ansible` scanner for `inventory*.yml` / `hosts*.yml` files.

| Key           | Type     | Notes                               |
|---------------|----------|-------------------------------------|
| `groups`      | integer  | top-level group count (YAML only)    |
| `group_names` | string[] | top-level group names (YAML only)    |

### `ansible_role`

Emitted by `ansible` scanner for directories containing `tasks/main.yml`.

| Key              | Type    | Notes                                  |
|------------------|---------|----------------------------------------|
| `has_handlers`   | boolean | `handlers/` sub-dir present             |
| `has_vars`       | boolean | `vars/` or `defaults/` sub-dir present  |
| `has_templates`  | boolean | `templates/` sub-dir present            |
| `template_count` | integer | number of `*.j2` files in `templates/`  |

### `ansible_template`

Emitted by `ansible` scanner for `.j2` templates under any `templates/`
directory. Renderings are populated by the registry post-pass when the
Compositfile declares `scan.ansible.extra_vars` or `scan.ansible.inventories`
(RFC 007).

| Key               | Type              | Notes                                          |
|-------------------|-------------------|------------------------------------------------|
| `parent_role`     | string            | name of the owning role dir (heuristic)        |
| `vault_encrypted` | boolean           | true when file starts with `$ANSIBLE_VAULT;`   |
| `renderings`      | array of objects  | per-context render results                     |

Each `renderings[]` entry carries:
- `source: <tag>` — `"extra_vars"` for the extras-only case, or the relative inventory path when per-inventory rendering is active
- On success: `rendered: "<string>"` + `checksum: "sha256:<hex>"`
- On success for dotenv-shaped output: `rendered_parsed: { format: "dotenv", keys: { KEY: "value", … } }`
- On failure: `error: "<msg>"` + optional `unresolved_variable: "<name>"`
- `output_truncated: true` when the rendered output exceeded 1 MiB

**Role-readable:** `name`, `path`, `renderings[].rendered_parsed.keys`
(via `rendered_must_contain`). Vault-encrypted templates never carry
renderings and surface `vault_unsupported` Info in the diff.

### `agent_spec`

Emitted by `agent_spec` scanner for `SKILL.md`, `AGENTS.md`, and
`CLAUDE.md` files at any depth in the workspace. Surfaces the YAML
frontmatter where present (Anthropic skill manifests) plus the
filename kind so role rules can scope to skills vs. free-form
agent instructions.

| Key             | Type    | Notes                                                                  |
|-----------------|---------|------------------------------------------------------------------------|
| `kind`          | string  | `"skill"` (SKILL.md), `"agents"` (AGENTS.md), `"claude"` (CLAUDE.md)    |
| `description`   | string  | YAML frontmatter `description`; folded scalars collapse to one line     |
| `allowed_tools` | string  | YAML frontmatter `allowed-tools` value (verbatim — array or string)     |
| `model`         | string  | YAML frontmatter `model`, when present                                  |
| `version`       | string  | YAML frontmatter `version`, when present                                |
| `lines`         | integer | total line count of the file                                            |

`name` falls back to the directory basename when frontmatter has no
`name:` field — keeps `skills/<id>/SKILL.md` repos distinguishable
without polluting the resource list with `null` names.

**Detection rules:** `SKILL.md` requires either parseable YAML
frontmatter or a top-level heading containing the word "skill"
(case-insensitive). `AGENTS.md` / `CLAUDE.md` are recorded whenever
they exist — they're free-form by convention.

### `cargo_workspace` / `cargo_crate`

Emitted by `cargo_manifest` scanner for `Cargo.toml`. A single
manifest can produce both resources when it declares `[workspace]`
and `[package]` simultaneously (root binary + workspace).

`cargo_workspace`:

| Key            | Type     | Notes                                        |
|----------------|----------|----------------------------------------------|
| `members`      | string[] | values from `[workspace] members = [ … ]`     |
| `member_count` | integer  | length of `members`                           |
| `version`      | string   | `[workspace.package] version`, when present   |

`cargo_crate`:

| Key       | Type   | Notes                              |
|-----------|--------|------------------------------------|
| `version` | string | `[package] version`                |
| `edition` | string | `[package] edition` (e.g. `2024`)   |
| `license` | string | `[package] license`, when present   |

`name` for `cargo_crate` comes from `[package] name`. For
`cargo_workspace` it falls back to the parent directory basename
(workspace roots usually don't carry their own `name`).

`Cargo.toml` files inside `target/` are skipped (vendored builds).

### `go_module`

Emitted by `go_module` scanner for `go.mod` files. Multi-module
repos surface one resource per `go.mod`; `vendor/` is skipped.

| Key                 | Type    | Notes                                              |
|---------------------|---------|----------------------------------------------------|
| `go_version`        | string  | from `go <version>` directive                      |
| `direct_requires`   | integer | count of `require` entries without `// indirect`   |
| `indirect_requires` | integer | count of `require` entries with `// indirect`      |

`name` is the module path declared by `module <path>`.

## Adding a new attribute

1. Update the scanner to emit the key into `resource.extra`.
2. Add a row to the relevant table above.
3. If a role matcher or diff check reads the key, land the reader in the
   same PR so the contract stays consistent.
4. If the key meaningfully changes the shape of the resource, update the
   corresponding RFC changelog (RFC 001 for report format; RFC 005 for
   role semantics).

## Breaking changes

Renaming or removing an attribute breaks downstream consumers. Process:

1. Add the new key alongside the old one, emit both for one release.
2. Update role/check readers to prefer the new key, fall back to the old.
3. Remove the old key in the next minor bump.

`composit-report` is versioned via RFC 001; breaking attribute changes
require bumping the report schema version.
