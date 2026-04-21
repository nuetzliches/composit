#!/usr/bin/env bash
#
# Paced walkthrough of `composit scan` + `composit diff` on the demo-drift
# workspace. Designed to be the inner command for an asciinema recording:
#
#   asciinema rec composit-demo.cast -c "bash docs/demo/record.sh"
#
# Total runtime: ~35s. Three governance errors must surface — if the output
# drifts from the reference in examples/demo-drift/README.md, the HN artefact
# has regressed and the recording should not ship.

set -euo pipefail

# Resolve the composit binary:
#   1. $COMPOSIT override wins (useful in CI).
#   2. `composit` on $PATH (the HN-realistic path: user installed it).
#   3. Fallback to the debug build inside this repo.
if [[ -n "${COMPOSIT:-}" ]]; then
  BIN="$COMPOSIT"
elif command -v composit >/dev/null 2>&1; then
  BIN="composit"
else
  BIN="$(git rev-parse --show-toplevel)/target/debug/composit"
  if [[ ! -x "$BIN" ]]; then
    echo "composit binary not found. Run 'cargo build' or install composit first." >&2
    exit 1
  fi
fi

DEMO_DIR="$(git rev-parse --show-toplevel)/examples/demo-drift"

# Mimic a human typing the command, then execute it.
prompt() {
  printf '\033[1;32m$\033[0m '
  local line="$*"
  for ((i = 0; i < ${#line}; i++)); do
    printf '%s' "${line:$i:1}"
    sleep 0.02
  done
  printf '\n'
  sleep 0.4
}

section() {
  printf '\n\033[1;34m# %s\033[0m\n' "$*"
  sleep 0.8
}

# ── Act I: the workspace ───────────────────────────────────────────────

section "A synthetic widgetshop — one compose file, a Compositfile, a rogue MCP"
prompt "ls examples/demo-drift/"
(cd "$DEMO_DIR" && ls -A)
sleep 2

# ── Act II: scan ───────────────────────────────────────────────────────

section "Inventory what actually exists"
prompt "composit scan --dir examples/demo-drift --no-providers"
"$BIN" scan --dir "$DEMO_DIR" --no-providers
sleep 2.5

# ── Act III: diff ──────────────────────────────────────────────────────

section "Compare against the Compositfile"
prompt "composit diff --dir examples/demo-drift --offline"
"$BIN" diff --dir "$DEMO_DIR" --offline
sleep 3

# Clean up so the fixture stays as shipped.
rm -f "$DEMO_DIR/composit-report.yaml"
