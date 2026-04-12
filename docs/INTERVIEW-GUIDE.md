# Composit — Interview-Leitfaden / Interview Guide

Zweisprachig: Deutsch (Hauptsprache) + *English (italic)*

---

## Zweck / Purpose

Wir validieren ob der Pain "Agent Infrastructure Visibility" real genug ist,
dass Platform Engineers und CTOs dafuer zahlen wuerden. Methode: Mom Test —
wir fragen nach bestehendem Verhalten, nicht nach hypothetischer Nutzung.

*We're validating whether the pain of "Agent Infrastructure Visibility" is
real enough that platform engineers and CTOs would pay for a solution.
Method: Mom Test — we ask about existing behavior, not hypothetical usage.*

### Was wir herausfinden wollen

| Frage | Validiert Dimension |
|-------|-------------------|
| Wie organisieren sie den Ueberblick ueber Agent-erstellte Ressourcen? | Problem Severity |
| Was investieren sie heute in Tooling und Prozesse rund um Infrastructure Tracking? | Existing Spending |
| Wie laeuft die Tooling-Entscheidung im Team ab? | Monetization Clarity |
| Welche regulatorischen Anforderungen beschaeftigen das Team aktuell? | Monetization Clarity |

---

## Zielgruppen / Target Personas

### Platform Engineer
Verwaltet Infrastruktur fuer Teams die AI Agents nutzen. Wird gepaged wenn
etwas kaputt geht. Fuehrt Drift-Audits durch.
*Manages infrastructure for teams using AI agents. Gets paged when things
break. Runs drift audits.*

### CTO / Engineering Manager
Verantwortlich fuer Budget, Compliance, Team-Produktivitaet. Beschaeftigt
sich mit regulatorischen Anforderungen (SOC2, EU AI Act) und Cloud-Kosten.
*Responsible for budget, compliance, team productivity. Navigating
regulatory requirements (SOC2, EU AI Act) and cloud cost management.*

### DevOps Lead
Hands-on aber auch Team-Verantwortung. Betreibt Multi-Cloud-Setups.
Arbeitet daran, Agent-generierte Ressourcen in bestehende IaC-Workflows
zu integrieren.
*Hands-on but also team responsibility. Runs multi-cloud setups.
Working on integrating agent-created resources into existing IaC workflows.*

---

## Interview-Ablauf (~40 Minuten)

### 1. Warm-up (5 min)

**1.1** Erzaehl kurz was du machst — Rolle, Team-Groesse, Tech-Stack.
*Tell me briefly what you do — role, team size, tech stack.*

**1.2** Welche AI Coding Agents nutzt dein Team? (Claude Code, Cursor, Copilot, Devin, andere?)
*Which AI coding agents does your team use?*

**1.3** Seit wann nutzt ihr die? Wie intensiv?
*How long have you been using them? How intensively?*

**1.4** Wer hat entschieden sie einzufuehren?
*Who made the decision to adopt them?*

---

### 2. Bestehendes Verhalten: Was passiert heute? (15 min)

> Ziel: Verstehen was sie TATSAECHLICH tun, nicht was sie tun WUERDEN.
> *Goal: Understand what they ACTUALLY do, not what they WOULD do.*

**2.1** Wie ist bei euch der Prozess, wenn ein Agent etwas Neues erstellt —
eine Funktion, eine Datenbank, einen Cron-Job? Wie fliesst das in euren
Ueberblick ein?
*What's your process when an agent creates something new — a function,
a database, a cron job? How does that flow into your overview?*

**2.2** Wie organisiert ihr den Ueberblick ueber alle Services und
Ressourcen in eurem Setup? Gibt es ein zentrales Inventar?
*How do you organize the overview of all services and resources in your
setup? Is there a central inventory?*

**2.3** Wie laeuft es typischerweise, wenn ihr auf eine Ressource stosst,
die nicht dokumentiert war? Wie geht ihr damit um?
*What typically happens when you come across a resource that wasn't
documented? How do you handle it?*

**2.4** Wie viel Zeit fliesst pro Woche in das Pflegen eures Infrastruktur-
Ueberblicks? (Inventar-Updates, Dashboard-Checks, State-Abgleich)
*How much time per week goes into maintaining your infrastructure overview?
(Inventory updates, dashboard checks, state reconciliation)*

**2.5** Gab es in den letzten 6 Monaten eine Situation, in der der
Ueberblick ueber eure Infrastruktur besonders wichtig war?
Was war der Anlass?
*In the last 6 months, was there a situation where having a clear
picture of your infrastructure was especially important? What triggered it?*

**2.6** Wenn ihr euren Infrastruktur-Ueberblick verbessern wolltet —
wo waere der groesste Hebel?
*If you wanted to improve your infrastructure overview — where would
the biggest lever be?*

---

### 3. Loesungsversuche: Was habt ihr probiert? (10 min)

> Ziel: Verstehen welche Loesungen sie SCHON getestet haben und warum die nicht reichen.
> *Goal: Understand which solutions they've ALREADY tried and why they fell short.*

**3.1** Welche Tools oder Prozesse nutzt ihr aktuell fuer Infrastructure
Tracking und Inventory Management?
*Which tools or processes do you currently use for infrastructure
tracking and inventory management?*

**3.2** Was davon funktioniert gut fuer euch? Wo seht ihr Verbesserungspotenzial?
*What works well for you? Where do you see room for improvement?*

**3.3** Wie sieht das Investment in diese Tools / Prozesse aus?
(Lizenzkosten, Personalzeit, Wartungsaufwand)
*What does the investment in these tools / processes look like?
(License costs, personnel time, maintenance effort)*

**3.4** Habt ihr euch schon nach Alternativen umgesehen?
Was ist euch dabei aufgefallen?
*Have you looked into alternatives? What did you notice?*

---

### 4. Zahlungsbereitschaft (indirekt) (5 min)

> Ziel: Budget-Kontext verstehen ohne direkt "Wuerdest du zahlen?" zu fragen.
> *Goal: Understand budget context without directly asking "Would you pay?"*

**4.1** Welche DevOps/Platform-Tools bezahlt ihr aktuell?
(Backstage-Hosting, Datadog, Spacelift, Vanta, PagerDuty, etc.)
Was kosten die ungefaehr?
*Which DevOps/platform tools do you currently pay for? Rough cost?*

**4.2** Wer entscheidet bei euch ueber Tooling-Budget? Wie laeuft der
Prozess ab?
*Who decides on tooling budget at your company? How does the process work?*

**4.3** Wenn du an den Aufwand denkst, den ihr heute fuer das Tracking
betreibt — haettet ihr diesen Aufwand lieber als Tool-Kosten oder als
Personalzeit?
*Thinking about the effort you spend on tracking today — would you rather
have that as tool costs or personnel time?*

**4.4** Bis zu welchem Betrag kannst du ein Tool selbst entscheiden
ohne Genehmigung?
*Up to what amount can you decide on a tool yourself without approval?*

---

### 5. Regulatorische Anforderungen (5 min)

> Ziel: Verstehen welche regulatorischen Themen das Team beschaeftigen
> und wie sie sich darauf vorbereiten.
> *Goal: Understand which regulatory topics occupy the team and how
> they're preparing.*

**5.1** Welche Zertifizierungen oder regulatorischen Anforderungen
sind fuer euer Team aktuell relevant? (SOC2, ISO27001, GDPR, etc.)
*Which certifications or regulatory requirements are currently
relevant for your team?*

**5.2** Wie bereitet ihr euch auf Audits vor? Welche Schritte
gehoeren zur Vorbereitung?
*How do you prepare for audits? What steps are part of the preparation?*

**5.3** Welche neuen regulatorischen Entwicklungen beobachtet ihr
aktuell? (z.B. EU AI Act, branchenspezifische Anforderungen)
*Which new regulatory developments are you currently watching?
(e.g., EU AI Act, industry-specific requirements)*

**5.4** Hat sich euer Tooling in den letzten 12 Monaten aufgrund
regulatorischer Anforderungen veraendert? Was wurde angepasst?
*Has your tooling changed in the last 12 months due to regulatory
requirements? What was adjusted?*

---

### 6. Abschluss (2 min)

**6.1** Wenn du eine Sache an eurem aktuellen Setup aendern koenntest,
was waere das?
*If you could change one thing about your current setup, what would it be?*

**6.2** Kennst du andere Platform Engineers / CTOs die aehnliche
Herausforderungen haben? Duerfte ich die kontaktieren?
*Do you know other platform engineers / CTOs with similar challenges?
Could I reach out to them?*

**6.3** Duerfen wir in 4 Wochen nochmal sprechen? Wir bauen gerade
ein Tool das genau dieses Problem loest und wuerden gerne euer Feedback
zur ersten Version holen.
*Can we talk again in 4 weeks? We're building a tool that addresses
exactly this problem and would love your feedback on the first version.*

---

## Auswertungs-Schema / Evaluation Schema

Nach jedem Interview fuellst du diese Matrix aus:

### Signal-Staerke pro Dimension

| Dimension | Signal | Bewertung |
|-----------|--------|-----------|
| **Problem real?** | Beschreibt konkreten Pain mit Beispielen | Stark / Mittel / Schwach |
| **Workaround-Kosten?** | Nennt Zahlen: Stunden, Euro, Incidents | Stark / Mittel / Schwach |
| **Loesungen probiert?** | Hat aktiv nach Loesungen gesucht | Stark / Mittel / Schwach |
| **Zahlungsbereitschaft?** | Hat Budget-Kontext, koennte entscheiden | Stark / Mittel / Schwach |
| **Compliance-Druck?** | Harter Deadline, Audit-Angst | Stark / Mittel / Schwach |
| **Referral?** | Kennt andere mit dem Problem | Ja / Nein |

### Verdichtung ueber alle Interviews

| Metrik | Ziel (nach 10+ Interviews) |
|--------|---------------------------|
| % die den Pain als "must have" beschreiben | > 50% = starkes Signal |
| Durchschnittliche Workaround-Kosten | > $200/Monat = Existing Spending validiert |
| % die selbst Budget entscheiden koennten | > 30% = Monetization Clarity verbessert |
| % mit aktivem Compliance-Druck | > 40% = Compliance als Kauftrigger validiert |
| Anzahl Referrals | > 5 = organisches Netzwerk fuer Early Access |

### Kill-Signal

Wenn nach 10 Interviews **keiner** den Pain als "must have" beschreibt
und **keiner** Budget-Kontext nennen kann → zurueck zu ITERATE, Positionierung
ueberdenken.

---

## Anti-Patterns: Was wir NICHT tun

### 1. Keine Loesung pitchen
Nicht: "Wir bauen composit, ein Tool das X kann — wuerdest du das nutzen?"
Sondern: "Wie loest ihr das heute?"
*Don't pitch the solution. Ask about their current behavior.*

### 2. Keine hypothetischen Fragen
Nicht: "Wuerdest du fuer ein Tool zahlen?"
Sondern: "Welche Tools bezahlt ihr heute? Was kosten die?"
*Don't ask hypothetical questions. Ask about real spending.*

### 3. Keine Komplimente fischen
Wenn jemand sagt "Klingt toll!" — das ist kein Signal.
Signal ist: "Ich verbringe 4 Stunden pro Woche damit und es nervt mich."
*"Sounds great!" is not a signal. "I spend 4 hours per week on this" is.*

### 4. Nicht zu frueh ueber Features reden
Erst den Pain vollstaendig verstehen. Dann, wenn ueberhaupt, am Ende
kurz erwaehnen was wir bauen.
*Understand the pain fully before mentioning what we're building.*

### 5. Keine fuehrenden Fragen
Nicht: "Ist es frustrierend wenn Agents Dinge erstellen ohne dass ihr es wisst?"
Sondern: "Wie fliesst es in euren Ueberblick ein, wenn ein Agent etwas erstellt?"
*Don't lead. Ask open questions.*

### 6. Nicht nach Rechtsverstoessen fragen
Fragen so formulieren, dass niemand preisgeben muss, gegen geltendes
Recht zu verstossen. "Wie organisiert ihr X?" statt "Habt ihr X unter
Kontrolle?" — EU AI Act, DSGVO, SOC2 sind sensible Themen.
*Frame questions so nobody has to reveal they're violating regulations.
"How do you organize X?" not "Do you have X under control?"*

---

## Kanaele fuer Interview-Partner / Channels

| Kanal | Persona | Ansprache |
|-------|---------|-----------|
| LinkedIn (Platform Eng, DevOps) | Platform Eng, DevOps Lead | DM mit konkreter Frage zum Agent-Workflow |
| CNCF Slack (#platform-engineering) | Platform Eng | Thread zu Agent-Sprawl-Erfahrungen starten |
| r/devops, r/platformengineering | Alle | Post: "Wie trackt ihr was eure AI Agents erstellen?" |
| Lokale Meetups (DevOps, Cloud Native) | Alle | Lightning Talk + Gespraech danach |
| Eigenes Netzwerk | CTO, DevOps Lead | Direkte Anfrage an bekannte CTOs/Devs |
| Claude Code Discord / Community | Platform Eng | Frage nach Infrastruktur-Tracking-Workflow |

---

## Vorlage: Interview-Notizen / Template: Interview Notes

```markdown
# Interview: [Name / Pseudonym]
**Datum:** YYYY-MM-DD
**Rolle:** [Platform Eng / CTO / DevOps Lead]
**Team-Groesse:** [X Devs]
**Agents im Einsatz:** [Claude Code, Cursor, ...]

## Kern-Findings
- Pain real? [Ja/Nein + Beispiel]
- Workaround-Kosten: [X Stunden/Woche, Y EUR/Monat]
- Loesungen probiert: [Tool A, Tool B — warum nicht ausreichend]
- Zahlungsbereitschaft: [Budget-Kontext, Entscheidungs-Betrag]
- Compliance-Druck: [SOC2/EU AI Act/Keiner]

## Signal-Matrix
| Dimension | Bewertung |
|-----------|-----------|
| Problem real | Stark / Mittel / Schwach |
| Workaround-Kosten | Stark / Mittel / Schwach |
| Loesungen probiert | Stark / Mittel / Schwach |
| Zahlungsbereitschaft | Stark / Mittel / Schwach |
| Compliance-Druck | Stark / Mittel / Schwach |
| Referral | Ja / Nein |

## Zitate (woertlich)
- "..."

## Notizen
[Freiform]
```

---

*Stand: 2026-04-12. Basiert auf Business Validation v2 (7.0 BUILD).*
