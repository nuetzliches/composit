# Composit Live Demo — Dogfooding on `nuts-infra`

This directory is the canonical live-demo artifact:
the output of running `composit scan` + `composit diff` against a real
production-adjacent stack with an **intentionally misaligned Compositfile**.

The diff is meant to fail loudly — that's the point. It makes the
IS-vs-SHOULD story visible in one screen.

## Contents

| File                   | Purpose                                           |
|------------------------|---------------------------------------------------|
| `Compositfile`         | SHOULD-state with deliberate drift vs. reality    |
| `composit-diff.html`   | Rendered output of `composit diff --output html`  |

`composit-report.yaml` (the IS-state) is **not committed** because it contains
workspace-specific paths. Regenerate it with `composit scan` on the target
repo (see below).

## Expected findings

The Compositfile is pitched tight against the real `nuts-infra` scan to
surface every category of violation:

| Category    | What triggers                                              |
|-------------|-------------------------------------------------------------|
| Providers   | `unused_provider` warnings for croniq/hookaido/powerbrain  |
|             | (they're approved but not in the scan — governance stale) |
| Resources   | `resource_count_exceeded` (max_total 60 vs. actual 96)     |
|             | `resource_type_not_allowed` (caddy_site, env_file, etc.)   |
|             | `resource_type_max_exceeded` (docker_service: 38 > 20)     |
|             | `required_resource_missing` (prometheus_config, 0 < 1)     |
| Policies    | `policy_file_missing` (referenced .rego not in repo)       |

## Reproduce

```bash
# 1. Scan the target stack (IS-state)
composit scan --dir /path/to/nuts-infra --no-providers

# 2. Compare against the demo Compositfile (SHOULD-state)
composit diff \
  --report /path/to/nuts-infra/composit-report.yaml \
  --compositfile examples/demo/Compositfile \
  --output html
```

The HTML file opens in a browser and is a shareable artifact —
use it for HN screenshots, blog posts, or demo recordings.

## Terminal output snapshot

```
composit diff
============================================================
Workspace: nuts-infra

PROVIDERS (3 warnings)
  WARN   unused_provider — Approved provider "croniq" not found in scan report
  WARN   unused_provider — Approved provider "hookaido" not found in scan report
  WARN   unused_provider — Approved provider "powerbrain" not found in scan report

BUDGETS (pass)
  PASS  All 1 checks passed

RESOURCES (6 errors)
  ERROR  resource_count_exceeded — Total resources 96 exceeds max_total 60
  ERROR  resource_type_not_allowed — Resource type "caddy_site" (24 found) not in allow list
  ERROR  resource_type_not_allowed — Resource type "dockerfile" (2 found) not in allow list
  ERROR  resource_type_not_allowed — Resource type "env_file" (10 found) not in allow list
  ERROR  resource_type_max_exceeded — docker_service: 38 found, max allowed is 20
  ERROR  required_resource_missing — Required resource type "prometheus_config": 0 found, minimum is 1

POLICIES (1 warning)
  WARN   policy_file_missing — Policy references missing file: policies/no-experimental-images.rego

------------------------------------------------------------
  6 errors | 4 warnings | 0 info | 4 passed
```
