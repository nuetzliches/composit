# Show HN draft — Composit

Ready-to-paste post body for news.ycombinator.com/submit. Voice: concrete,
no hype, "what it does / what it doesn't". One demo link, one repo link,
one honest "what's next".

---

## Title options (pick one)

**Primary (recommended):**

> Show HN: Composit — see every service your AI agents created in your infra

**Alternatives:**

1. `Show HN: Composit — governance-as-code for AI-generated infrastructure`
2. `Show HN: A CLI that diffs your real infra against a declared Compositfile`
3. `Show HN: composit scan + diff — IS-state vs. SHOULD-state for agent sprawl`

Title goes under 80 chars. Keep the pragmatic framing — HN rewards "what it does"
over "what it could become".

---

## Post body

> Composit is an open-source CLI for infrastructure governance.
> 
> `composit scan` walks a repo and produces a machine-readable inventory:
> docker-compose services, K8s manifests, Helm charts, Terraform, workflows,
> cron jobs, MCP servers, nginx sites, Grafana dashboards, OPA policies — 13
> scanners today. `Compositfile` (HCL) is the SHOULD-state you declare:
> approved providers, budgets, resource limits, policies. `composit diff`
> surfaces the gap.
> 
> **Demo (11 seconds):** https://asciinema.org/a/EOPTSBcM0k3wGbvh
> 
> The demo runs against a synthetic widgetshop workspace shipped in the
> repo. Three canonical drifts fire — one unapproved provider, one `:latest`
> image outside the allowlist, one missing required workflow:
> 
> ```
> 3 errors | 0 warnings | 0 info | 7 passed
> ```
> 
> You can reproduce it in 30 seconds:
> 
> ```
> cargo install --git https://github.com/nuetzliches/composit
> composit scan --dir examples/demo-drift --no-providers
> composit diff --dir examples/demo-drift --offline
> ```
> 
> **Why I built it.** Infrastructure governance was already broken before AI
> — most teams can't answer "what services do we run, who created them, do
> they match our contracts?" in machine-readable form. AI agents make that
> urgent: Claude Code, Cursor, Devin provision real infrastructure faster
> than humans can track. After a few weeks nobody has the full picture.
> 
> Composit doesn't deploy anything. It reads. The scanner is read-only by
> design, the Compositfile is reviewed like any other config, and the diff
> has a `--strict` mode that fails CI when governance drifts from reality.
> Composit already scans itself in its own CI: see the self-scan step in
> `.github/workflows/ci.yml`.
> 
> **What it's not.** Not an AI monitoring tool (attribution is metadata,
> not the core). Not a deployment tool (that's Terraform/Pulumi). Not a
> service catalog (not Backstage or Port). Not a policy engine (it
> integrates with OPA, doesn't replace it).
> 
> **Open.** The report schema, the provider manifest, and the contract
> envelope are published as RFCs (001-003 draft) with JSON Schemas. Anyone
> can implement a Composit-compatible provider — we ship three reference
> ones at nuetzliche.it. The CLI is MIT, single-binary Rust, no SaaS
> required.
> 
> **Rough edges, by design.** v0.1 is CLI-only — no web dashboard, no
> team-tier, no hosted registry. OPA runtime evaluation is on the roadmap
> (we parse `.rego` today, we don't yet evaluate it). Contract-tier
> provider flows work end-to-end but only one public reference provider
> is live.
> 
> I'd especially love feedback from platform engineers: does this match a
> pain you have, or is the diff output too opinionated / not opinionated
> enough? Compositfile syntax — HCL — ergonomic or not?
> 
> Repo (scan it on your own stack!): https://github.com/nuetzliches/composit

---

## First comment (post immediately after submission)

Post this yourself as the first comment — HN convention, sets the tone
before random threads take over. Paste verbatim:

> Author here. A few things I'd pre-emptively flag:
> 
> - **Scope.** This is the v0.1 CLI. There's no web app, no hosted registry,
>   no auth backend. The spec (RFCs 001-003, JSON Schemas) is what we want
>   to stabilise with community input before piling on product.
> 
> - **Attribution.** Git-blame + `Co-Authored-By` gets you Claude Code and
>   Copilot for free; Cursor / ChatGPT copy-paste is not detectable. It's a
>   signal, not a fact. The IS-vs-SHOULD compare works regardless of who
>   created what.
> 
> - **Why a new Compositfile instead of extending Terraform/OPA?** Terraform
>   is `.tf`-centric (it IS the SHOULD-state for what it owns). Composit
>   reads deckarations across tools — compose, K8s, CI, cron, MCP — that
>   Terraform doesn't know about, and compares them to a small governance
>   file that fits on a screen. The two are complements.
> 
> - **How fast?** On a 30-service monorepo the scan is sub-second offline;
>   online provider checks are the latency budget.
> 
> Happy to dig into any of this.

---

## Anticipated Q&A

Quick reference during the thread. Keep answers short and concrete.

**"Isn't this just Backstage/Port?"**
No. Those are service catalogs you populate. Composit reads declarations
that already exist in the repo. Closer to `terraform plan` than Backstage.

**"Why not a web UI?"**
v0.1 is a CLI because a CLI is diff-able, script-able, CI-able, and
single-binary. Team tier with a dashboard is roadmap, but the spec +
CLI need adopters first.

**"Rust?"**
Single-binary distribution, no runtime dependencies, fast on large repos.
`cargo install` today; npx wrapper on the roadmap.

**"Does it touch my infrastructure?"**
No. Scan is read-only — filesystem walks plus optional GET requests to
provider manifest URLs you declare. It never writes to your compose files,
never calls cloud APIs, never modifies state.

**"Can I write my own scanner?"**
Extra patterns via the `scan { extra_patterns { … } }` block in the
Compositfile covers most cases. Native scanner plugins are a deliberate
non-goal for v0.1 (quality control); a proper plugin API lands when 10+
people ask for one.

**"OPA runtime evaluation?"**
We parse `.rego` files (package, rules, entrypoints) but don't yet evaluate
them against scan-derived inputs. That's the next major CLI milestone —
tracked in NEXT-STEPS.

**"Compositfile spec?"**
RFC 004, pending. The current parser is the de-facto spec until then.

---

## Where to post

- HN Show HN: https://news.ycombinator.com/submit
- Subreddits to cross-post **after** HN (wait 4-6 h to avoid dilution):
  r/devops, r/platformengineering, r/rust (tooling angle)
- Slacks: Platform Engineering Slack `#show-and-tell`, CNCF
  `#governance`

## What to measure (30-day window)

Matches the signal-benchmark table in `docs/NEXT-STEPS.md`:

| Metric         | Green  | Yellow    | Red     |
|----------------|--------|-----------|---------|
| GitHub stars   | ≥300   | 100-300   | <100    |
| Pageviews      | ≥3000  | 1000-3000 | <1000   |
| External PRs   | ≥2     | 1         | 0       |
| New providers  | ≥1     | 0         | —       |

Green on 2+ axes → proceed with RFC finalisation and team tier exploration.
Mixed → positioning iteration, not kill. Red across the board → see Kill
Criteria in NEXT-STEPS.

## Checklist before submitting

- [ ] Repo README renders cleanly on github.com (no broken links)
- [ ] asciinema demo plays on mobile
- [ ] `cargo install --git https://github.com/nuetzliches/composit`
      works on a clean machine
- [ ] `composit scan --dir examples/demo-drift --no-providers` + diff runs
      with the exact 3-error output the post promises
- [ ] GitHub Discussions enabled and the six feature-interest discussions
      are live (already done, commit `ff4351d`)
- [ ] Landing page reachable at the URL linked in the README
