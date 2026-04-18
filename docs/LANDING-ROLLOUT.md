# Landing-Page Rollout Plan

**Status:** Draft · **Erstellt:** 2026-04-18

Plan für den Go-Live der Composit Landing-Page unter
`composit.nuetzliche.it`. Ziel vor HN-Launch: produktive, EU-konforme
Seite mit strukturierten Signalen (Signups + Feature-Votes).

**Erfolgskriterium:** ≥150 Signups in 30 Tagen (Grün-Benchmark aus
[NEXT-STEPS.md](NEXT-STEPS.md) → Sprint 3).

---

## Architektur

```
Browser (Besucher)
   │ HTTPS
   ▼
Caddy (int-baumeister/services/caddy-core oder caddy-app)
 ├─ composit.nuetzliche.it/            → landing/index.html (statisch)
 ├─ composit.nuetzliche.it/api/signup  → hookaido (Webhook + Delivery)
 ├─ composit.nuetzliche.it/.well-known/composit.json  (Provider-Manifest)
 └─ plausible.nuetzliche.it/           → Plausible Container
       │
       ▼
   Events: pageview + feature-vote (custom goal)
```

### Komponenten-Übersicht

| Komponente | Host | Zweck | Daten |
|---|---|---|---|
| `landing/index.html` | Caddy (statisch) | Marketing-Seite für den Composit **OSS-CLI** | keine |
| `/api/signup` | hookaido via Caddy reverse_proxy | Empfängt Waitlist-Submissions | E-Mail + Feature-Votes → Mail-Delivery an `composit@nuetzliche.it` |
| `/.well-known/composit.json` | Caddy (statisch) | Öffentliches Provider-Manifest (aus [RFC 002 Draft](rfcs/)) | keine (nur Metadaten) |
| Plausible | eigener Compose-Service | Cookieless Analytics, EU-konform | aggregierte Pageviews + Custom Events, keine Personen-IDs |

---

## Phase 1 — Infrastruktur (nuts-infra Repo)

**Aufwand:** ~30–60 Min. Branch: `composit-landing` auf `nuts-infra`.

1. **DNS** — A/AAAA für `composit.nuetzliche.it` und
   `plausible.nuetzliche.it` auf int-baumeister zeigen.
2. **Caddy-vhost** in `int-baumeister/services/caddy-app/Caddyfile`
   (oder caddy-core, je nach bestehender Trennung):

   ```caddy
   composit.nuetzliche.it {
       root * /srv/composit-landing
       file_server
       encode gzip zstd

       header /.well-known/composit.json Access-Control-Allow-Origin *

       handle_path /api/signup {
           rate_limit zone=signup 10r/m
           reverse_proxy hookaido-internal:8080
       }
   }
   ```

3. **Static-Files Deployment** — entweder
   - (a) Caddy direkt aus geklontem `composit` Repo lesen (bind-mount
     `landing/`), **oder**
   - (b) kleiner Forgejo-Actions Job, der bei Push auf composit/main
     den `landing/` Ordner per rsync auf den Host schiebt.

   **Empfehlung:** (a) bis erste Iteration läuft, (b) wenn CI sowieso dran.

4. **`.well-known/composit.json`** von der gleichen Domain ausliefern
   (bereits im Caddyfile oben). Das erfüllt die URL in
   [`examples/Compositfile`](../examples/Compositfile) und powerbrain's
   Manifest-Discovery-Story.

---

## Phase 2 — Plausible (nuts-infra)

**Aufwand:** ~60 Min (neuer compose service).

1. Neuer Ordner `int-baumeister/services/plausible/` mit
   `docker-compose.yml` (Plausible + Clickhouse + Postgres).
   Standard-Setup, ~90 Zeilen compose.
2. Caddy-Site `plausible.nuetzliche.it` → `reverse_proxy` auf den Container.
3. Erste Anmeldung, Site `composit.nuetzliche.it` anlegen,
   **Goals**: `feature-vote`, `Signup`.
4. Tracking-Snippet in `landing/index.html` einfügen:

   ```html
   <script defer data-domain="composit.nuetzliche.it"
           src="https://plausible.nuetzliche.it/js/script.js"></script>
   ```

   Der bestehende `window.plausible(...)`-Call im Skript feuert dann automatisch.

---

## Phase 3 — Waitlist-Backend via hookaido

**Aufwand:** ~45 Min wenn hookaido-Channel-Workflow vertraut.

1. **Channel anlegen** in hookaido: `composit-waitlist`. HMAC-Auth
   bewusst deaktivieren für öffentliches Submit-Endpoint ODER einen
   Public-Token generieren und in Landing-Code hardcoden (niedriges
   Risiko, Rate-Limit in Caddy ergänzen).
2. **Delivery-Targets:**
   - Primary: Mail an `composit@nuetzliche.it` mit Body = gesamte Payload.
   - Sekundär: Persistierung im hookaido-Archiv (wenn DLQ/Archive
     aktiviert — sonst skippen).
3. **Rate-Limit in Caddy:** `rate_limit zone=signup 10r/m` o.ä.,
   gegen Bot-Spam.
4. **Landing-Page wire-up** in `landing/index.html`:

   ```js
   const WAITLIST_ENDPOINT = "/api/signup";
   const ANALYTICS_ENDPOINT = null; // Plausible reicht
   ```

   Form postet `{email, features: [...]}` → Caddy reverse_proxy →
   hookaido → Mail.

---

## Phase 4 — Legal (GDPR/TMG)

**Aufwand:** ~20 Min wenn Texte von `nuetzliche.it` kopiert werden können.

1. Footer-Links in `landing/index.html`: Impressum + Datenschutz.
   Entweder absolute Links auf `nuetzliche.it/impressum` etc., oder
   lokale `impressum.html` / `datenschutz.html` Dateien im `landing/` Ordner.
2. **Datenschutz-Anpassungen** (composit-spezifisch):
   - Waitlist-Signups: gespeicherte Daten (E-Mail + Feature-Votes),
     Zweck (Benachrichtigung bei Launch), Rechtsgrundlage (Art. 6 Abs. 1
     lit. a DSGVO — Einwilligung durch aktives Absenden), Speicherdauer,
     Widerruf.
   - Plausible: cookieless, keine Einwilligung nötig, aber explizit erwähnen.
3. **Consent-Banner:** NICHT erforderlich (Plausible cookieless + keine
   Tracker von Dritten). Das ist ein Feature der Stack-Wahl.

---

## Phase 5 — Landing-Code anpassen (composit Repo)

**Aufwand:** ~15 Min. Branch in composit direkt auf main oder kleiner PR.

Änderungen in `landing/index.html`:

1. `WAITLIST_ENDPOINT = "/api/signup"` (relativ — Page und API auf
   gleicher Domain).
2. Plausible-Script-Tag im `<head>`.
3. Footer-Links auf Impressum/Datenschutz.
4. Optional: `og:image` hinzufügen — kleiner PNG-Screenshot vom
   Terminal-Output-Beispiel wäre stark für HN-Shares.

---

## Phase 6 — Launch-Checkliste

Vor öffentlicher Verlinkung:

- [ ] DNS propagiert (`dig composit.nuetzliche.it +short`)
- [ ] TLS-Zertifikat von Caddy ausgestellt (check in Caddy-Logs)
- [ ] Plausible empfängt Pageview beim eigenen Besuch
- [ ] Test-Signup → E-Mail kommt an mit vollständigen `features[]`
- [ ] Alle 6 Feature-Buttons klickbar, `voted`-CSS funktioniert,
      Plausible-Event feuert (Real-Time-View in Plausible)
- [ ] Responsive-Check auf Mobile (DevTools genügt)
- [ ] Lighthouse-Score ≥90 (Performance/Best-Practices/SEO)
- [ ] Impressum + Datenschutz erreichbar und inhaltlich korrekt
- [ ] `.well-known/composit.json` ausgeliefert (Referenz für powerbrain-Trial)
- [ ] Rate-Limit testweise überschreiten → 429

---

## Phase 7 — Monitoring-Cadence

- **Wöchentlich** (≤10 Min): Plausible-Dashboard checken —
  Pageviews-Trend, Top-Referrer, Feature-Vote-Verteilung.
- **Bei Signup-Mail**: `features[]` aus Body in ein simples
  Spreadsheet/CSV tracken (30-Tage-Zähler).
- **Day 30 nach HN-Launch** (oder nach erstem Traffic-Peak):
  Gegen NEXT-STEPS-Benchmarks messen:
  - `≥150 Signups` → grün, Sprint-4-Entscheidungspunkt mit positivem Signal
  - `50–150 Signups` → gelb, Message/Positioning iterieren (kein Stop)
  - `<50 Signups` → rot, Kill-Criterion prüfen
  - **Top-Feature** muss identifizierbar sein (>30% der Klicks auf eins),
    sonst ist die Wertversprechen-Frage offen.

---

## Risiken und Ausstiegspunkte

| Risiko | Wahrscheinlichkeit | Gegenmittel |
|---|---|---|
| hookaido-Channel-Setup zäher als erwartet | mittel | Fallback: mailto wie aktuell, erstmal live gehen |
| Plausible-Compose komplexer als Standard | niedrig | Umami als leichtere Alternative einplanen |
| DNS-Propagation > 1h | niedrig | Außerhalb Kritischer-Pfad; HN-Post nicht direkt danach |
| Bot-Spam auf `/api/signup` | mittel-hoch | Caddy rate_limit + hookaido Payload-Validation (E-Mail-Regex) + honeypot-Feld in Form |
| Plausible-Script in Landing triggered Ad-Blocker | mittel | Dokumentieren — Feature-Votes funktionieren auch ohne (serverseitige Waitlist-Metrik bleibt primär) |

---

## Out of scope (bewusst weggelassen)

- A/B-Testing verschiedener Taglines — erst mit Traffic-Baseline relevant
- Custom OG-Image-Rendering — Einmal-Screenshot reicht
- Newsletter-System / Drip-Sequenz — NEXT-STEPS sagt explizit
  "No drip sequence, no spam"
- Eigener Signup-Dashboard — Plausible + Mail-Archiv decken die
  Signal-Frage ab, mehr ist premature

---

## Reihenfolge der Umsetzung

Wenn am Stück: **~3–4 Stunden fokussierte Arbeit**, aber _nicht_ an
einem Tag sinnvoll — DNS-Wartezeit und Plausible-Setup-Review wollen
Pause.

**Empfehlung:** 3 Sessions

1. **Session 1:** Phase 1 + Phase 2 (Infra steht, Plausible empfängt
   Events — nichts öffentlich)
2. **Session 2:** Phase 3 + Phase 5 (Waitlist-Loop End-to-End durchgetestet)
3. **Session 3:** Phase 4 + Phase 6 (Legal + Checkliste + Go-Live)
