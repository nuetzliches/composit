# RFC 007 — Jinja2 Template Resolution for Ansible Templates

- **Status:** Draft
- **Authors:** nuetzliches/composit maintainers
- **Created:** 2026-04-24
- **Related:** [RFC 001 — composit-report format](001-composit-report-format.md), [RFC 005 — resource roles](005-compositfile-resource-roles.md), [RFC 006 — cross-file variable resolution](006-cross-file-variable-resolution.md)
- **Discussion:** _(link to GitHub Discussion when published)_

## Summary

Teaches `composit scan` to render Ansible-style Jinja2 templates (`.j2`
files) so the resulting config surfaces in the report as a first-class
resource. Extends RFC 006 (which handles simple `${VAR}` substitution in
docker-compose) to the richer Ansible variable model: inventory, host
vars, group vars, role defaults/vars, and `--extra-vars`-style overrides.

## Motivation

The `ansible` scanner added in composit v0.3 surfaces playbooks,
inventories, and roles, but templates are opaque: a `.j2` file shows up
only as a count inside its role's resource. That means:

- Role constraints (RFC 005 `image_pin`, `must_expose`, …) cannot govern
  the *rendered* nginx config, systemd unit, or env file that Ansible
  will actually deploy. The template is effectively un-governed
  infrastructure.
- Drift detection cannot answer "what value will this host see?" when
  the scanner sees only `{{ nginx_port | default(80) }}`.
- Cross-inventory comparisons (staging vs production) are impossible
  because the scanner never resolves inventory-specific values.

RFC 006 solved this for docker-compose because Compose uses a single
flat `.env` per compose file. Ansible has a layered variable model — we
need a proper resolver, not string substitution.

## Non-goals

- **Not** a full Ansible engine. We render variables, loops, and filters
  safely in a sandbox; we do not execute modules, connect to hosts, or
  run tasks.
- **Not** a Jinja2 feature-parity target. We implement the subset
  Ansible commonly uses for config templates: variable lookup, the
  `default` filter, `lower`/`upper` filters, string concatenation, and
  simple conditionals. Unsupported constructs degrade to a warning and
  leave the template unrendered (surfaced as `template_unrenderable`
  Info in the diff).
- **Not** execution of `lookup('...')` or `query('...')`. Those hit the
  filesystem or network; rendering a `.j2` must remain offline and
  deterministic.
- **Not** a replacement for `ansible-playbook --check --diff`. Our
  purpose is inventory/reporting, not deploy simulation.

## Design

### Variable resolution order

Matches Ansible's documented precedence, lowest to highest:

1. Role defaults (`roles/<role>/defaults/main.yml`)
2. Inventory group vars (`inventory/group_vars/<group>.yml`)
3. Inventory host vars (`inventory/host_vars/<host>.yml`)
4. Role vars (`roles/<role>/vars/main.yml`)
5. Playbook `vars:` block
6. Compositfile `scan.ansible.extra_vars` (new — see §Compositfile additions)

All sources live in the scanned workspace — we do not read from the
operator's shell, `ANSIBLE_VARS_FILE`, vault, or any runtime state.
Reports must be reproducible from the tree alone.

### Compositfile additions

```hcl
scan {
  ansible {
    # Variables that override every inventory source — analogous to
    # `ansible-playbook --extra-vars`. Useful to pin a value for drift
    # comparison: e.g. force `nginx_port = 443` to check all templates
    # resolve to that.
    extra_vars = {
      env            = "production"
      nginx_port     = 443
      certbot_email  = "ops@example.com"
    }

    # Inventory selection: which inventories to render for. A template
    # is rendered once per inventory in this list; the report carries
    # one resolved resource per (template, inventory) pair.
    #
    # Default: [] → render only with extra_vars, no inventory loop.
    # Common pattern: ["inventory/production.yml", "inventory/staging.yml"]
    inventories = ["inventory/production.yml", "inventory/staging.yml"]

    # Per-template redaction, same semantics as RFC 006 `redact`.
    # Matches case-insensitively against rendered-value keys (not
    # template-variable names).
    redact = ["*_SECRET", "*_KEY", "DATABASE_URL"]
  }
}
```

All fields are optional. Omitting the `ansible { }` block means templates
are listed as today (path + template_count) without rendering.

### Resource-type additions

A new resource type `ansible_template`:

```yaml
- type: ansible_template
  name: nginx.conf.j2
  path: ./ansible/roles/nginx/templates/nginx.conf.j2
  parent_role: nginx
  renderings:
    - inventory: ./inventory/production.yml
      rendered:
        listen: "443"
        server_name: "api.example.com"
      checksum: sha256:ab12…
      unresolved: []
    - inventory: ./inventory/staging.yml
      rendered:
        listen: "443"
        server_name: "staging.api.example.com"
      checksum: sha256:cd34…
      unresolved: []
```

`rendered` is emitted as a structured map when the template produces
a recognised format (nginx syntax → parsed; systemd unit → parsed; env
file → key/value map). When we can't parse the rendered output, we
store the raw string under `rendered_raw` instead.

`checksum` lets `composit diff` detect drift between inventories without
diffing text — a changed checksum is a signal to look.

### Role-constraint integration

Role matchers and constraints gain a way to target rendered templates:

```hcl
role "web-host-config" {
  match {
    type = "ansible_template"
    name = ["nginx.conf.j2"]
  }
  # Per-rendering constraint: every rendering must carry these values.
  rendered_must_contain = {
    "listen"      = "443"
    "server_name" = "*.example.com"
  }
}
```

New rule names:

| Rule                             | Severity | Fires when                                            |
|----------------------------------|----------|-------------------------------------------------------|
| `template_unrenderable`          | Info     | An unsupported Jinja2 construct is hit                |
| `template_missing_var`           | Info     | A required variable is referenced with no default     |
| `template_value_mismatch`        | Error    | `rendered_must_contain` fails for any rendering       |
| `template_inventory_drift`       | Warning  | Same template produces different values across inventories *when* a `role.same_across_inventories` attribute is set |

### Safety model

Rendering runs in a sandbox:

- No filesystem access beyond the template bytes already in memory.
- No network access (`url_lookup`, `uri`, etc. — all refuse).
- Hard 1-second CPU budget per template; templates that exceed it
  are flagged `template_unrenderable: timeout`.
- Output truncated to 1 MiB per rendering; beyond that, `rendered_raw`
  stores the first 1 MiB and an `output_truncated: true` flag.

We pick the Rust `minijinja` crate (already friendly to sandboxed use)
rather than shelling out to Python. `minijinja` supports the Ansible
subset we care about and is audited for safe embedding.

### Interaction with RFC 006

RFC 006 resolution runs before RFC 007. If a rendered env-file template
declares `API_PORT=5432`, and a later `docker-compose.yml` in the same
tree references `${API_PORT}`, the two resolvers can chain if and only
if the operator explicitly lists the rendered file in `scan.resolvable`.
We do not invent synthetic env files from templates by default — that
would blur the line between "what's written" and "what's computed".

## Implementation sketch

1. **Ansible scanner** (`src/scanners/ansible.rs`): add a `.j2`
   walk that records template path + owning role, without rendering.
   Emits `ansible_template` resources with empty `renderings`.
2. **Inventory loader**: parse each inventory listed in
   `scan.ansible.inventories` into a `HashMap<host, VarScope>`.
   `VarScope` carries the layered defaults → host_vars → extra_vars
   map for that host.
3. **Renderer**: for each (template, inventory) pair, invoke
   `minijinja` with the merged VarScope. Sandbox via a custom
   `Environment` that rejects `lookup`/`query`/`url_lookup`.
4. **Parsers**: recognised rendered formats (nginx, systemd, dotenv)
   are parsed into structured maps using existing scanners' primitives
   (`nginx`, `env_files`) so the `rendered` field is structured.
5. **Diff integration** (`src/commands/diff.rs`): extend
   `check_resolution` to emit `template_missing_var` from
   `Resource.renderings[].unresolved`, and add role-constraint
   handling for `rendered_must_contain`.

## Rollout

- v0.4: scanner emits `ansible_template` resources without renderings.
  No behaviour change for existing reports (new resource_type, new
  renderings field — both additive).
- v0.5: rendering behind a CLI flag `--render-templates`. Opt-in so
  operators with big inventories can skip the cost.
- v0.6: rendering by default when `scan.ansible.inventories` is set.

## Open questions

1. **Vault-encrypted files.** Ansible's `ansible-vault`-encrypted YAML
   is readable only with a passphrase. v0.1 refuses to render any
   template whose inventory includes a vault file and emits a
   `vault_unsupported` Info. Future RFC could support vault-password-
   file references.
2. **Host-specific loops.** `with_items: "{{ groups.web }}"` renders
   one value per host in a group. Our model is "per inventory"; we'd
   need "per host" semantics to cover this. Deferred.
3. **Template-generates-config chain.** A `.j2` that renders into a
   docker-compose file that itself has `${VAR}` — two-stage resolution.
   v0.1 does not cover; we could add a `resolve_after_render` flag in
   a later revision.
4. **Which file formats to parse.** The default list (nginx, systemd,
   dotenv) is arbitrary. An operator-extensible renderer→parser map
   (by file extension or template name) could live in
   `scan.ansible.parsers`. **Status (2026-04-24):** dotenv is the only
   format implemented; nginx/systemd still pending.
5. **`rendered_must_contain` fallback accuracy.** When a rendering is
   not parseable into a structured map (no nginx/systemd parser yet),
   the diff checker falls back to a substring match: `"<key>" in rendered
   && "<value-without-globs>" in rendered`. This is conservative
   (false-positives possible with ambiguous key names that share
   substrings). A format-specific parser per §4 removes the ambiguity.
   Tracking issue: consider emitting `template_match_fallback` Warning
   when the substring path fires instead of the parsed path.

## Migration

Additive RFC. Existing `composit-report.yaml` files remain valid. The
`ansible_template` resource type is new. The report's v0.1 JSON Schema
already allows arbitrary resource types through `additionalProperties`,
so schema changes are informational — we document the new fields under
§Resource in RFC 001's next revision.

## Changelog

- **2026-04-24** — Initial draft + v0.2 implementation pass:
  - Scanner emits `ansible_template` with `vault_encrypted` flag.
  - `scan.ansible.extra_vars` + `scan.ansible.inventories` both honoured.
  - minijinja-backed rendering with `UndefinedBehavior::Strict` + 1 MiB
    output cap.
  - Dotenv format parser populates `rendered_parsed` for role checks.
  - Role constraint `rendered_must_contain` enforces keys on every
    rendering (new rule `template_value_mismatch`).
  - Vault-encrypted templates surface `vault_unsupported` Info in diff.
