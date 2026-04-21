# Demo: Governance Drift

A synthetic "widgetshop" workspace shaped so `composit scan` followed by
`composit diff` surfaces exactly three governance errors. No real
infrastructure, no private endpoints — safe to clone, fork, or record.

▶ **[Watch the 11-second asciinema demo](https://asciinema.org/a/EOPTSBcM0k3wGbvh)** — the exact `scan + diff` run this README describes.

This is the reference fixture for the Show-HN demo and the regression
guard behind `tests/scanner_e2e.rs::demo_drift_surfaces_three_expected_errors`.

## What's in the repo

```
examples/demo-drift/
├── Compositfile           # the SHOULD-state
├── docker-compose.yml     # 4 services (3 approved, 1 drift)
├── .cursor/
│   └── mcp.json           # a rogue MCP provider
└── .env
```

## Run it

```bash
# 1. Inventory what exists
composit scan --dir examples/demo-drift --no-providers

# 2. Compare against the Compositfile
composit diff --dir examples/demo-drift --offline
```

## Expected output

```
PROVIDERS (1 error)
  ERROR  unapproved_provider — Provider "rogue-tools" found in report
         but not approved in Compositfile
         Endpoint: https://mcp.example.com/rogue-tools

RESOURCES (2 errors)
  ERROR  image_not_allowed — Image "redis:latest" not in allowed list
         for docker_service
         ./docker-compose.yml
  ERROR  required_resource_missing — Required resource type "workflow":
         0 found, minimum is 1

  3 errors | 0 warnings | 0 info | 7 passed
```

## The three drifts, explained

| Rule | Trigger | What it represents |
|---|---|---|
| `image_not_allowed` | `cache` service uses `redis:latest` | An agent pulled an image outside the approved registry/tag list. |
| `required_resource_missing` | `require "workflow" { min = 1 }` but no `.github/workflows/` exists | The CI gate every PR is supposed to go through has been removed or never existed. |
| `unapproved_provider` | `.cursor/mcp.json` declares `mcp.example.com/rogue-tools` | An MCP server was wired up in the dev tool without being added to the Compositfile. |

## CI mode

In a pipeline, use `--strict` so errors fail the build:

```bash
composit diff --dir examples/demo-drift --offline --strict
# exit code 1
```
