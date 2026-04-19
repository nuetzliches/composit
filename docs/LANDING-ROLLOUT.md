# Landing-Page Rollout Plan

**Status:** Draft · **Erstellt:** 2026-04-18 · **Revidiert:** 2026-04-18

Plan für den Go-Live der Composit Landing-Page unter
`composit.public-schloss.nuetzliche.it`. Ziel vor HN-Launch: produktive,
EU-konforme Seite.

**Scope-Entscheidung (2026-04-18):** Keine Waitlist / kein Signup-Form,
kein Analytics. composit ist OSS, es gibt keinen "Launch" zu announcen —
`cargo install` ist jederzeit verfügbar. GitHub-Stars sind als externes
Signal robust genug und passen zur Open-Spec / Free-CLI-Positionierung.

**Erfolgskriterium** (30 Tage nach HN-Launch):

- ≥300 GitHub-Stars auf `nuetzliches/composit`

---

## Architektur

```
Browser (Besucher)
   │ HTTPS
   ▼
Caddy (int-baumeister/services/caddy-core oder caddy-app)
 └─ composit.public-schloss.nuetzliche.it/            → landing/index.html (statisch)
```

Keine dynamischen Endpunkte, keine Datenbank, kein hookaido-Channel,
kein Analytics-Backend. Reine Static-Site.

### Komponenten-Übersicht

| Komponente | Host | Zweck | Daten |
|---|---|---|---|
| `landing/index.html` | Caddy (statisch) | Marketing-Seite für den Composit **OSS-CLI** | keine |

### Außerhalb dieses Rollouts

- **Org-Manifest** (`nuetzliche.it/.well-known/composit.json`) —
  separater Rollout auf der Root-Domain. Listet die drei echten
  Provider (croniq, hookaido, powerbrain) als Capabilities. Gehört
  inhaltlich NICHT auf `composit.public-schloss.nuetzliche.it`, weil composit selbst
  kein Provider ist, sondern der CLI der Provider-Manifeste liest.
- **Team-Tier-SaaS** — falls Signale stark genug sind (Sprint-4-Entscheidungspunkt),
  entsteht daraus eine separate Waitlist MIT konkretem Produkt im Beta-Stadium.
  Nicht jetzt.

---

## Phase 1 — Infrastruktur (nuts-infra Repo)

**Aufwand:** ~20–30 Min. Branch: `composit-landing` auf `nuts-infra`.

1. **DNS** — A/AAAA für `composit.public-schloss.nuetzliche.it` auf
   int-baumeister zeigen.
2. **Caddy-vhost** in `int-baumeister/services/caddy-app/Caddyfile`
   (oder caddy-core, je nach bestehender Trennung):

   ```caddy
   composit.public-schloss.nuetzliche.it {
       root * /srv/composit-landing
       file_server
       encode gzip zstd

       # Long cache for static assets, short for HTML
       @assets path *.css *.js *.png *.jpg *.svg *.woff2
       header @assets Cache-Control "public, max-age=31536000, immutable"
       header /index.html Cache-Control "public, max-age=300"
   }
   ```

3. **Static-Files Deployment** — entweder
   - (a) Caddy direkt aus geklontem `composit` Repo lesen (bind-mount
     `landing/`), **oder**
   - (b) kleiner Forgejo-Actions Job, der bei Push auf composit/main
     den `landing/` Ordner per rsync auf den Host schiebt.

   **Empfehlung:** (a) bis erste Iteration läuft, (b) wenn CI sowieso dran.

---

## Phase 2 — Landing-Code anpassen (composit Repo)

**Aufwand:** ~30 Min. Direkt auf main.

Umbau `landing/index.html` gemäß Scope-Entscheidung (keine Waitlist):

1. **Form raus.** Die gesamte `<section id="waitlist">` entfernen.
2. **Primary CTA: GitHub Star.** Hero-Button "Join the waitlist" →
   "★ Star on GitHub". Führt direkt zu `github.com/nuetzliches/composit`.
3. **Quick-Start-Block.** Neue Sektion mit copy-paste-ready Install-Zeile:

   ```bash
   # Install (Rust toolchain required until npx distribution ships)
   cargo install --git https://github.com/nuetzliches/composit

   # Or one-shot: scan any repo without installing
   cargo run --git https://github.com/nuetzliches/composit scan
   ```

   Inline Copy-Button pro Block (clipboard-only, kein Tracking).
4. **Waitlist-Code im `<script>`-Block entfernen** — WAITLIST_ENDPOINT,
   Form-Submit-Handler, mailto-Fallback. Die Feature-Vote-Logik
   (localStorage-Dedup) bleibt als reine Client-UX.
5. **Feature-Voting umframen.** "What would you pay for?" →
   "Which direction should composit take?" — weg vom Kommerz-Framing,
   hin zum Roadmap-Feedback für ein OSS-Projekt.
6. **Footer-Kontakt** statt Form: `composit@nuetzliche.it` als
   einfacher `mailto:` Link. Kein CTA-Gewicht, nur für wenn jemand
   wirklich schreiben will.
7. Optional: `og:image` hinzufügen — Screenshot vom diff-Terminal-Output
   wäre stark für HN-Shares.

---

## Phase 3 — Legal (GDPR/TMG)

**Aufwand:** ~20 Min wenn Texte von `nuetzliche.it` kopiert werden können.

1. Footer-Links in `landing/index.html`: Impressum + Datenschutz.
   Entweder absolute Links auf `nuetzliche.it/impressum` etc., oder
   lokale `impressum.html` / `datenschutz.html` Dateien im `landing/` Ordner.
2. **Datenschutz-Anpassungen** (composit-spezifisch, minimal da keine
   Datenerhebung):
   - Feature-Votes: localStorage-Key `composit-voted-features`, nur
     clientseitig; keine Übertragung, kein Tracking.
   - mailto-Kontakt: keine automatische Verarbeitung, nur manuelle Antwort.
3. **Consent-Banner:** NICHT erforderlich. Die Stack-Wahl (statisch,
   kein Analytics, keine Cookies) ist bewusst so gewählt.

---

## Phase 4 — Launch-Checkliste

Vor öffentlicher Verlinkung:

- [ ] DNS propagiert (`dig composit.public-schloss.nuetzliche.it +short`)
- [ ] TLS-Zertifikat von Caddy ausgestellt (check in Caddy-Logs)
- [ ] GitHub-Star-Button klickbar, führt auf `nuetzliches/composit`
- [ ] Alle 6 Feature-Buttons klickbar, `voted`-CSS funktioniert
- [ ] Quick-Start Copy-Button kopiert Install-Zeile in Clipboard
- [ ] Responsive-Check auf Mobile (DevTools genügt)
- [ ] Lighthouse-Score ≥90 (Performance/Best-Practices/SEO)
- [ ] Impressum + Datenschutz erreichbar und inhaltlich korrekt
- [ ] `composit@nuetzliche.it` mailto funktioniert (Mail kommt an)

---

## Phase 5 — Monitoring-Cadence

- **GitHub-Stars**: nuetzliches/composit Insights-Tab,
  Wochen-Zählerstand notieren.
- **Day 30 nach HN-Launch** (oder nach erstem Traffic-Peak):
  Gegen Erfolgskriterium messen:

  | Metrik | Grün | Gelb | Rot |
  |---|---|---|---|
  | GitHub-Stars | ≥300 | 100–300 | <100 |

  - **Grün** → Sprint-4-Entscheidungspunkt mit positivem Signal,
    Team-Tier-Explorationsphase starten.
  - **Gelb** → Positioning/Message iterieren. Keine neue Feature-Arbeit
    bevor Signal-Quelle stabil ist.
  - **Rot** → Kill-Criterion prüfen. composit bleibt OSS-CLI,
    Fokus auf Reference-Providers.

---

## Risiken und Ausstiegspunkte

| Risiko | Wahrscheinlichkeit | Gegenmittel |
|---|---|---|
| DNS-Propagation > 1h | niedrig | Außerhalb Kritischer-Pfad; HN-Post nicht direkt danach |
| Feature-Votes ohne Signalrückfluss | hoch (by design) | Votes sind reine UX; GitHub-Stars sind das externe Signal |

---

## Out of scope (bewusst weggelassen)

- Waitlist / Signup-Formular — siehe Scope-Entscheidung oben
- hookaido-Channel für Form-Submissions — entfallen mit Waitlist-Entscheidung
- A/B-Testing verschiedener Taglines — erst mit Traffic-Baseline relevant
- Custom OG-Image-Rendering — Einmal-Screenshot reicht
- Newsletter-System / Drip-Sequenz
- Eigener Signup-Dashboard
- Org-Manifest auf `nuetzliche.it` (separater Mini-Rollout)

---

## Reihenfolge der Umsetzung

Wenn am Stück: **~1 Stunde fokussierte Arbeit**. An einem Tag problemlos
machbar — DNS propagiert typischerweise in Minuten, die Code-Umbauten
sind klein.

Phase 1 → 2 → 3 → 4, in Folge.
