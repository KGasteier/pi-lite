# cocopi v3 · Nutzerflüsse (S1, Fassung 2 · I2)

**Form:**
- Jeder Schritt hat **eine** Hauptaktion, einen **sichtbaren Abschluss** und einen **Rückweg**.
- Die Orte stammen aus INVENTAR.md, z. B. D-ST oder M-LI.
- Texte in „…“ sind wörtlich gemeint.
- Die Persona ist **Mira** (Kap. 1 Tutorial), Slot `account03`. Ab I2 ist `account03` nur **der Arbeitsplatz, den Mira gerade aus dem Pool hat** (E7); beim nächsten Check-in kann es ein anderer sein. Seminarleitung: **Aylin**.
- Begriffe: `GLOSSAR.md`.
- **Nummern ab Fassung 2:** F1–F4 unverändert. Neu sind F5–F9 (I2). Die I1-Flüsse „Hybrid umschalten“ und „Fehler“ heißen jetzt **F10** (bis I1: F5) und **F11** (bis I1: F6).
- Form der I2-Flüsse F5–F9: je Schritt **was Mira sieht**, **eine Hauptaktion**, **Abbruch** und **Fehler** (was / tun). Codes und Wortlaut: KONZEPT B11.

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

> **Fassung 2:** Im Poolbetrieb ersetzt **F5 Check-in** diesen Fluss. F1 gilt nur noch für Hubs ohne Empfang (I1-Betrieb, Tests).

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

> **Fassung 2:** Das Mitnehmen nach R2 und Zurückholen geschieht ab I2 beim Check-in (F6) und Check-out (F7). F3 bleibt für einzelne Dateien während der Belegung; Konfliktname wie F7: `Name (R2 TT.MM. HH-MM).ext`.

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

## I2 · Ansichten (ASCII)

### Desktop · Check-in (D-ST, Fassung 2)

```
┌──────────────────────────────────────────────────────────────┐
│ cocopi                                       Datei Ansicht Hilfe│
├──────────────────────────────────────────────────────────────┤
│                                                              │
│      Dein Arbeitsplatz auf R2. Melde dich im Browser         │
│      mit deinem UdK-Konto an.                                │
│                                                              │
│            ┌────────────────────────────────┐                │
│            │    Einchecken mit UdK-Konto    │                │
│            └────────────────────────────────┘                │
│              Ich habe einen Einladungscode                   │
│                                                              │
│   Kein lokaler Agent. Dateien gehen nur mit deiner           │
│   Bestätigung nach R2.                                       │
├──────────────────────────────────────────────────────────────┤
│ ☁ Nur Remote · nicht eingecheckt                             │
└──────────────────────────────────────────────────────────────┘
```

### Desktop · Warteschlange und Sync-Auswahl

```
┌ Warteschlange ───────────────────┐  ┌ Was möchtest du nach R2 mitnehmen? ──────┐
│                                  │  │ Aus ~/cocopi · nur deine Auswahl wird     │
│ Alle 10 Arbeitsplätze sind       │  │ übertragen.                               │
│ belegt. Du bist Nr. 2 in der     │  │ ☑ Projekte / Seminar-Handout  4,2 MB      │
│ Warteschlange.                   │  │   · zuletzt benutzt                       │
│                                  │  │ ☐ Projekte / Bildreihe       310 MB       │
│ Du rückst automatisch nach.      │  │ ☐ Skills                     12 KB        │
│ Lass dieses Fenster einfach      │  │ Builds/Cache werden nicht übertragen      │
│ offen.                           │  │ (1,1 GB)                                  │
│                                  │  │ [ Auswahl senden und starten · 4,2 MB ]   │
│               [Abbrechen]        │  │ Ohne Dateien starten · Check-in beenden … │
└──────────────────────────────────┘  └───────────────────────────────────────────┘
```

### Desktop · Check-out-Vorschau

```
┌ Deine Dateien kommen mit ────────────────────────────────────┐
│ Kommt zurück auf deinen Rechner  │ Wird gelöscht              │
│ Ergebnisse/Seminar-Handout.pdf   │ Builds, Cache und          │
│   1,8 MB · neu                   │ temporäre Dateien · 1,1 GB │
│ Projekte/Seminar-Handout/        │  target/ · node_modules/   │
│   gliederung.md 3 KB · geändert  │ Auch alle übrigen Dateien  │
│ Zusammen 1,8 MB                  │ auf R2 werden … entfernt.  │
│──────────────────────────────────┴────────────────────────────│
│ Danach ist account03 leer und für andere frei. Auf R2 bleibt  │
│ nichts.                                                       │
│ [ Auschecken und Dateien holen ]          Zurück zur Arbeit   │
└───────────────────────────────────────────────────────────────┘
```

### Desktop · Seminar und Mitsteuern (Sicht Mira)

```
╔══════════════════════════════════════════════════════════════╗  ← Rahmen #4fb8d8
║ Aylin steuert mit · noch 27 min                   [Beenden]  ║  ← Leiste, immer sichtbar
╠═══════════════╦══════════════════════════════════════════════╣
║ Sessions      ║ Projekt: Seminar-Handout   [Chat|Dateien|Seminar]
║ ● Seminar-    ║ ▌Aylin · Seminarleitung                      ║
║   Handout     ║ ▌Kürze die Einleitung auf drei klare Sätze.  ║
║   arbeitet    ║  Agent: Ich kürze die Einleitung …           ║
╚═══════════════╩══════════════════════════════════════════════╝
```

---

## F5 · Check-in (UdK-Konto oder Gast, Warteschlange)

Grundlage: KONZEPT B2, B3. Fehlertexte: KONZEPT B11.

| # | Was Mira sieht | Hauptaktion | Abbruch | Fehler (was / tun) |
|---|---|---|---|---|
| 1 | D-ST Check-in (Ansicht oben). D-SZ „☁ Nur Remote · nicht eingecheckt“. | **[Einchecken mit UdK-Konto]** | Programm schließen, nichts passiert | `empfang_offline`: „Der Empfang auf R2 ist nicht erreichbar.“ / „Prüfe deine Verbindung. …“ |
| 1G | Gast: Link „Ich habe einen Einladungscode“ öffnet ein Feld „Einladungscode der Seminarleitung“. | Code aus dem QR einfügen, **[Als Gast einchecken]** | Link erneut klicken: Feld schließt sich | `abgelaufen`: „Dieser Code ist abgelaufen.“ / „Bitte um einen neuen Code.“ · `verbraucht`: „Dieser Code wurde schon benutzt.“ / „Erzeuge einen neuen Code. …“ |
| 2 | „Warte auf Anmeldung im Browser …“ · „Melde dich dort mit deinem UdK-Konto an. Dieses Fenster wartet auf die geprüfte Bestätigung vom Empfang.“ Der System-Browser zeigt die authentik-Seite. | Im Browser anmelden (cocopi wartet, kein Klick in cocopi) | **[Abbrechen]** → „Check-in abgebrochen.“, zurück zu 1 | `browser_fehlt`: „Der Browser konnte nicht geöffnet werden.“ / „Öffne einen Browser …“ · `anmeldung_abgebrochen` · `keine_berechtigung`: „Du darfst diesen Bereich nicht öffnen.“ / „Frage deine Seminarleitung.“ · `zu_viele` |
| 2E | Nur beim allerersten Check-in: Fingerabdruck des Empfangs „Empfang R2 · Fingerabdruck AB12 CD34 …“ (Vergleich mit Aushang/Seminarleitung). | **[Empfang vertrauen]** | **[Abbrechen]** → nichts gespeichert | – (abweichender Schlüssel später: `ausweis_ungueltig`) |
| 3 | Frei: direkt Schritt 4. Voll: Karte „Warteschlange“: „Alle 10 Arbeitsplätze sind belegt. Du bist Nr. 2 in der Warteschlange.“ · „Du rückst automatisch nach. Lass dieses Fenster einfach offen.“ Die Nummer zählt von selbst herunter. | keine (Warten, rückt ohne Klick nach) | **[Abbrechen]** → Platz in der Warteschlange wird aufgegeben, „Check-in abgebrochen.“ | `warteschlange_verfallen`: „Dein Platz in der Warteschlange ist verfallen.“ / „Checke erneut ein. Lass das Fenster beim Warten offen.“ |
| 3a | „Ein Arbeitsplatz wird für dich frei“ (kurz), dann Schritt 4. | – | – | – |
| 4 | Abholfach vorhanden? → F7 Schritt 7. Sonst weiter zu F6. D-SZ: „☁ Nur Remote · Mira · account03 @ R2 · eingecheckt“. | weiter zu F6 | „Check-in beenden …“ (ab hier ist der Arbeitsplatz belegt: führt in die Check-out-Vorschau F7, nie stilles Freigeben) | – |

**Nie:** Rolle aus dem Client; Passwort in cocopi; ein Arbeitsplatz wird jemandem weggenommen.

## F6 · Sync-Auswahl (beim Check-in)

Grundlage: KONZEPT B4, B6.

| # | Was Mira sieht | Hauptaktion | Abbruch | Fehler (was / tun) |
|---|---|---|---|---|
| 1 | „Dateien in ~/cocopi werden geprüft …“ · „Es wird noch nichts übertragen.“ | – (läuft) | **[Abbrechen]** → zurück zur Auswahl (leer) bzw. Startkarte | `ordner_fehlt` (Client): „Der Ordner ~/cocopi fehlt.“ / „cocopi legt ihn jetzt leer an. Lege deine Projekte unter Projekte/ ab.“ |
| 2 | „Was möchtest du nach R2 mitnehmen?“ Häkchenliste mit Größen. Vorausgewählt nur „Projekte / Seminar-Handout · zuletzt benutzt“. Grau: „Builds/Cache werden nicht übertragen (1,1 GB)“. Große Dateien (> 200 MB) einzeln mit eigenem Häkchen, nie vorausgewählt. | Häkchen setzen/entfernen, dann **[Auswahl senden und starten · 4,2 MB]** | „Ohne Dateien starten“ (nichts wird übertragen) · „Check-in beenden …“ (→ F7) | `zu_gross`: „Deine Auswahl ist größer als 2 GB.“ / „Nimm weniger Projekte mit. …“ |
| 3 | „Deine Auswahl geht nach R2 …“ mit Fortschritt in MB. | – (läuft) | **[Abbrechen]** → „Abbruch wird bestätigt …“, dann zurück zu Schritt 2. Auf R2 entsteht nichts Halbes. | `pfad_nicht_erlaubt`: „Dieser Ordner ist nicht freigegeben.“ / „Wähle einen Ordner in ~/cocopi.“ · `empfang_offline` |
| 4 | Arbeitsansicht (F2). In D-DA liegen die gewählten Projekte unter `Projekte/`. | weiter mit F2 | – | – |

**Nie:** Upload ohne Klick auf Schritt 2; Symlinks oder Pfade außerhalb `~/cocopi`; Hintergrundabgleich.

## F7 · Check-out (Vorschau, Konflikte, Abholfach)

Grundlage: KONZEPT B5, B6.

| # | Was Mira sieht | Hauptaktion | Abbruch | Fehler (was / tun) |
|---|---|---|---|---|
| 1 | Arbeitsansicht, oben rechts „Auschecken …“ (einziger Ort). | **[Auschecken …]** | – | – |
| 2 | „Check-out wird geprüft …“ · „Noch wird nichts gelöscht oder übertragen.“ | – (läuft) | **[Abbrechen]** → zurück zur Arbeit | `checkout_laeuft`: „Dieser Arbeitsplatz wird gerade zurückgegeben.“ / „Versuche es erneut …“ |
| 3 | Vorschau „Deine Dateien kommen mit“ (Ansicht oben): links „Kommt zurück auf deinen Rechner“ mit Größe und Status neu/geändert, rechts „Wird gelöscht“ mit Beispielen. „Danach ist account03 leer und für andere frei. Auf R2 bleibt nichts.“ · „Hast du eine Datei lokal geändert, liegt die R2-Fassung daneben. Nichts wird überschrieben.“ Große Dateien einzeln mit Häkchen. | **[Auschecken und Dateien holen]** | „Zurück zur Arbeit“ → nichts geändert | `kein_platz_lokal`: „Auf deinem Rechner ist nicht genug Platz.“ / „Mach {Größe} frei …“ |
| 4 | „Dateien kommen zurück …“ mit Fortschritt. „Erst nach geprüfter Rückkehr der Dateien und bestätigter Leerung wird der Arbeitsplatz freigegeben.“ | – (läuft) | **[Abbrechen]** → „Abgebrochen. Der Arbeitsplatz bleibt belegt; deine Dateien bleiben auf R2, bis du auscheckst.“ | `pruefsumme_falsch`: „Die Dateien sind nicht vollständig angekommen. Es wurde nichts gelöscht.“ / „Klicke ‚Erneut prüfen‘. …“ · `empfang_offline` |
| 5 | Konflikt (lokal geändert): Zeile „gliederung.md · deine Fassung bleibt · R2-Fassung als ‚gliederung (R2 28.10. 17-40).md‘ daneben“. | – (Hinweis) | – | – |
| 6 | Abschluss: „Ausgecheckt. 2 Dateien in ~/cocopi · 1 Konflikt erhalten. account03 ist leer.“ Startkarte F5 Schritt 1. | **[Ordner öffnen]** | – | `leeren_unbestaetigt`: „Der Arbeitsplatz ist noch nicht nachweislich leer.“ / „Die Seminarleitung muss den Arbeitsplatz prüfen.“ (Deine Dateien sind trotzdem schon lokal.) |
| 7 | **Abholfach** (nach erzwungenem Check-out, beim nächsten Check-in vor F6): „Dateien vom letzten Mal abholen“ · „Im verschlüsselten Abholfach liegen 1,8 MB. Verfügbar bis 31.10. 23:30.“ · „Sie werden in ~/cocopi zurückgespielt. Lokale Änderungen bleiben erhalten.“ | **[Dateien vom letzten Mal abholen]** → „Dateien vom letzten Mal abgeholt · 2 Dateien · 0 Konflikte erhalten.“ → F6 | „Später abholen · weiter zur Auswahl“ (bleibt bis Ablauf) · „Check-in beenden …“ | `abholfach_fremd` (Client, falscher Geräteschlüssel): „Dieses Abholfach kann nur der Rechner öffnen, mit dem du eingecheckt warst.“ / „Checke an diesem Rechner ein. Nach 72 h wird es gelöscht.“ |

**Nie:** löschen vor bestätigtem Erhalt; lokale Datei überschreiben; „frei“ melden, bevor die Leere geprüft ist.

## F8 · Seminar-Chat und Direktnachricht

Grundlage: KONZEPT B7.

| # | Was Mira sieht | Hauptaktion | Abbruch | Fehler (was / tun) |
|---|---|---|---|---|
| 1 | Reiter „Seminar“ (neben Chat/Dateien). Links „Teilnehmende“ mit Abzeichen (Seminarleitung, Teilnehmende, Gast). Rechts „Seminarchat“ und die Zeile „Direkt miteinander · nur mit Zustimmung mitsteuern“. | Reiter **[Seminar]** | Reiter **[Chat]** | `ausweis_ungueltig`: „Dein Geräteausweis ist ungültig.“ / „Melde dich erneut am Empfang an.“ |
| 2 | Seminarchat mit Verlauf, z. B. „Aylin · Seminarleitung: Willkommen! Heute gestalten wir ein zweiseitiges Seminar-Handout.“ Feld „Nachricht schreiben …“. | Text, **[Nachricht senden]** (Enter) | Feld leeren / Esc | `nicht_gesendet`: „Nachricht nicht gesendet.“ / „Die Verbindung zu {Person} fehlt. …“ (Text bleibt im Feld) |
| 3 | Person in „Teilnehmende“ anklicken → „Nachricht · Aylin“ (Direktnachricht). | Text, **[Nachricht senden]** | Zurück zum Seminarchat | wie 2 |

## F9 · Mitsteuern (Anfrage, Zustimmung, aktiv, Ende)

Grundlage: KONZEPT B7. Zwei Sichten: Aylin (A) und Mira (M).

| # | Was man sieht | Hauptaktion | Abbruch | Fehler (was / tun) |
|---|---|---|---|---|
| 1 | A: Teilnehmende › Mira › Session „Seminar-Handout“ ▾. Knöpfe „Ansehen“ und „Mitsteuern anfragen“ (nur für Rolle Seminarleitung). | **[Mitsteuern anfragen]** (oder „Ansehen“, ≤ 120 min) | – | `keine_berechtigung` bei anderer Rolle (Knopf fehlt ohnehin) |
| 2 | A: „Anfrage gesendet · wartet auf Zustimmung“. | warten | **[Anfrage zurückziehen]** | – |
| 3 | M: Dialog „Mitschreiben erlauben?“ · „Aylin (Seminarleitung) möchte in ‚Seminar-Handout‘ mitschreiben.“ | **[Erlauben 30 min]** · [Nur ansehen] · **[Ablehnen]** (Fokus) | Esc = Ablehnen | „Diese Session ist nicht mehr verfügbar. Lehne die Anfrage ab.“ |
| 4 | M: blauer Rahmen `#4fb8d8` + Leiste „Aylin steuert mit · noch 27 min · Beenden“. Prompts von Aylin im Chat als „Aylin · Seminarleitung“ mit Farbmarke. Rückfragen beantwortet Mira. A: „Du steuerst Miras Session ‚Seminar-Handout‘ · Zurückgeben“, Feld „Auftrag für die freigegebene Session“. | A: Auftrag, **[Als Seminarleitung senden]** | M: **[Beenden]** · A: **[Zurückgeben]** | A: „Auftrag nicht an R2 gesendet: …“ / „Versuche es erneut. Mira sieht nichts davon.“ |
| 5 | Ende (beide): Rahmen und Leiste verschwinden, Zeile im Chat „Freigabe beendet“. Grund: Beenden, Zurückgeben, Ablauf, Check-out, Ausweis abgelaufen, Verbindung > 60 s weg. | – | – | `freigabe_beendet`: „Freigabe beendet.“ / „Mira steuert ihre Session wieder allein.“ |

**Nie:** Eingabe ohne Zustimmung; Mitsteuern ohne sichtbaren Rahmen und Leiste; Rolle aus dem Client.

## F10 · Hybrid umschalten (bis I1: F5)

Texte: KONZEPT A2.

| # | Ansicht | Hauptaktion | Sichtbarer Abschluss | Rückweg |
|---|---|---|---|---|
| 1 | D-SZ | Klick auf „☁ Nur Remote“ (alternativ Menü › Einstellungen › Modus) | D-SE › Modus mit zwei Auswahlen | Fenster schließen |
| 2 | D-SE › Modus | **„Auch auf diesem Rechner (Hybrid) …“** wählen | Dialog „Auch auf diesem Rechner arbeiten?“ | **[Abbrechen]** (Standard) |
| 3 | Dialog | **[Hybrid einschalten]** | D-SZ: „⇄ Hybrid · …“. Bei „Neue Session“ gibt es jetzt die Wahl „Wo? R2 / Dieser Rechner“. Laufende Sessions sind unverändert. | Schritt 4 |
| 3F | Dialog, pi fehlt lokal | – | „**pi ist auf diesem Rechner nicht installiert.** Hybrid ist eingeschaltet, aber lokal kann noch nichts starten. Anleitung: Hilfe › Hybrid einrichten.“ Es wird **nichts installiert**. | „Nur auf R2“ |
| 4 | D-SE › Modus | **„Nur auf R2 (empfohlen)“** | D-SZ: „☁ Nur Remote“. Eine laufende lokale Session bleibt mit „Dieser Rechner“ markiert, bis sie geschlossen wird. | Schritt 2 |

## F11 · Fehler (bis I1: F6)

Form jedes Fehlers: Was ist passiert, und was kann ich tun. Der Fehler steht an **einem** Ort und bleibt dort, bis er gelesen ist.

| Fall | Ort | Text | Hauptaktion | Abschluss |
|---|---|---|---|---|
| R2 nicht erreichbar (Start oder unterwegs) | D-SZ „offline“ + Meldung oben in D-CH | „**Keine Verbindung zu R2.** Deine Sessions laufen dort weiter. cocopi verbindet sich automatisch neu.“ Zustände grau mit „Stand 14:02“ | **[Jetzt neu verbinden]** | D-SZ „verbunden“, Meldung verschwindet von selbst |
| Dritte Session (T3) | an „Neue Session“ | KONZEPT A1 „Du hast schon 2 offene Sessions …“ | **[‚Bildreihe‘ ruhen lassen]** | Die neue Session ist offen, die andere zeigt „ruht“. Nichts wird gelöscht. |
| Andere Session arbeitet schon (T3, 1 aktiv) | an [Senden] | „**Eine andere Session arbeitet gerade** (‚Gliederung‘). Dein Auftrag wartet und startet danach automatisch.“ | **[Warten]** (Standard) / „Andere stoppen“ | Der Eintrag zeigt „wartet auf Start · Platz 1“. |
| Speichergrenze | D-CH | KONZEPT A4 (Text gilt weiter) „Diese Aufgabe brauchte mehr Speicher …“ | **[Neu öffnen]** | Zustand „bereit“ |
| Protokoll passt nicht | D-ST/D-CH | KONZEPT A1 „cocopi muss aktualisiert werden …“ | **[Download-Seite öffnen]** (System-Browser) | – |
| Gerät widerrufen | D-ST | „**Dieses Gerät wurde abgemeldet.** …“ | Einladung einlösen (F1); ab I2: erneut einchecken (F5) | F1 / F5 |
| Session abgestürzt | D-SL Eintrag „Fehler“ + Zeile in D-CH | „**Die Session wurde unerwartet beendet.** Der Verlauf ist gespeichert.“ | **[Neu öffnen]** | Zustand „bereit“, gleicher Verlauf |
