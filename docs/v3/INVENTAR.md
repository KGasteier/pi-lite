# cocopi v3 · Funktionsinventar (A5, Entwurf S1)

Stand: I1, Basis `v3/s1` @ `eb534bb`. Grundlage: `copi/app/src/{view,net_view,coco_view,material_ui,app}.rs`, `ui/index.html`, `ui/app.js`, Tutorial Kap. 3–6 und 14, Prompt §3 und §4.2.
Status: **Entwurf zur Abstimmung mit S2 (Desktop) und S5 (Mobil)**. Wird nach Zustimmung eingefroren (§5.1).

## 0. Regeln für dieses Inventar

- Jede Funktion bekommt **genau einen v3-Ort**. Tastenkürzel und das Plattformmenü sind nur Abkürzungen **zu diesem Ort**, also kein zweiter Ort.
- Spalte „Plattform“: **D** = nur Desktop, **M** = nur Mobil, **D+M** = beide, **weg** = entfällt in v3 (I1–I3).
- Bei **Dublette** steht in der Tabelle, welcher Weg bleibt. Der andere Weg wird gestrichen.
- Ein Ort heißt nach dem, was die Nutzerin sieht, nicht nach dem Code.

### v3-Orte Desktop (feste Liste)

| Kürzel | Ort | Inhalt in einem Satz |
|---|---|---|
| **D-ST** | Startkarte | Erster Start oder nicht angemeldet: Modus, Einladung einlösen, Verbindung. |
| **D-SL** | Seitenleiste „Sessions“ (links) | Liste der eigenen Sessions mit Zustand, oben „Neue Session“. |
| **D-KZ** | Kopfzeile | Workspace (R2-Projektordner) und der Umschalter **Chat · Dateien**. |
| **D-CH** | Chat (Hauptbereich) | Verlauf, Rückfrage-Karten, Werkzeugblöcke. |
| **D-EI** | Eingabe (unten) | Textfeld, **Senden** bzw. **Stopp** (ein Knopf), Modell. |
| **D-DA** | Dateien (Hauptbereich, über Umschalter) | „Nach R2 übertragen“, „Änderungen prüfen“, „Ergebnisse holen“, Vorschau. |
| **D-SZ** | Statuszeile (unten, fest) | Modus · Konto @ R2 · Verbindung · Verbrauch. Nur Anzeige, ein Klick öffnet die Erklärung. |
| **D-SE** | Einstellungen (eigenes Fenster, macOS ⌘, / sonst Menü „Bearbeiten › Einstellungen“) | Konto, Geräte, Modus, Darstellung, Vorlesen, Erweitert. |
| **D-ME** | Plattformmenü (Menüleiste) | Datei, Bearbeiten, Ansicht, Hilfe: nur Abkürzungen auf die Orte oben, dazu „Hilfe“ und „Tastaturkürzel“. |

### v3-Orte Mobil (feste Liste)

| Kürzel | Ort | Inhalt |
|---|---|---|
| **M-LI** | Sessions-Liste (Startansicht) | Sessions mit Zustand. Oben Konto und Verbindung. |
| **M-SE** | Session-Ansicht | Verlauf, Rückfrage-Karte, kurze Eingabe, **Senden** bzw. **Stopp**. |
| **M-VO** | Ergebnis-Vorschau (Vollbild) | PDF, Bild oder Text ansehen. |
| **M-EI** | Einstellungen | Dieses Gerät, Benachrichtigungen, Abmelden. |
| **M-QR** | QR-Scanner | Koppeln und Bestätigen. Wird aus M-EI oder aus der Einladung geöffnet. |

---

## 1. Desktop heute (copi/cocopi-v2, egui)

### 1.1 Sessions und Seitenleiste

| # | Funktion heute | Heutiger Ort | v3-Ort | Begründung | Plattf. |
|---|---|---|---|---|---|
| 1 | Sessionliste öffnen | linke Seitenleiste (`view.rs` `sidebar`) | **D-SL** | Kern, bleibt | D+M (M-LI) |
| 2 | Neue Session „+ Neu“ (Alt+N) | Seitenleiste | **D-SL** oben | Kern. Bei voller Grenze gibt es eine Erklärung (T3). | D |
| 3 | Session umbenennen (R/F2) | Kontextmenü der Karte | **D-SL** Menü „⋯“ am Eintrag (sichtbar) | §3.4: kein reines Rechtsklick-Menü, der Knopf „⋯“ ist sichtbar | D |
| 4 | Session löschen (⌫) | Kontextmenü | **D-SL** „⋯ › Löschen …“ mit Bestätigung | Löschen ist mobil ausdrücklich nicht vorgesehen (§4.2) | D |
| 5 | Ansicht-Picker `SIDE_VIEWS`: Extensions/Plugins, Skills, pi·Agent- und Kontextdateien | Seitenleisten-Kopf | **D-SE › Erweitert** (nur im Modus Hybrid) | Zeigt lokale `~/.pi`-Dateien, die bei Remote-only nicht existieren. Für Lernende kein Alltagsbedarf. | D |
| 6 | Inventarkarten: Vorschau, Bearbeiten, Intern bearbeiten, VSCodium ↗ | Seitenleiste (Inventar) | **D-SE › Erweitert** (nur Hybrid) | wie #5. Vier Aktionen pro Karte sind zu dicht → „Ansehen“ und „In Editor öffnen“ | D |
| 7 | Herkunftsfilter „Alle Orte / nur lokal“ | Seitenleiste | **weg** | Bei Remote-only gibt es nur R2. Bei Hybrid zeigt jeder Eintrag seine Herkunft als Text „R2“ bzw. „Dieser Rechner“, ein Filter ist nicht nötig. | weg |
| 8 | Bookmark-Filter, 3-stufiger Zyklus (Alle → Zeit → Farbkategorie) | Seitenleiste | **weg**; Ersatz: „⋯ › Anheften“ in **D-SL**, angeheftete Sessions stehen oben | Bei höchstens 2 offenen Sessions ist der Zyklus überdimensioniert und versteckt (§3.4). | weg |
| 9 | Bookmark-Farbpalette (10 Farben) | Kontextmenü | **weg** | wie #8. Farbe konkurriert mit der Signalfarbe für den Zustand. | weg |
| 10 | Sortierung (`PickerKind::Sort`) | Seitenleiste | **weg**: fest „zuletzt benutzt“, Angeheftete oben | eine Ordnung genügt | weg |
| 11 | Vorschau-Panel „Kurzinfo“ für Dateien (Alt+5, „×“) | rechts neben Seitenleiste | **D-DA** Vorschau | Dateivorschau gehört zum Dateifluss (T4), nicht zur Sessionliste | D (M: M-VO) |
| 12 | „Zweck?“ / „Von KI erklären lassen…“ / „Zustimmen & erklären“ | Vorschau-Panel | **weg** | startet lokales `pi` (`preview.rs` `ask_haiku`) und verstößt damit gegen E4 | weg |
| 13 | „Hilfsmodell…“ (KI-Hilfsmodell wählen) | Vorschau-Panel | **weg** | gehört zu #12 | weg |
| 14 | „Cache leeren“ (Kurzinfos) | Vorschau-Panel | **weg** | gehört zu #12; ohne #12 gibt es keinen Cache für Lernende | weg |

### 1.2 Kopfzeile

| # | Funktion heute | Heutiger Ort | v3-Ort | Begründung | Plattf. |
|---|---|---|---|---|---|
| 15 | „/ Befehle“-Menü | Kopfzeile | **weg** – **Dublette** zu #31 (Slash-Popup in der Eingabe) | ein Ort für Befehle: „/“ in der Eingabe | weg |
| 16 | Workspace-Picker (Arbeitsordner) + Ordnerdialog | Kopfzeile + `folderdialog` | **D-KZ** Workspace, Auswahl aus R2-Ordnern (Hub `/api/dirs`) | Tutorial Kap. 3, Schritt 4 | D (M: nur Anzeige in M-SE) |
| 17 | „Workspace in VSCodium öffnen“ | Kopfzeile | **weg** | lokales Programm, bei Remote-only ohne Sinn | weg |
| 18 | Modell-Picker (Alt+M) | Kopfzeile | **D-EI** „Modell“-Knopf links neben Senden | Das Modell gilt für den nächsten Auftrag, deshalb steht es bei der Eingabe. Die Kopfzeile wird leerer (§3.5). | D |
| 19 | Thinking-Picker (Alt+T) | Kopfzeile | **D-EI** im Modell-Dialog als Wahl „Gründlichkeit: normal / gründlich“ | **Dublette** zu #18 als zweiter Knopf; ein Dialog genügt. Fachbegriff „Thinking“ wird ersetzt. | D |
| 20 | „⑂ Fork“ (Alt+F) | Kopfzeile | **D-SL** „⋯ › Variante ab früherer Nachricht …“ | selten gebraucht, deshalb an einen benannten Ort; zusammengelegt mit Orca „Worktree je Aufgabe“ (siehe ORCA-SYNTHESE) | D |
| 21 | Schriftgröße A− / „15 px“ / A+ (Alt+±) | Kopfzeile | **D-SE › Darstellung**; Kürzel ⌘/Strg + / − / 0 | Die Knöpfe sind eine **Dublette** zu den Kürzeln. Plattformkonvention (§3.2). | D |
| 22 | „?“ Tastaturhilfe (F1) | Kopfzeile + `keyhelp` | **D-ME › Hilfe › Tastaturkürzel**; F1 bleibt als Abkürzung | Platz in der Kopfzeile | D |
| 23 | Backends-/Netz-Umschalter (Strg+Shift+B) | Kopfzeile | **weg**; siehe #36–#44 | Die Backends-Ansicht wird aufgelöst | weg |
| 24 | Seitenleiste ein/aus (Strg+B, Alt+2) | Kopfzeile-Icon | **D-ME › Ansicht › Seitenleiste**; Kürzel ⌘/Strg+B | Das Icon bleibt nur, wenn S2 es plattformüblich findet (macOS: ja). | D |

### 1.3 Chat und Hauptbereich

| # | Funktion heute | Heutiger Ort | v3-Ort | Begründung | Plattf. |
|---|---|---|---|---|---|
| 25 | Chatverlauf (Markdown, Code, Tool-Blöcke) | Hauptbereich (`md-egui`) | **D-CH** | Kern | D+M (M-SE) |
| 26 | Startkarte: pi-Version, Kürzel, Update-Hinweis „pi update“, Kontextdateigruppen | erster Chat-Eintrag | **D-ST**: Inhalt neu (Modus, Einladung, Verbindung). Version und Update-Hinweis **weg** | Bei Remote-only läuft pi auf R2, nicht auf dem Rechner der Lernenden. „pi update“ würde ein lokales pi starten. | D |
| 27 | Rückfrage-Dialoge (Select/Confirm/Input/Editor aus Extensions) | modal (`dialogs`) | **D-CH** Rückfrage-Karte im Verlauf | Eine Rückfrage steht dort, wo die Antwort hingehört, und wartet sichtbar. Am Handy gibt es dieselbe Karte. | D+M |
| 28 | Nachricht „Kopieren“ (Markdown) / „Anfrage in die Eingabe übernehmen“ | Knöpfe an der Nachricht | **D-CH** an der Nachricht: nur „Kopieren“ | „Übernehmen“ ist eine **Dublette** zu Kopieren + Einfügen. | D |
| 29 | Ausgabegröße Alt+± | Kürzel | siehe #21 | **Dublette** | – |
| 30 | Esc / Alt+A Abbrechen | Kürzel + Knopf „Abbrechen“ | **D-EI** Knopf „Stopp“, Abkürzung Esc | Alt+A ist eine **Dublette** zu Esc und wird gestrichen. | D+M |

### 1.4 Eingabe

| # | Funktion heute | Heutiger Ort | v3-Ort | Begründung | Plattf. |
|---|---|---|---|---|---|
| 31 | Textfeld, Enter = senden, Alt+Enter = Zeile | Fußzeile | **D-EI** | Kern | D+M |
| 32 | Senden / Abbrechen (zwei Knöpfe) | Fußzeile | **D-EI** **ein** Knopf: „Senden“, solange gearbeitet wird „Stopp“; steer statt Neu-Senden | eine Hauptaktion je Zustand (§3.5) | D+M |
| 33 | Slash-Popup „/“ | Eingabe | **D-EI** | einziger Ort für Befehle (siehe #15) | D |
| 34 | `!cmd` / `!!cmd` Shell, „pi update“ u. a. pi-CLI-Befehle | Eingabe-Parser (`commands.rs`) | `!cmd` bleibt in **D-EI**, läuft auf R2. pi-CLI-Befehle **weg** | Ein pi-CLI-Befehl startet `pi` lokal (`cli.rs`) und verstößt damit gegen E4. Das Update von pi ist Aufgabe der Administration. | D |
| 35 | Verlauf älter/neuer: Pfeilknöpfe + Alt+↑/↓ | Fußzeile | **D-EI** ↑/↓ im leeren Feld (Plattformkonvention) | Die Knöpfe sind eine **Dublette** zu den Tasten. | D |
| 36 | „Im großen Fenster bearbeiten“ (Prompt-Editor) | Fußzeile | **weg**: das Textfeld wächst bis 50 % Höhe | zweiter Editor für dasselbe Feld = **Dublette** | weg |
| 37 | „Material hinzufügen“ (+): Dateien/URLs, Chips, „Nur Dateiverweise“, Medienroute, Materialmodell | Fußzeile + Materialleiste (`material_ui.rs`) | **D-DA** „Nach R2 übertragen“; Medienroute und Materialmodell **weg** | Tutorial Kap. 4.2: Material kommt über „Nach R2 übertragen“. Zwei Wege für Dateien wären eine **Dublette**. Route und Modell sind Expertenwahl. | D |
| 38 | Extension-Widgets ober-/unterhalb, Extension-Statuszeile | um die Eingabe | **D-EI** ein Bereich direkt über dem Textfeld | Eine Stelle für alle Extension-Ausgaben. Erscheint nur, wenn eine Extension etwas meldet. | D |

### 1.5 Status und Anzeigen

| # | Funktion heute | Heutiger Ort | v3-Ort | Begründung | Plattf. |
|---|---|---|---|---|---|
| 39 | Laufaktivität: Phase, Runden, Tools, Laufzeit, Prozent-Schätzung | Statuszeile über der Eingabe (`run_activity.rs`) | **D-SL** Zustand am Session-Eintrag + **D-CH** eine Zeile „arbeitet · 2:14“ am Verlaufsende; Prozent **weg** | Der Zustand hat eine Quelle (A1). Prozentangaben ohne Serverfortschritt verletzen §3.7. | D+M |
| 40 | Tokenleiste ↑ ↓ R CH, ⓘ Erklärung, „Tokens & Kontext“-Dialog | `usage_bar` | **D-SZ** „Verbrauch“ (T6), Klick → Erklärung | eine Anzeige an einem Ort (Orca-Synthese, T6) | D |
| 41 | Tokenanzeige „Von KI erklären lassen“ | Usage-Popup | **weg** | startet lokales pi (`explain_usage`), verstößt gegen E4 | weg |
| 42 | Vorlesen an/aus/stopp (Piper lokal) | `usage_bar` rechts | **D-SE › Vorlesen** (an/aus); „Stopp“ erscheint nur während des Vorlesens in **D-EI** | Seltene Einstellung. Der Stopp-Knopf muss in Reichweite sein, solange gesprochen wird. | D |
| 43 | Toasts (Meldungen) | rechts unten | **D-CH** Meldungsbereich oben im Hauptbereich, bleibt bis gelesen | §3.7: Fehler dürfen nicht verschwinden, bevor sie gelesen sind | D+M |
| 44 | Demo-Modus-Hinweis, „Backends werden geladen …“, „Verbindung getrennt – Wiederaufbau läuft …“ | Backends-Panel | **D-SZ** Verbindung | Der Verbindungszustand steht an einer festen Stelle (§3.8). | D+M (M-LI Kopf) |

### 1.6 Backends, Instanzen, Raster (`net_view.rs`)

| # | Funktion heute | Heutiger Ort | v3-Ort | Begründung | Plattf. |
|---|---|---|---|---|---|
| 45 | Backends-Liste mit Häkchen verbinden/trennen | Backends-Leiste | **weg**: verbunden wird automatisch mit dem eigenen R2-Konto; Trennen über **D-SE › Konto › Abmelden** | Lernende haben genau ein Backend (E1/A4). | weg |
| 46 | „+ Backend hinzufügen“ (SSH/Hub-URL), „✎ Backend bearbeiten“, „Entfernen“ | Backends-Leiste + Overlay | **D-SE › Erweitert › Verbindung** (Admin/Test); Normalweg ist die Einladung (A3) | Eine Einladung ersetzt die manuelle URL. SSH-Bootstrap kann den Hub neu starten (Tutorial Kap. 2) und entfällt deshalb für Lernende. | D |
| 47 | „+ Instanz“ je Backend | Backends-Leiste | **weg** – **Dublette** zu #2 „Neue Session“ | Instanz und Session sind für Lernende dasselbe. | weg |
| 48 | Ansicht „Haupt-Session / Einzelchat“ | Backends-Leiste | **weg** – **Dublette** zu #1 (Klick in der Sessionliste) | Der Hauptbereich zeigt immer die gewählte Session. | weg |
| 49 | Raster „▦ Raster n“, Kacheln, „Zum Einzelchat“, Kachel-Eingabe | Backends-Leiste + `net_grid` | **weg** | T3: höchstens 2 offen, 1 arbeitet. Ein Raster ist für Lernende eine zweite Chat-Ansicht und damit eine **Dublette**. | weg |
| 50 | Instanzmenü: „Stoppen (Instanz entfernen)“, „Neu starten“ | Rechtsklick/Karte | **D-SL** „⋯ › Ruhen lassen“; Neustart **weg** (Aufwachen beim Öffnen, Hibernation) | Hibernation statt manuellem Neustart (Orca-Synthese) | D+M (M: nur über Zustand) |
| 51 | „Arbeitsordner hierher holen (rsync)“ | Instanzmenü | **D-DA** „Ergebnisse auf den Mac holen“ | **Dublette** zu T4; der rsync-Weg holt alles ohne Prüfung (Tutorial Kap. 5). | D |
| 52 | Kachel-Rückfragen Ja/Nein/OK/Abbrechen | Kachel | siehe #27 | **Dublette** | – |

### 1.7 cocopi-Zusammenarbeit (`coco_view.rs`, P2P-Experiment)

| # | Funktion heute | Heutiger Ort | v3-Ort | Begründung | Plattf. |
|---|---|---|---|---|---|
| 53 | „Netz starten/stoppen“, Identität „Ich · …“ | rechte Leiste | **weg** (im Seminar-Build aus, Feature-Flag) | UI sagt selbst „Experiment – nicht für vertrauliche Inhalte“. Nicht im Umfang §4.1. | weg |
| 54 | Chats, Einladungen annehmen/ablehnen, Mitglieder, Verlassen, Lokal löschen | Reiter „Chats“ | **weg** (Flag) | wie #53 | weg |
| 55 | Kontakte: Kontaktadresse, „Prüfen …“ (Prüfwörter), Blockieren, Entfernen | Reiter „Kontakte“ | **weg** (Flag). Das **Prüfwort-Muster** wird für A3 übernommen | Idee wiederverwendet, Funktion entfällt | weg |
| 56 | Räume, Seminar-Schritt, Fernzugriff anfragen/erlauben/beenden | Reiter „Räume“ | **weg** (Flag) | Lehrenden-Einsicht ist eigenes Thema (Persona Aylin → offene Frage) | weg |
| 57 | Nebenagenten/Personas, Agentengespräch (A2A), Not-Aus | Reiter „Agenten“ | **weg** (Flag) | nicht im Umfang §4.1 | weg |

### 1.8 Ohne Bedienoberfläche

| # | Funktion | Ort heute | v3 | Begründung |
|---|---|---|---|---|
| 58 | `--shot`, `--size`, `COPI_SCENE=…` | CLI/Env | bleibt (Entwicklung, Screenshots für S1/S6) | kein Nutzerort |
| 59 | `--sync-states` (AgentLife-Spiegel) | CLI | später **D-SE › AgentLife** (T7, Opt-in) | P1, nicht I1 |
| 60 | Lokales pi starten (`Pi::start`, `LocalInstances`, CLI-Befehle, Hilfsmodell) | `pi-backend` über `winshim::pi_command` | nur bei Modus Hybrid, Sperre an **einer** Stelle (KONZEPT A2) | E4 |

---

## 2. pi-lite-UI heute (Mobilbasis, `ui/`)

Die pi-lite-UI ist funktionsgleich zu §1 (gleiche Logik, Webview). Hier steht nur, **was mobil bleibt**. Alles andere gilt mobil als **weg**.

| # | Funktion (`ui/index.html` id) | v3-Mobilort | Begründung |
|---|---|---|---|
| m1 | `#sessions` Sessionliste | **M-LI** | Kern §4.2 |
| m2 | `#newsess` Neue Session | **weg (mobil)** | §4.2: kurzer Auftrag in **bestehender** Session. Neue Sessions zählen gegen T3. → offene Frage F3 |
| m3 | `#sideview`, `#inventory` (Extensions/Skills/Kontextdateien) | weg | lokal, Expertenfunktion |
| m4 | `#originfilter`, `#bookmarkfilter` | weg | wie #7–#10 |
| m5 | `#commandmenu`, `#slashpopup` | weg | Befehle sind am Handy nicht sinnvoll |
| m6 | `#cwdpicker`, `#folderdialog`, `#workspaceeditor` | weg; Workspace nur als **Text** in M-SE | Projekte anlegen/wählen nicht mobil |
| m7 | `#modelpicker`, `#thinkpicker` | weg | Modellwahl am Desktop |
| m8 | `#fork` | weg | Desktop |
| m9 | `#textcontrols` | weg; Systemschriftgröße (Dynamic Type/Android-Schriftgröße) | Plattformkonvention |
| m10 | `#gridtoggle`, `#netview`, `#netgrid` | weg | wie #49 |
| m11 | `#nettoggle`, `#netpanel` („in Vorbereitung“, ohne Funktion) | **weg (auch Desktop-Web)** | Platzhalter ohne Funktion widerspricht §3.1 |
| m12 | `#keyboardhelp`, `#keyhelp` | weg | keine Hardware-Tastatur vorausgesetzt |
| m13 | `#log` Chatverlauf | **M-SE** | Kern |
| m14 | Rückfrage-Dialoge (`#modal`/`#dlg`) | **M-SE** Rückfrage-Karte | Kern §4.2 |
| m15 | `#prompt`, `#send`, `#abort` | **M-SE**, ein Knopf Senden/Stopp | Kern §4.2 |
| m16 | `#historyolder/newer`, `#promptexpand` | weg | Dichte |
| m17 | `#materialadd`, `#materialbar`, `#materialdetails` | weg | Dateien senden ist Desktop (T4) |
| m18 | `#statusline` (Laufaktivität), `#workspinner` | **M-LI** Zustand am Eintrag, **M-SE** eine Zeile | A1-Zustand |
| m19 | `#usagebar`, `#usagepopup` | weg (I1). Später ggf. nur Lesen in M-EI | Dichte; Verbrauch zuerst am Desktop |
| m20 | `#speech*` | weg | lokales Piper |
| m21 | `#editor` (Markdown-Editor), `#preview` Kurzinfo/Zweck | weg; Ergebnis ansehen in **M-VO** | Lesen statt Bearbeiten |
| m22 | `#toasts` | **M-SE/M-LI** Meldungsbereich | wie #43 |
| m23 | `#widgets-above/below`, `#extstatus` | weg (I1) | Extensions-UI mobil später prüfen |

**Neu mobil:** M-QR Koppeln, M-QR Rechner bestätigen, M-EI Benachrichtigungen (ohne Inhalt), M-EI „Dieses Handy abmelden“, M-VO Ergebnisvorschau.

**Transport (S5):** Die 30 `invoke`-Aufrufe in `app.js`, die 2 in `material-browser.js` und die 3 in `shared-bookmarks.js` (eigene Zählung: 35) bleiben mobil **nur**, soweit sie m1, m13–m15, m18 und M-VO bedienen: `net_sessions`, `resume_session`, `send`, `net_backends`, Rückfrage-Antwort und Abbruch. Alle übrigen entfallen mobil und brauchen keinen Hub-Endpunkt.

---

## 3. Neu in v3 (heute ohne Ort)

| Funktion | v3-Ort | Abkürzung | Plattf. |
|---|---|---|---|
| Modus anzeigen (Remote-only / Hybrid) | **D-SZ** links, immer sichtbar | – | D |
| Modus umschalten | **D-SE › Modus** | – | D (mobil: nein, §4.2) |
| Konto + R2-Identität anzeigen | **D-SZ** („Mira · account03 @ R2“) | – | D; M-LI Kopf |
| Einladung einlösen (Login) | **D-ST** | Plattformmenü „Datei › Einladung einlösen …“ | D; M-QR |
| Handy koppeln (QR zeigen) | **D-SE › Geräte** | – | D |
| Rechner per Handy bestätigen | erscheint als Dialog dort, wo die Aktion ausgelöst wurde; Scan in **M-QR** | – | D+M |
| Geräte ansehen und widerrufen | **D-SE › Geräte** (einziger Widerrufsort für Nutzerinnen) | – | D; M-EI nur „Dieses Handy abmelden“ |
| Dateifluss: Nach R2 übertragen / Änderungen prüfen / Ergebnisse holen | **D-DA** | Umschalter „Dateien“ in D-KZ | D |
| R2-Last und Quote (T5) | **D-SZ** „R2“-Feld, Klick → Details | – | D |
| Sessiongrenze erklären (T3) | Meldung an **D-SL** „Neue Session“ | – | D+M |
| Benachrichtigung „fertig / wartet auf dich“ | **M-EI** an/aus | – | M |

## 4. Streichliste (Dubletten und Weglassen)

| Gestrichen | Bleibt (einziger Ort) | Art |
|---|---|---|
| #15 „/ Befehle“-Menü | #33 Slash in Eingabe | Dublette |
| #19 eigener Thinking-Knopf | #18 Modell-Dialog | Dublette |
| #21 Schrift-Knöpfe | Einstellungen + ⌘/Strg ± | Dublette |
| #28 „In Eingabe übernehmen“ | „Kopieren“ | Dublette |
| #30 Alt+A | Esc / Stopp-Knopf | Dublette |
| #32 zweiter Knopf „Abbrechen“ | ein Knopf Senden/Stopp | Dublette |
| #35 Verlauf-Pfeilknöpfe | ↑/↓ | Dublette |
| #36 großer Prompt-Editor | wachsendes Textfeld | Dublette |
| #37 Material-Leiste | D-DA „Nach R2 übertragen“ | Dublette |
| #47 „+ Instanz“ | „Neue Session“ | Dublette |
| #48 Einzelchat-Wahl | Klick in Sessionliste | Dublette |
| #49 Raster | Sessionliste + Chat | Dublette |
| #51 rsync-Abruf | „Ergebnisse holen“ | Dublette |
| #52 Kachel-Rückfragen | Rückfrage-Karte | Dublette |
| #7–#10 Filter/Farben/Sortierung | Anheften | Weglassen |
| #12–#14, #41 KI-Hilfsmodell | – | Weglassen (E4) |
| #17 VSCodium, #26 pi-Update, #34 pi-CLI | – | Weglassen (E4) |
| #45 Backend-Häkchen, #23 Backends-Umschalter | automatische Verbindung | Weglassen |
| #53–#57 P2P-Zusammenarbeit | – | Weglassen (Flag) |
| m11 Netz-Platzhalter | – | Weglassen |

**Zählung:** 60 Desktop-Einträge, davon **14 Dubletten** gestrichen, **22 weggelassen**, 24 mit v3-Ort. Mobil bleiben 7 von 23 pi-lite-Funktionen, dazu kommen 5 neue.

## 5. Offene Punkte für S2/S5

- **F1:** Soll „Modell“ (#18) für Lernende überhaupt sichtbar sein, oder legt die Administration es je Slot fest? Vorschlag: sichtbar, aber nur Modelle aus der Freigabeliste des Slots.
- **F2:** `!cmd` (#34) für Lernende: behalten (läuft auf R2) oder hinter „Erweitert“? Vorschlag: behalten, Hilfe in F1-Liste.
- **F3:** Neue Session am Handy (m2): §4.2 verbietet nur „Projekte anlegen“. Vorschlag: in I1 weg, in I2 neu entscheiden.
- **F4:** Lehrenden-Sicht (Persona Aylin: „prüft, ob alle arbeiten können“) hat heute keinen Ort außer dem Raster/Fernzugriff, die beide wegfallen. Vorschlag: Admin-Prüfskript von S4 statt App-Funktion. Das braucht eine Entscheidung durch K.
