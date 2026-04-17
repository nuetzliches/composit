# Composit Landing Page

Static one-pager that fronts the open-source CLI and collects the first
external signal (waitlist signups + anonymous feature-interest votes).

Single file, no build step, no framework. Works when served from:
- GitHub Pages (`landing` subdirectory → `docs/` convention, or a
  dedicated branch)
- Any static host (Cloudflare Pages, Netlify, Vercel, plain S3/nginx)
- Opened directly (`file://`) for local preview

## Structure

- `index.html` — the whole page. Inline CSS, inline JS, no dependencies.

## Wiring before going live

Two placeholders need real backends before this replaces a hand-rolled
"coming soon" page:

1. **Waitlist endpoint** — `WAITLIST_ENDPOINT` in the inline script
   (defaults to `null`). Any JSON-accepting form handler works
   (Formspree, Plunk, Buttondown, a minimal self-hosted endpoint). The
   form POSTs `{ email, features: [...] }`.

   Fallback (`null` endpoint): the form triggers a `mailto:` to
   `FALLBACK_EMAIL` with the features pre-filled in the body. Good
   enough for day-one signups without infrastructure.

2. **Feature-vote endpoint** — `ANALYTICS_ENDPOINT` in the inline script
   (defaults to `null`). Accepts `{ feature: "<slug>" }`. If you run
   Plausible or Umami, the script already fires `feature-vote` custom
   events automatically — no endpoint change needed.

Feature slugs:
`drift-alerts`, `cost-attribution`, `compliance`, `multi-agent`,
`dashboard`, `policy-runtime`.

Votes are also persisted in `localStorage` so a visitor can't double-vote
from the same browser.

## Signal benchmarks

From `docs/NEXT-STEPS.md`:

| Signal                         | Result                 |
|--------------------------------|------------------------|
| ≥150 signups / 30 days         | Green — proceed        |
| 50–150 signups, no clear feature | Yellow — iterate message |
| <50 signups                    | Red — positioning wrong |

Feature votes should also converge on a top-2. If no feature dominates,
the paid tier's proposition isn't clear yet.

## Editing

Everything is inline and readable top-to-bottom. Colours live under
`:root` CSS variables. Copy is in `<h1>`, `.tagline`, section headings,
`who-card` labels, and the `feature-btn` bodies.

Before a launch:

1. Swap the social preview image (`og:image`) if one is ready — the
   meta tag isn't set yet because we don't have artwork.
2. Replace `FALLBACK_EMAIL` if you're routing signups elsewhere.
3. Point `WAITLIST_ENDPOINT` / `ANALYTICS_ENDPOINT` at real services.
