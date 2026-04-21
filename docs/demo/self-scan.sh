#!/usr/bin/env bash
#
# Self-scan: runs `composit scan + diff --strict` against the composit
# repo itself. Builds the binary first so the run always exercises the
# current working tree — not whatever version happens to be on $PATH.
#
# Use cases:
#   - Local gate before pushing ("does my change break governance?")
#   - CI step that fails the PR if Compositfile drifts from scan reality
#
# Exit codes:
#   0 — no drift, Compositfile matches scan
#   1 — drift detected or build failed
#
# The report is written to composit-report.yaml at the repo root. It is
# gitignored so the file surfaces locally for inspection but never ends
# up in a commit.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

echo "▸ Building composit (debug profile, current tree)…"
cargo build --quiet

BIN="$REPO_ROOT/target/debug/composit"

echo "▸ Scanning $REPO_ROOT"
"$BIN" scan --dir . --no-providers --quiet

echo
"$BIN" diff --offline --strict
