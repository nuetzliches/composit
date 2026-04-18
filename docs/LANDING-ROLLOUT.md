# Landing-Page Rollout Plan

**Status:** Draft · **Erstellt:** 2026-04-18 · **Revidiert:** 2026-04-18

Plan für den Go-Live der Composit Landing-Page unter
`composit.public-schloss.nuetzliche.it`. Ziel vor HN-Launch: produktive,
EU-konforme Seite mit strukturierten Signalen.

**Scope-Entscheidung (2026-04-18):** Keine Waitlist / kein Signup-Form.
composit ist OSS, es gibt keinen "Launch" zu announcen — `cargo install`
ist jederzeit verfügbar. GitHub-Stars + cookieless Pageviews +
Feature-Vote-Events sind härtere Signale als Email-Signups und passen
zur Open-Spec / Free-CLI-Positionierung.

**Erfolgskriterien** (30 Tage nach HN-Launch):

- ≥300 GitHub-Stars auf `nuetzliches/composit`
- ≥3000 Landing-Page-Pageviews
- Feature-Votes mit klar identifizierbarem Top-2 (≥50% Anteil)

---

## Architektur

```
Browser (Besucher)
   │ HTTPS
   ▼
Caddy (int-baumeister/services/caddy-core oder caddy-app)
 ├─ composit.public-schloss.nuetzliche.it/            → landing/index.html (statisch)
 └─ plausible.public-schloss.nuetzliche.it/           → Plausible Container
       │
       ▼
   Events: pageview + feature-vote (custom goal)
```

Keine dynamischen Endpunkte, keine Datenbank, kein hookaido-Channel.
Reine Static-Site + Analytics.

### Komponenten-Übersicht

| Komponente | Host | Zweck | Daten |
|---|---|---|---|
| `landing/index.html` | Caddy (statisch) | Marketing-Seite für den Composit **OSS-CLI** | keine |
| Plausible | eigener Compose-Service | Cookieless Analytics, EU-konform | aggregierte Pageviews + Custom Events, keine Personen-IDs |

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

1. **DNS** — A/AAAA für `composit.public-schloss.nuetzliche.it` und
   `plausible.public-schloss.nuetzliche.it` auf int-baumeister zeigen.
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

## Phase 2 — Plausible (nuts-infra)

**Aufwand:** ~60 Min (neuer compose service).

1. Neuer Ordner `int-baumeister/services/plausible/` mit
   `docker-compose.yml` (Plausible + Clickhouse + Postgres).
   Standard-Setup, ~90 Zeilen compose.
2. Caddy-Site `plausible.public-schloss.nuetzliche.it` → `reverse_proxy` auf den Container.
3. Erste Anmeldung, Site `composit.public-schloss.nuetzliche.it` anlegen,
   **Goals**:
   - `feature-vote` (Custom Event, props: `feature`)
   - `github-star-click` (Custom Event, Outbound-Click auf GitHub-CTA)
   - `quickstart-copy` (Custom Event, wenn Copy-Button geklickt wird)
4. Tracking-Snippet in `landing/index.html` einfügen:

   ```html
   <script defer data-domain="composit.public-schloss.nuetzliche.it"
           src="https://plausible.public-schloss.nuetzliche.it/js/script.outbound-links.js"></script>
   ```

   `outbound-links.js` (Plausible-Variante) erfasst GitHub-Clicks automatisch
   als Outbound-Events — kein zusätzlicher JS-Code nötig.

---

## Phase 3 — Landing-Code anpassen (composit Repo)

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

   Inline Copy-Button pro Block (triggert `quickstart-copy` Event).
4. **Waitlist-Code im `<script>`-Block entfernen** — WAITLIST_ENDPOINT,
   Form-Submit-Handler, mailto-Fallback. Die Feature-Vote-Logik
   (localStorage + Plausible-Event) bleibt.
5. **Feature-Voting umframen.** "What would you pay for?" →
   "Which direction should composit take?" — weg vom Kommerz-Framing,
   hin zum Roadmap-Feedback für ein OSS-Projekt.
6. **Footer-Kontakt** statt Form: `composit@nuetzliche.it` als
   einfacher `mailto:` Link. Kein CTA-Gewicht, nur für wenn jemand
   wirklich schreiben will.
7. Optional: `og:image` hinzufügen — Screenshot vom diff-Terminal-Output
   wäre stark für HN-Shares.

---

## Phase 4 — Legal (GDPR/TMG)

**Aufwand:** ~20 Min wenn Texte von `nuetzliche.it` kopiert werden können.

1. Footer-Links in `landing/index.html`: Impressum + Datenschutz.
   Entweder absolute Links auf `nuetzliche.it/impressum` etc., oder
   lokale `impressum.html` / `datenschutz.html` Dateien im `landing/` Ordner.
2. **Datenschutz-Anpassungen** (composit-spezifisch, deutlich reduziert
   ohne Signup):
   - Plausible: cookieless, keine Einwilligung nötig, aber explizit
     erwähnen (IP-Anonymisierung, kein Cross-Site-Tracking, Server in EU).
   - Feature-Votes: localStorage-Key `composit-voted-features`, nur
     clientseitig; Plausible-Events anonym ohne Personenbezug.
   - mailto-Kontakt: keine automatische Verarbeitung, nur manuelle Antwort.
3. **Consent-Banner:** NICHT erforderlich. Die Stack-Wahl (statisch +
   cookieless Analytics) ist bewusst so gewählt.

---

## Phase 5 — Launch-Checkliste

Vor öffentlicher Verlinkung:

- [ ] DNS propagiert (`dig composit.public-schloss.nuetzliche.it +short`)
- [ ] TLS-Zertifikat von Caddy ausgestellt (check in Caddy-Logs)
- [ ] Plausible empfängt Pageview beim eigenen Besuch
- [ ] GitHub-Star-Button klickbar, Outbound-Event landet in Plausible
- [ ] Alle 6 Feature-Buttons klickbar, `voted`-CSS funktioniert,
      Plausible `feature-vote`-Event feuert (Real-Time-View)
- [ ] Quick-Start Copy-Button funktioniert + triggert Event
- [ ] Responsive-Check auf Mobile (DevTools genügt)
- [ ] Lighthouse-Score ≥90 (Performance/Best-Practices/SEO)
- [ ] Impressum + Datenschutz erreichbar und inhaltlich korrekt
- [ ] `composit@nuetzliche.it` mailto funktioniert (Mail kommt an)

---

## Phase 6 — Monitoring-Cadence

- **Wöchentlich** (≤10 Min): Plausible-Dashboard checken
  - Pageviews-Trend
  - Top-Referrer (HN, Reddit, Blog-Links)
  - Feature-Vote-Verteilung
  - GitHub-Outbound-Click-Rate (Pageview → Star-Interesse)
- **GitHub-Stars** (separat): nuetzliches/composit Insights-Tab,
  Wochen-Zählerstand notieren.
- **Day 30 nach HN-Launch** (oder nach erstem Traffic-Peak):
  Gegen Erfolgskriterien messen:

  | Metrik | Grün | Gelb | Rot |
  |---|---|---|---|
  | GitHub-Stars | ≥300 | 100–300 | <100 |
  | Pageviews | ≥3000 | 1000–3000 | <1000 |
  | Feature-Votes Top-2 | ≥50% klarer Schwerpunkt | gemischt | keine Dominanz |

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
| Plausible-Compose komplexer als Standard | niedrig | Umami als leichtere Alternative einplanen |
| DNS-Propagation > 1h | niedrig | Außerhalb Kritischer-Pfad; HN-Post nicht direkt danach |
| Plausible-Script in Landing triggered Ad-Blocker | mittel | GitHub-Stars als robustes primäres Signal — Pageviews sind sekundär |
| Feature-Votes zu wenig, keine klare Tendenz | mittel | Framing-Iteration; mehr Traffic zuerst abwarten |

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

Wenn am Stück: **~2 Stunden fokussierte Arbeit** (deutlich weniger als
der ursprüngliche Plan mit Waitlist-Backend). An einem Tag machbar,
aber DNS-Wartezeit plus Plausible-Review lohnen eine zweite Session.

**Empfehlung:** 2 Sessions

1. **Session 1:** Phase 1 + Phase 2 (Infra steht, Plausible empfängt
   Events — Landing noch alte Version)
2. **Session 2:** Phase 3 + Phase 4 + Phase 5 (Code-Umbau, Legal, Go-Live)
