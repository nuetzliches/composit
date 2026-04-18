# Composit Landing Page

Static one-pager that fronts the open-source CLI and funnels visitors
to GitHub-Stars, the Quick-Start, and anonymous feature-interest votes.

Single file, no build step, no framework. Works when served from:

- Caddy (the production target: `composit.public-schloss.nuetzliche.it` on nuts-infra)
- Any static host (GitHub Pages, Cloudflare Pages, Netlify, plain nginx)
- Opened directly (`file://`) for local preview

See [`docs/LANDING-ROLLOUT.md`](../docs/LANDING-ROLLOUT.md) for the
go-live plan.

## Structure

- `index.html` — the whole page. Inline CSS, inline JS, no dependencies.

## No signup on purpose

composit is OSS. There is no "launch" to announce — `cargo install` is
available now. The page collects signal through:

1. **GitHub stars** (primary CTA, outbound-link tracked)
2. **Quick-start copy actions** (did anyone try to install?)
3. **Feature-interest votes** (which direction is worth building?)
4. **Pageviews / referrers** (where is the traffic coming from?)

All four are more robust than email signups for an OSS project. A real
Waitlist can come later if/when a team-tier SaaS materialises with a
concrete beta to sign up for.

## Wiring before going live

One optional wiring left: the Plausible script tag. The Feature-Vote
and Copy-Button handlers already call `window.plausible(...)` if that
global exists. Add this to the `<head>` once Plausible is running:

```html
<script defer data-domain="composit.public-schloss.nuetzliche.it"
        src="https://plausible.public-schloss.nuetzliche.it/js/script.outbound-links.js"></script>
```

`script.outbound-links.js` (Plausible variant) picks up the GitHub-CTA
click automatically — no extra code needed.

Custom goals to configure in Plausible:

- `feature-vote` (props: `feature`)
- `quickstart-copy` (props: `block`)

Outbound-link events for the GitHub-CTA are emitted automatically by
`script.outbound-links.js`.

## Feature slugs

`drift-alerts`, `cost-attribution`, `compliance`, `multi-agent`,
`dashboard`, `policy-runtime`.

Votes are persisted in `localStorage` so a visitor can't double-vote
from the same browser. The Plausible call stays strictly anonymous
(cookieless, no cross-site tracking).

## Signal benchmarks

From [`docs/NEXT-STEPS.md`](../docs/NEXT-STEPS.md), 30 days post HN-Launch:

| Metric                        | Green | Yellow    | Red  |
|-------------------------------|------:|----------:|-----:|
| GitHub stars                  | ≥300  | 100–300   | <100 |
| Landing pageviews             | ≥3000 | 1000–3000 | <1000 |
| Feature-vote top-2 dominance  | ≥50%  | mixed     | flat |

## Editing

Everything is inline and readable top-to-bottom. Colours live under
`:root` CSS variables. Copy is in `<h1>`, `.tagline`, section headings,
`who-card` labels, quickstart blocks, and the `feature-btn` bodies.

Before a launch:

1. Add the Plausible script tag (see above).
2. Swap the social preview image (`og:image`) if one is ready — the
   meta tag isn't set yet because there's no artwork.
3. Confirm the Impressum/Datenschutz URLs in the footer point to the
   right pages on `nuetzliche.it`.
