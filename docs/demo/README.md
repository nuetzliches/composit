# Asciinema Recording — Composit Demo

The HN-facing walkthrough. Shows `composit scan` and `composit diff` on
the `examples/demo-drift/` workspace end-to-end in ~35 seconds.

## Dependencies

- `asciinema` (`apt install asciinema` or `brew install asciinema`)
- A built `composit` binary, reachable as one of:
  - `composit` on `$PATH` (preferred — matches the install story),
  - `target/debug/composit` (the script will fall back to it), or
  - `$COMPOSIT` env var pointing at a binary.

## Record

```bash
cargo build                                                # if not installed
asciinema rec composit-demo.cast --cols 100 --rows 30 \
  -c "bash docs/demo/record.sh"
```

`--cols 100` matters — the default 80-col width wraps the scan/diff output
(the `docker-compose.yml` row, the `unapproved_provider` error) and makes
the recording look sloppy. 100×30 fits every line without wrapping.

Press `Ctrl-D` or wait for the script to finish. A `composit-demo.cast`
file appears in the current directory.

## Verify before shipping

```bash
asciinema play composit-demo.cast
```

Must show:

```
3 errors | 0 warnings | 0 info | 6 passed
```

If the counts drift, do **not** upload — the fixture changed under the
recording. Re-inspect `examples/demo-drift/` and re-run.

## Publish

```bash
asciinema upload composit-demo.cast
```

The returned URL embeds directly into the HN post body
(`docs/HN-LAUNCH.md`) and the landing page.

## Why this is scripted, not hand-typed

Hand-recording invites typos, variable pacing, and accidental shell
history leaking. The script is deterministic: the same 35 seconds every
time, identical terminal output, safe to re-record on demand when the
diff rendering changes.
