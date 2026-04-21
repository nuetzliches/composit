# Composit Landing Page

Static one-pager that fronts the open-source CLI and funnels visitors
to GitHub-Stars, the Quick-Start, and anonymous feature-interest votes.

Single file, no build step, no framework. Works when served from:

- GitHub Pages (production: `nuetzliches.github.io/composit`, deployed via `.github/workflows/pages.yml`)
- Any static host (GitHub Pages, Cloudflare Pages, Netlify, plain nginx)
- Opened directly (`file://`) for local preview

## Structure

- `index.html` — the whole page. Inline CSS, inline JS, no dependencies.

## No signup on purpose

composit is OSS. There is no "launch" to announce — `cargo install` is
available now. The page drives visitors toward:

1. **GitHub stars** (primary CTA, direct link)
2. **Quick-start copy actions** (copy-paste-ready install line)
3. **Asciinema demo** (embedded player, 11s)
4. **Feature-interest links** — each button deep-links to a GitHub
   Discussion so a 👍 reaction is the signal

No tracking, no localStorage state, no backend.

## Feature discussions

Each feature button links to its own discussion under
`github.com/nuetzliches/composit/discussions`:
`drift-alerts`, `cost-attribution`, `compliance`, `multi-agent`,
`dashboard`, `policy-runtime`.

## Editing

Everything is inline and readable top-to-bottom. Colours live under
`:root` CSS variables. Copy is in `<h1>`, `.tagline`, section headings,
`who-card` labels, quickstart blocks, and the `feature-btn` bodies.

Before a launch:

1. Swap the social preview image (`og:image`) if one is ready — the
   meta tag isn't set yet because there's no artwork.
2. Confirm the Impressum/Datenschutz URLs in the footer point to the
   right pages on `nuetzliche.it`.
