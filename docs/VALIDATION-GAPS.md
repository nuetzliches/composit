# Composit — Validation Gaps

Identified by cross-referencing `croniq/VALIDATION.md` (2026-04-11) with the
composit concept. These are weaknesses in the current composit documentation
that need to be addressed before any public launch.

---

## Gap 1: The Companion Model Is the Product — But Not Framed That Way

**Source:** croniq VALIDATION.md, Section 5:
> "Webhooks/Notifications wurden in Hookaido ausgelagert — Validierung muss
> das Companion-Modell mitdenken."

**Problem:** croniq validates as a standalone tool (7/10). But its own validation
document flags that the companion model (croniq + hookaido) is unvalidated.
Composit IS this companion model, elevated to a product. But our strategy docs
treat composit as a new idea rather than the natural answer to a known gap.

**Fix:** Frame composit explicitly as the missing piece that the individual
project validations already call for. The narrative should be:
- croniq alone = validated but monetization unclear
- hookaido alone = validated but monetization unclear
- powerbrain alone = validated but monetization unclear
- composit = the layer that makes the combination more valuable than the parts

Add a section to STRATEGY.md: "Composit as the monetization thesis for the
nuetzliches ecosystem."

---

## Gap 2: No Ideal Customer Profile (ICP)

**Source:** croniq has a sharp ICP: "Teams with 20-100 scheduled jobs who've
outgrown cron but won't adopt Airflow."

**Problem:** Composit has no defined ICP. "Creators" is too vague. Who
specifically feels this pain first?

**Candidates:**
- **Solo technical founders** running 5-15 agent-generated services on their
  own infra. No platform team. No time for dashboards. Need a 10-second
  answer to "is everything OK?"
- **Small agency/consultancy CTOs** managing agent-generated solutions across
  multiple client projects. The "which client's webhook broke?" problem.
- **AI-forward startups (5-20 devs)** where agents provision infrastructure
  faster than the team can document. The entropy is already visible.

**Fix:** Pick one primary ICP. Validate it. Write composit's value prop from
their perspective.

---

## Gap 3: Silent Failure at Ecosystem Level — Underexplored

**Source:** croniq's #1 validated pain point is "silent failures / zero
observability." Entire SaaS businesses (Cronitor, Healthchecks.io) exist
because of this.

**Problem:** Composit solves the SAME pain at a higher level — not "this job
failed silently" but "this entire business case degraded silently." We mention
this but don't treat it as the primary selling point.

**The stronger narrative:**
> Cronitor tells you a cron job failed. Composit tells you that your
> PR-review-bot business case is degraded because the scheduling component
> hasn't run in 3 days, the webhook channel has 47 messages in the DLQ,
> and the knowledge collection hasn't been updated since last Tuesday.
> One alert. Full picture.

**Fix:** Make "silent ecosystem failure" the #1 pain point in README and
HN-LAUNCH. It's more compelling than "agents build fast" because it connects
to proven, monetizable pain (see Cronitor/Healthchecks.io revenue).

---

## Gap 4: DSL Fatigue Risk

**Source:** croniq VALIDATION.md, Section 5:
> "Dangerous Assumption: That a DSL (Croniqfile) is a selling point rather
> than a learning curve barrier."

**Problem:** Composit introduces ANOTHER DSL (Compositfile). Users already
potentially learn Croniqfile + Hookaidofile + Compositfile. Three DSLs from
the same ecosystem is a hard sell.

**Options:**
- A) Compositfile uses a different approach entirely (YAML, TOML, JSON)
- B) Compositfile shares syntax with Croniqfile/Hookaidofile (Caddyfile-family)
- C) Compositfile is auto-generated, not hand-written (composit observes,
     creator annotates)

**Leaning:** C is most consistent with the "creator control" thesis. The
creator shouldn't have to write a file to know what they have. Composit
should tell THEM. The Compositfile becomes an output (with creator overrides),
not an input.

**Fix:** Revisit the Compositfile design. If composit is about visibility,
the primary artifact should be auto-generated state, not hand-written config.
The creator's input is policies and business-case annotations — not topology.

---

## Gap 5: Monetization Thesis Missing

**Source:** croniq scores 7/10, with "monetization unproven" as the key weakness.
Open-core with RBAC/audit as paid features is an assumption.

**Problem:** Composit's business model section lists "multi-creator workspaces"
and "managed registry" as commercial features. But it doesn't connect to the
proven willingness-to-pay signals from the croniq validation.

**The insight:** croniq's enterprise buyers want RBAC + audit logs + SLA monitoring.
These are ALL cross-cutting concerns that naturally live in composit, not in
individual tools:
- RBAC across croniq + hookaido + powerbrain = composit feature
- Audit logs across all providers = composit feature
- SLA monitoring across business cases = composit feature
- Cost tracking = composit feature

**Fix:** Position composit as the enterprise layer for the nuetzliches ecosystem.
The individual tools stay lean and OSS-focused. The enterprise upsell lives in
composit. This resolves croniq's monetization problem AND gives composit a clear
revenue thesis.

---

## Gap 6: "Middle Ground" Market Risk

**Source:** croniq VALIDATION.md, Section 5:
> "The 'middle ground' may not be a market — teams might always choose free
> cron or enterprise Airflow/Temporal, skipping the middle."

**Problem:** The same risk applies to composit. Creators might:
- At small scale: "I still have an overview, don't need a tool"
- At large scale: "We hired a platform team / bought Backstage"

The middle (too big for manual tracking, too small for a platform team) might
be empty — or might be huge. It's unvalidated.

**Fix:** This needs real validation. The composit ICP must be someone who is
IN this middle today and feeling the pain. The croniq interviews hint at it:
"Sarah" with 47 cron jobs is exactly the person who would also lose track of
the broader ecosystem. Start there.

---

## Summary: Priority Fixes for Composit

| # | Gap | Priority | Effort |
|---|-----|----------|--------|
| 1 | Frame as companion-model product | High | Low (docs) |
| 2 | Define ICP | High | Medium (research) |
| 3 | "Silent ecosystem failure" as #1 pain | High | Low (rewrite) |
| 4 | Compositfile: auto-generated vs. hand-written | High | Medium (design) |
| 5 | Monetization = enterprise layer | Medium | Low (docs) |
| 6 | Middle-ground market validation | Medium | High (interviews) |
