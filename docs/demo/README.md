# Asciinema Recording — Composit Demo

The HN-facing walkthrough. Shows `composit scan` and `composit diff` on
the `examples/demo-drift/` workspace end-to-end in ~11 seconds.

## Dependencies

- `asciinema` (`apt install asciinema` or `brew install asciinema`)
- A built `composit` binary, reachable as one of:
  - `composit` on `$PATH` (preferred — matches the install story),
  - `target/debug/composit` (the script will fall back to it), or
  - `$COMPOSIT` env var pointing at a binary.

## Record

```bash
cargo build                                                # if not installed
asciinema rec composit-demo.cast --cols 150 --rows 30 \
  -c "bash docs/demo/record.sh"
```

`--cols 150` matters — the scan summary row for each docker service runs
up to ~147 chars (path + attribution + image + ports + networks inline).
Narrower terminals wrap mid-line. 150×30 fits every line unbroken.

The asciinema player auto-scales in the browser so this width is fine for
HN / the landing page. Shrinking the terminal output itself to fit 100
cols is a future polish task, not a blocker.

Press `Ctrl-D` or wait for the script to finish. A `composit-demo.cast`
file appears in the current directory.

## Verify before shipping

```bash
asciinema play composit-demo.cast
```

Must show:

```
3 errors | 0 warnings | 0 info | 7 passed
```

If the counts drift, do **not** upload — the fixture changed under the
recording. Re-inspect `examples/demo-drift/` and re-run.

## Publish

```bash
asciinema upload composit-demo.cast
```

The returned URL is what you paste into the landing page
(see `landing/index.html`) and any external post body.

## Why this is scripted, not hand-typed

Hand-recording invites typos, variable pacing, and accidental shell
history leaking. The script is deterministic: the same 11 seconds every
time, identical terminal output, safe to re-record on demand when the
diff rendering changes.
