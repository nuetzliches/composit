# Composit Landing Page

Static one-pager that fronts the open-source CLI and funnels visitors
to GitHub-Stars, the Quick-Start, and anonymous feature-interest votes.

Single file, no build step, no framework. Works when served from:

- Caddy (the production target: `composit.nuetzliche.it` on nuts-infra)
- Any static host (GitHub Pages, Cloudflare Pages, Netlify, plain nginx)
- Opened directly (`file://`) for local preview

See [`docs/NEXT-STEPS.md`](../docs/NEXT-STEPS.md) for the
go-live plan.

## Structure

- `index.html` — the whole page. Inline CSS, inline JS, no dependencies.

## No signup on purpose

composit is OSS. There is no "launch" to announce — `cargo install` is
available now. The page drives visitors toward:

1. **GitHub stars** (primary CTA, direct link)
2. **Quick-start copy actions** (copy-paste-ready install line)
3. **Feature-interest votes** (client-side UX only)

GitHub stars are the primary external signal; the rest are lightweight
UX to make the page feel alive. A real Waitlist can come later if/when
a team-tier SaaS materialises with a concrete beta to sign up for.

## Feature slugs

`drift-alerts`, `cost-attribution`, `compliance`, `multi-agent`,
`dashboard`, `policy-runtime`.

Votes are persisted in `localStorage` so a visitor can't double-vote
from the same browser. No tracking, no network call — purely local UX.

## Signal benchmarks

From [`docs/NEXT-STEPS.md`](../docs/NEXT-STEPS.md), 30 days post HN-Launch:

| Metric        | Green | Yellow  | Red  |
|---------------|------:|--------:|-----:|
| GitHub stars  | ≥300  | 100–300 | <100 |

## Editing

Everything is inline and readable top-to-bottom. Colours live under
`:root` CSS variables. Copy is in `<h1>`, `.tagline`, section headings,
`who-card` labels, quickstart blocks, and the `feature-btn` bodies.

Before a launch:

1. Swap the social preview image (`og:image`) if one is ready — the
   meta tag isn't set yet because there's no artwork.
2. Confirm the Impressum/Datenschutz URLs in the footer point to the
   right pages on `nuetzliche.it`.
