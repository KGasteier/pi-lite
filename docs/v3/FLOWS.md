# cocopi v3 · Nutzerflüsse (S1, I1)

**Form:**
- Jeder Schritt hat **eine** Hauptaktion, einen **sichtbaren Abschluss** und einen **Rückweg**.
- Die Orte stammen aus INVENTAR.md, z. B. D-ST oder M-LI.
- Texte in „…“ sind wörtlich gemeint.
- Die Persona ist **Mira** (Kap. 1 Tutorial), Slot `account03`.

---

## Hauptansichten (ASCII)

### Desktop · Startkarte (D-ST), nicht angemeldet

```
┌──────────────────────────────────────────────────────────────┐
│ cocopi                                       Datei Ansicht Hilfe│
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                Willkommen bei cocopi                         │
│                                                              │
│   Du hast eine Einladung bekommen – als Link oder als        │
│   QR-Code. Öffne den Link, oder füge ihn hier ein.           │
│                                                              │
│   ┌──────────────────────────────────────┐  ┌───────────┐    │
│   │ Einladungslink oder Code             │  │ Anmelden  │    │
│   └──────────────────────────────────────┘  └───────────┘    │
│                                                              │
│   Keine Einladung? Frag deine Seminarleitung.                │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ ☁ Nur Remote · nicht angemeldet                              │
└──────────────────────────────────────────────────────────────┘
```

### Desktop · Arbeiten (D-SL + D-KZ + D-CH + D-EI + D-SZ)

```
┌───────────────┬──────────────────────────────────────────────┐
│ Sessions      │ Projekt: Abschlussarbeit ▾     [Chat|Dateien]│  ← D-KZ
│ ┌───────────┐ ├──────────────────────────────────────────────┤
│ │+ Neue Sess│ │                                              │
│ └───────────┘ │  Du: Lies notes.md und mach eine Gliederung. │
│               │                                              │
│ ● Gliederung  │  Agent: Ich lese notes.md …                  │  ← D-CH
│   arbeitet    │  ▸ read notes.md                             │
│   2:14     ⋯  │                                              │
│               │  ┌ Rückfrage ───────────────────────────┐    │
│ ○ Bildreihe   │  │ Soll ich outline.md anlegen?          │    │
│   ruht     ⋯  │  │            [Nein]  [Ja, anlegen]      │    │
│               │  └───────────────────────────────────────┘    │
│               │  arbeitet · 2:14                             │
│               ├──────────────────────────────────────────────┤
│               │ ┌──────────────────────────────┐ Modell ▾    │  ← D-EI
│               │ │ Nachricht an den Agenten …   │ [ Stopp ]   │
│               │ └──────────────────────────────┘             │
├───────────────┴──────────────────────────────────────────────┤
│ ☁ Nur Remote · Mira · account03 @ R2 · verbunden · Verbrauch │  ← D-SZ
└──────────────────────────────────────────────────────────────┘
```

### Desktop · Dateien (D-DA)

```
├──────────────────────────────────────────────────────────────┤
│ Projekt: Abschlussarbeit ▾                    [Chat|Dateien] │
├──────────────────────────────────────────────────────────────┤
│  Auf R2                                 Auf diesem Mac       │
│  notes.md          vor 2 min            notes.md      gleich │
│  outline.md   neu  vor 1 min            –                    │
│  bilder/ (12)                           bilder/ (12)  gleich │
│                                                              │
│  [Nach R2 übertragen …]  [Änderungen prüfen]                 │
│                          [Ergebnisse auf den Mac holen …]    │
│  ── Vorschau: outline.md ─────────────────────────────────── │
│  # Gliederung …                                              │
├──────────────────────────────────────────────────────────────┤
```

Hauptaktion je Zustand:
- Gibt es neue Dateien auf R2, ist „Ergebnisse … holen“ der hervorgehobene Knopf.
- Sonst ist es „Nach R2 übertragen“.

Die beiden anderen Knöpfe sind sekundär.

### Desktop · Einstellungen › Geräte (D-SE)

```
┌ Einstellungen ───────────────────────────────────────────────┐
│ Konto │ Geräte │ Modus │ Darstellung │ Vorlesen │ Erweitert  │
├──────────────────────────────────────────────────────────────┤
│ Geräte von account03                                         │
│  Miras MacBook  · Rechner · (dieser Rechner)                 │
│  Pixel 7        · Handy   · zuletzt aktiv 14:02 [Abmelden …] │
│                                                              │
│                                          [ Handy koppeln ]   │
└──────────────────────────────────────────────────────────────┘
```

### Mobil · Sessions-Liste (M-LI) und Session (M-SE)

```
┌──────────────────────┐   ┌──────────────────────┐
│ account03 @ R2   ⚙   │   │ ‹ Sessions  Gliederung│
│ verbunden            │   │ Abschlussarbeit      │
├──────────────────────┤   ├──────────────────────┤
│ ● Gliederung         │   │ Agent: Ich lese …    │
│   wartet auf dich    │   │ ┌ Rückfrage ───────┐ │
│                      │   │ │ outline.md       │ │
│ ✓ Bildreihe          │   │ │ anlegen?         │ │
│   fertig · 13:40     │   │ │ [Nein] [Ja]      │ │
│                      │   │ └──────────────────┘ │
│                      │   ├──────────────────────┤
│                      │   │ [Kurz nachsteuern…] ⏹│
└──────────────────────┘   └──────────────────────┘
```

I1 ist **read-only**: M-SE zeigt den Verlauf und den Zustand. Die Eingabe, die Rückfrage-Antwort und „Stopp“ kommen in I2.

---

## F1 · Anmelden (Desktop, Einladung)

Ablauf und Texte: KONZEPT A3 §1.

| # | Ansicht | Hauptaktion | Sichtbarer Abschluss | Rückweg |
|---|---|---|---|---|
| 1 | Das Programm startet. D-ST zeigt „Willkommen“, D-SZ „☁ Nur Remote · nicht angemeldet“. | Link einfügen oder Link öffnen (das Betriebssystem übergibt den Link an cocopi) | Das Feld ist gefüllt und „Anmelden“ ist aktiv. | Feld leeren |
| 2 | D-ST | **[Anmelden]** | „Einladung wird geprüft …“ | „Abbrechen“ während der Prüfung. Danach ist nichts gespeichert. |
| 3 | D-ST Erfolg | **[Los geht’s]** | D-SZ: „☁ Nur Remote · Mira · account03 @ R2 · verbunden“. Die D-SL ist leer und zeigt den Hinweis „Noch keine Session. Starte mit ‚Neue Session‘.“ | Einstellungen › Geräte › „Dieser Rechner abmelden“ |
| 3F | D-ST Fehler | je nach Text (A3) | Die Fehlermeldung bleibt stehen, bis neu eingegeben wird. | Feld ändern |

**Nie:** kein lokaler `pi`-Start und keine Abfrage der pi-Version (E4).

## F2 · Erstes Ergebnis

| # | Ansicht | Hauptaktion | Sichtbarer Abschluss | Rückweg |
|---|---|---|---|---|
| 1 | D-SL leer | **[+ Neue Session]** | Ein neuer Eintrag „Neue Session · bereit“, danach Fokus in D-EI | „⋯ › Ruhen lassen“ |
| 2 | D-KZ | Projekt wählen: „Projekt: — ▾“ → Liste der R2-Ordner unter `~/Projekte`, darunter „Neues Projekt …“ | In der Kopfzeile steht „Projekt: Abschlussarbeit“. | Ein anderes Projekt wählen |
| 3 | D-EI | Auftrag tippen und **[Senden]** (Enter) | Die Nachricht erscheint im Verlauf. Der Eintrag zeigt „arbeitet“, der Knopf wird zu **[Stopp]**. | **[Stopp]** bzw. Esc → „gestoppt“ im Verlauf |
| 4 | D-CH Rückfrage | Rückfrage beantworten, z. B. **[Ja, anlegen]** | Die Karte zeigt „✓ Ja, anlegen“ und bleibt im Verlauf, der Zustand wird wieder „arbeitet“. | Die Antwort ist endgültig (pi-Semantik). Alternativ vorher **[Stopp]**. |
| 5 | D-CH | – | Der Eintrag zeigt „✓ fertig“. Im Verlauf steht die Antwort, bei Dateien mit dem Link „In Dateien zeigen“. | – |
| 6 | D-DA | **[Ergebnisse auf den Mac holen …]** → F3 | | |

## F3 · Dateien (T4, Konzept für I2; I1 nur Ort festlegen)

| # | Ansicht | Hauptaktion | Sichtbarer Abschluss | Rückweg |
|---|---|---|---|---|
| 1 | D-KZ | Umschalter **[Dateien]** | D-DA mit zwei Spalten | **[Chat]** |
| 2a | D-DA | **[Nach R2 übertragen …]** → Dateiauswahl des Systems | Vor dem Senden: Liste „Diese 3 Dateien gehen nach R2 (4,2 MB)“, Knöpfe **[Abbrechen]** **[Übertragen]** | **[Abbrechen]**: nichts übertragen |
| 2b | D-DA | **[Änderungen prüfen]** | Liste: neu, geändert, gelöscht auf R2 bzw. auf dem Mac. **Es wird nichts verändert.** | Ansicht schließen |
| 2c | D-DA | **[Ergebnisse auf den Mac holen …]** | Vorschau der Liste, danach „3 Dateien geholt nach ~/cocopi/Abschlussarbeit“ mit Link **[Im Finder zeigen]** | Bei Konflikt bleibt die lokale Datei erhalten; die neue heißt `name (R2).ext` |
| 3 | D-DA | Datei anklicken | Vorschau unten: Markdown über md-egui, Bild oder PDF-Seite als Bild. HTML oder Unbekanntes wird nicht angezeigt; stattdessen der Knopf **[Im Standardprogramm öffnen]**. | – |

## F4 · Handy koppeln

Texte: KONZEPT A3 §2.

| # | Gerät · Ansicht | Hauptaktion | Sichtbarer Abschluss | Rückweg |
|---|---|---|---|---|
| 1 | Desktop · D-SE › Geräte | **[Handy koppeln]** | Dialog mit QR, Code und Countdown „noch 4:59“ | **[Abbrechen]**: Code verfällt sofort |
| 2 | Handy · erster Start | **[Mit Rechner koppeln]** | Kamera mit Rahmen | Zurück-Geste |
| 3 | Handy · M-QR | QR scannen (alternativ „Code eintippen“) | „Warte auf Bestätigung am Rechner …“ und das Prüfwort | **[Abbrechen]** |
| 4 | Desktop · Dialog | Prüfwort vergleichen, dann **[Erlauben]** | Desktop: „Pixel 7“ erscheint in der Liste. Handy: „Gekoppelt mit account03 @ R2.“ | **[Ablehnen]**. Die Abmeldung später erfolgt über D-SE › Geräte. |
| 5 | Handy · M-LI | – | Die Sessions sind sichtbar, mit Zuständen. | M-EI › „Dieses Handy abmelden“ |

## F5 · Hybrid umschalten

Texte: KONZEPT A2.

| # | Ansicht | Hauptaktion | Sichtbarer Abschluss | Rückweg |
|---|---|---|---|---|
| 1 | D-SZ | Klick auf „☁ Nur Remote“ (alternativ Menü › Einstellungen › Modus) | D-SE › Modus mit zwei Auswahlen | Fenster schließen |
| 2 | D-SE › Modus | **„Auch auf diesem Rechner (Hybrid) …“** wählen | Dialog „Auch auf diesem Rechner arbeiten?“ | **[Abbrechen]** (Standard) |
| 3 | Dialog | **[Hybrid einschalten]** | D-SZ: „⇄ Hybrid · …“. Bei „Neue Session“ gibt es jetzt die Wahl „Wo? R2 / Dieser Rechner“. Laufende Sessions sind unverändert. | Schritt 4 |
| 3F | Dialog, pi fehlt lokal | – | „**pi ist auf diesem Rechner nicht installiert.** Hybrid ist eingeschaltet, aber lokal kann noch nichts starten. Anleitung: Hilfe › Hybrid einrichten.“ Es wird **nichts installiert**. | „Nur auf R2“ |
| 4 | D-SE › Modus | **„Nur auf R2 (empfohlen)“** | D-SZ: „☁ Nur Remote“. Eine laufende lokale Session bleibt mit „Dieser Rechner“ markiert, bis sie geschlossen wird. | Schritt 2 |

## F6 · Fehler

Form jedes Fehlers: Was ist passiert, und was kann ich tun. Der Fehler steht an **einem** Ort und bleibt dort, bis er gelesen ist.

| Fall | Ort | Text | Hauptaktion | Abschluss |
|---|---|---|---|---|
| R2 nicht erreichbar (Start oder unterwegs) | D-SZ „offline“ + Meldung oben in D-CH | „**Keine Verbindung zu R2.** Deine Sessions laufen dort weiter. cocopi verbindet sich automatisch neu.“ Zustände grau mit „Stand 14:02“ | **[Jetzt neu verbinden]** | D-SZ „verbunden“, Meldung verschwindet von selbst |
| Dritte Session (T3) | an „Neue Session“ | KONZEPT A1 „Du hast schon 2 offene Sessions …“ | **[‚Bildreihe‘ ruhen lassen]** | Die neue Session ist offen, die andere zeigt „ruht“. Nichts wird gelöscht. |
| Andere Session arbeitet schon (T3, 1 aktiv) | an [Senden] | „**Eine andere Session arbeitet gerade** (‚Gliederung‘). Dein Auftrag wartet und startet danach automatisch.“ | **[Warten]** (Standard) / „Andere stoppen“ | Der Eintrag zeigt „wartet auf Start · Platz 1“. |
| Speichergrenze | D-CH | KONZEPT A4 „Diese Aufgabe brauchte mehr Speicher …“ | **[Neu öffnen]** | Zustand „bereit“ |
| Protokoll passt nicht | D-ST/D-CH | KONZEPT A1 „cocopi muss aktualisiert werden …“ | **[Download-Seite öffnen]** (System-Browser) | – |
| Gerät widerrufen | D-ST | „**Dieses Gerät wurde abgemeldet.** …“ | Einladung einlösen (F1) | F1 |
| Session abgestürzt | D-SL Eintrag „Fehler“ + Zeile in D-CH | „**Die Session wurde unerwartet beendet.** Der Verlauf ist gespeichert.“ | **[Neu öffnen]** | Zustand „bereit“, gleicher Verlauf |
