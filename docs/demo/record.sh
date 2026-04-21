#!/usr/bin/env bash
#
# Paced walkthrough of `composit scan` + `composit diff` on the demo-drift
# workspace. Designed to be the inner command for an asciinema recording:
#
#   asciinema rec composit-demo.cast --cols 150 --rows 30 \
#     -c "bash docs/demo/record.sh"
#
# 150x30 is important — the scan summary row for each docker_service runs
# up to ~147 chars (path + 40-col pad, attribution + last-modified
# arrow, image + ports + networks inline). Narrower terminals wrap
# mid-line. The script also runs from the repo root with relative paths
# so the recording doesn't leak $HOME via "Report written to:".
#
# Total runtime: ~11s (short on purpose — HN attention spans). Three
# governance errors must surface; if the output drifts from the reference
# in examples/demo-drift/README.md, the HN artefact has regressed and the
# recording should not ship.

set -euo pipefail

# Resolve the composit binary — prefer the build tree so demos of the
# composit repo itself always exercise the current working source, not
# whatever version was last `cargo install`ed into $PATH.
#   1. $COMPOSIT override wins (useful in CI or when recording against
#      a released binary on purpose).
#   2. Local debug build — the dogfooding default.
#   3. `composit` on $PATH as a last resort (no build tree: you're
#      recording someone else's checkout).
if [[ -n "${COMPOSIT:-}" ]]; then
  BIN="$COMPOSIT"
elif [[ -x "$(git rev-parse --show-toplevel)/target/debug/composit" ]]; then
  BIN="$(git rev-parse --show-toplevel)/target/debug/composit"
elif command -v composit >/dev/null 2>&1; then
  BIN="composit"
else
  echo "composit binary not found. Run 'cargo build' or install composit first." >&2
  exit 1
fi

# Always cd to the repo root so relative paths in both the prompt text
# and the actual commands match — and so the "Report written to:" line
# prints a short relative path instead of the user's absolute $HOME.
cd "$(git rev-parse --show-toplevel)"
DEMO_DIR="examples/demo-drift"

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
prompt "ls $DEMO_DIR/"
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
