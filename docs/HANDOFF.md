# Übergabe: pi-lite

## Aktueller Stand

Eingabebereich: links SVG-Spalte Material/History ↑/↓, rechts Editor/Senden/
Abbrechen in gleicher Dreierteilung. Vorlesen/Stopp unten rechts als SVG statt
Emoji. Footer-Strukturtest, alle JS-Tests, Rust/Release; installiert. WebKit-Optik
nicht live geprüft.

Kopfbereiche aufgeräumt: Editor nur noch kleines 26px-Symbol, im gemeinsamen
Workspace-Rahmen direkt am Picker. Sidebar: Kategorieauswahl statt doppeltem
Titel, einheitliche kompakte Aktionszeile (Neu/Sortierung/Cache); gleicher Aufbau
für Skills/Extensions/Agents/Harnesses. Inventarhinweis gekürzt, volle Erklärung
im Tooltip. Strukturtest test-sidebar-header.cjs und bestehende Regressionstests,
Rust/Release geprüft; installiert. Keine echte WebKit/Layout-Liveprüfung.

Session-Bookmarks: dauerhaft sichtbares Umriss-/gefülltes Symbol unten rechts;
Klick ohne Sessionöffnung, Kontextmenüaktion, lokaler persistenter Status.
Kompakte Sortierung Zeit/Bookmarks in Überschrift, nur Sessions-Ansicht.
Details/Testgrenzen: SESSION_BOOKMARKS.md. Release gebaut und installiert;
realer WebKit-Bedientest weiterhin offen.

Material erster Ausbau: + mit Portal-Mehrfachauswahl/URL-Liste, entfernbare Chips,
explizites Senden, automatische Bildübergabe und begrenzte lokale PDF-Extraktion.
Details: nur Verweise / manuelles Modell; Bildfallback gleicher Anbieter mit
Bestätigung. Separater Audio/Video-Adapter: MEDIA_ANALYSIS.md; bestätigte,
authentifizierte Routen, Laufzeit-Katalogprüfung, Zustimmung vor Upload,
Hauptmodell unverändert. Keine parallelen Jobs/Worker-Abbruch oder festen Kostenlimits;
keine Lokal-only-Garantie. Noch keine reale Route aus modelle.pdf eingerichtet.
Vollständiger Stand: MATERIAL.md und MEDIA_ANALYSIS.md.
Rust-Materialtests, Syntax/bestehende Regressionstests und Release geprüft;
neu installiert. Native Dateiauswahl/echte Provideranalyse/WebKit bleiben offen.

Slash-Korrektur nach Nutzerscreenshots: /model als lokale Modellauswahl ergänzt;
/mo und /model filtern nur Befehlsnamen-Präfixe statt Beschreibungen (keine falschen
/llama-Treffer). Skills auch ohne skill:-Präfix suchbar. Alphabetische Sortierung.
Klick/Enter führen direkt aus, Tab vervollständigt für optionale Argumente.
Wizard-Optionen ohne globalen brightness-Hover und äußeren Fokusrahmen; aktive
Auswahl hat nur einen Akzenthintergrund. Slash-Beschreibungen auf zwei Zeilen begrenzt.
Regression/Syntax/bestehende Tests und Release geprüft; neu installiert.
Echte WebKit-Interaktion weiterhin manuell durch Nutzer zu prüfen.

Wizard-select korrigiert: keine statische pinke erste Aktion mehr; Fokus/Hover
synchronisieren die Auswahl. ↑↓, Home/End, PageUp/PageDown navigieren mit explizitem
ScrollIntoView. Liste max. 60vh, min-height:0, nicht schrumpfende Einträge, sichtbare
Scrollbar; Zähler zeigt Position und tatsächliche Optionszahl. Regression mit 25
synthetischen Optionen bestanden; Syntax/bestehende JS/Rust/Release geprüft.
Neu installiert, echte WebKit-Bedienung noch durch Nutzer zu prüfen.
„New models“ filtert in der Extension bereits konfigurierte IDs heraus; 14 neue
Modelle sind nicht zwingend eine abgeschnittene Liste von 25 Gesamtmodellen.

Slash-Auswahl über Prompt: / öffnet dynamische RPC-Befehle, tippen filtert,
Hover/↑↓ markieren, Tab/Enter/Klick übernehmen, danach Enter führt aus, Esc schließt.
Extension-Aufrufe verwenden prompt auch bei Streaming; Wizard ohne kurzen Timeout.
Lokale endpoint-custom-Extension mit Backup um RPC-select-Fallback ergänzt;
Opt-in-Skript scripts/adapt-endpoint-rpc.py. Details/Testgrenzen: SLASH_COMMANDS.md.
DOM-Stubtests, Syntax/bestehende JS-Tests, Rust/Release bestanden; realer separater
RPC-Test entdeckt endpoint-custom, öffnet Wizard-select und bricht ohne Änderung ab.
Neu installiert; laufende Session unberührt, Neustart erforderlich. Echte WebKit-
Interaktion und vollständiger Wizard noch nicht live geprüft.

Nutzeranfragen im Chat: Hintergrund nutzt die Highlight-/Akzentfarbe --acc
(#e454c4), dunkler Text (#1e2227) für Kontrast; Übernahmepfeil erbt die Textfarbe.
Gilt auch für geladene Verläufe. Syntax, History/Preview/Footer, Rust-Tests und
Release geprüft; neu installiert, SHA256 identisch. Darstellung in laufendem
WebKit noch nicht geprüft, Neustart nötig.

VSCodium-Anbindung: Workspace-Button ↗ Editor, lokale Inline-Code/Markdown-Datei-
referenzen im Assistentenchat, Read/Write/Edit-Toolaktionen und Datei-Editor-Button.
Relative Chatpfade binden an den tatsächlichen Session-Workspace. Explizite Klicks,
keine Shell, nur existierende kanonisierte Ziele; Zeile/Spalte unterstützt.
Kein zweiter Agent und keine automatische KI-Übertragung. Siehe EDITOR_INTEGRATION.md.
Parser-/Rust-Pfadtests und bestehende Checks/Release bestanden. Installation erfolgt;
reale Klicks und VSCodium-Zeilensprung noch nicht live geprüft; Neustart erforderlich.

Nachtrag: Doppelte Vorlesen-Statuszeile wird beim Rendern ausgefiltert, der Status
bleibt für den Schalter rechts unten erhalten. Andere Extension-Meldungen bleiben
sichtbar. Rechts vom Prompt stehen Editor/Senden/Abbrechen nun in einer 76-px-
Spalte, drei gleich hohen Reihen mit 4-px-Abstand, bündig zum Eingabefeld.
Test scripts/test-footer.cjs ergänzt; Syntax, bestehende JS-Tests, Rust-Tests,
Release und Installation erfolgreich, SHA256 identisch. Isolierter Chromium-Check
bestätigt gleiche Breiten/Höhen, Abstände und bündige Ober-/Unterkanten.
Nutzer bestätigt nach Neustart: Änderungen funktionieren. Automatisierte Prüfung
und isolierter Layoutcheck sind oben dokumentiert; keine eigene ferngesteuerte
WebKit-Prüfung durchgeführt.

Korrektur 2026-10-07: Der vorherige Layoutversuch war nur im Quellcode vorhanden.
Jetzt Release gebaut und installiert, SHA256 identisch. Editor/Senden bilden eine
62-px-Spalte; Vorlesen/Stop stehen rechts in der unteren Tokenleiste. Picker-Knöpfe
haben eine explizite Hover-Farbe mit Rahmen; Dropdown-Einträge eigene Hover-Zustände.
Super+A erkennt Meta sowie WebKitGTK-Super/OS und zurückgesetzten Held-Key-Fallback.
Kein unmodifiziertes A wird abgefangen, keine Hyprland-Regeln verändert.
Syntax, History/Preview/Picker/Super-Tests, Rust-Tests und Build bestanden.
Isolierter Chromium-Layoutcheck: Buttons gestapelt, 62 px breit, Sprache am unteren
Rand. Dies ersetzt keinen echten WebKit-Maus-/Tastaturtest; dieser bleibt offen.
Das laufende Chatfenster wurde nicht beendet und verwendet noch die alte Binary.
Neustart erforderlich.

Eingabe: Option/Alt+Enter setzt explizit einen Zeilenumbruch an der Cursorposition
(ersetzt eine Auswahl), Enter und Shift+Enter senden. Platzhalter und F1-Hilfe
angepasst. Syntax-/Logiktests, Rust-Tests und Release-Build erfolgreich; installiert.
Reale Tastaturbedienung im laufenden WebKit-Fenster noch nicht geprüft; Neustart nötig.

Korrektur Editor/Installation: Die UI-Commits d46cd7f und 6a95e48 waren zunächst
nur im Quellcode, nicht in der installierten Binary. Jetzt Release neu gebaut und
installiert, SHA256 von Build und Installation identisch. Laufendes Fenster nicht
beendet; es braucht einen Neustart, bevor die neue Oberfläche sichtbar ist.
MD-Editor: Speichern hellgrau bei Schreibschutz, eigene SVG-Schlösser geschlossen/offen.
Prompt-Erweiterung über ⤢: frei editierbar ohne Schloss, Speichern übernimmt nur in
die Eingabe, kein Senden. Modelle: eigene Listbox statt nativer WebKit-Auswahl;
Pfeile/Home/End/PageUp/PageDown bewegen Markierung, Enter/Klick bestätigen, Esc
verwirft. Navigation mit 80 Einträgen logisch getestet (scripts/test-model-picker.cjs).
Syntax, History-/Preview-/Modelltests, Rust-Tests und Release-Build bestanden.
Reale WebKit-Darstellung und Navigation weiterhin NICHT live geprüft.

Privates GitHub-Projekt, main. Letzter Funktionsstand: Markdown in Vorschaufenstern
(KI-Erklärung, Cache-Treffer, lokaler Auszug, Token-Erklärung) über den bestehenden
DOMPurify-geschützten Renderer. Gebaut und lokal installiert; Darstellung noch
nicht live geprüft. Frühere private Git-Historie nicht veröffentlicht.

## Vorhandene Funktionen

Streaming-Chat/Thinking/Tools; Sessions öffnen/umbenennen/löschen/forken; Modell und
Thinking; Workspace-Shortcuts/Ordnerbrowser; Ressourceninventar und MD-Editor;
Tastaturhilfe F1; History ↑/↓ an Textgrenzen plus Alt+↑/↓ und Pfeilknöpfe;
Copy je Assistentenblock; kleines ↻ je Anfrage übernimmt Text nur ins Eingabefeld;
Tokenfußzeile und KI-Erklärung; lokale/gecachte Vorschau plus optionale KI-Zwecke;
optionale vorhandene Piper-Vorlesen-Extension; Omarchy-Setup und Desktopstarter.

## Offene Prüfungen (nicht als bestanden darstellen)

- Neue Markdown-Vorschauen in echtem WebKit ansehen.
- Clipboard, Anfrageübernahme, Tab-Reihenfolge, Dialognavigation und Ordnerbrowser.
- Tokenberechnung/Format mit pi-TUI vergleichen; Erklärung real testen.
- Speech-GUI und tatsächlichen Audio-Output prüfen (RPC-Status bereits geprüft).
- Frische Omarchy-Installation und x86_64 prüfen; macOS/Windows nicht unterstützt.
- Abbruch/Steer, Fehlerfälle und parallele/alte RPC-Ereignisse genauer testen.
- Nicht nachgewiesene Crashursachen nicht als Omarchy-/WebKit-Bug behaupten.

## Arbeitsweise

Siehe skills/pi-lite-dev/SKILL.md und CONTRIBUTING.md. Build mehrere Minuten auf
ARM. Nicht ungefragt den Nutzer-Desktop fernsteuern. Die vorhandene Unterhaltung
ist privat und darf nicht ins Repository kopiert werden. Sie kann in pi-lite über
Sessions geöffnet werden, wenn der bisherige Prozess sie nicht mehr schreibt.

## Fortsetzung in einer anderen Umgebung

Skill direkt lesen oder nach pi-Standard in einen Skills-Ordner installieren.
README enthält Setup und optionale Hilfsmodellkonfiguration. Zugang zum privaten
Repo benötigt eine Einladung. Kontext aus diesem Dokument ist eine Zusammenfassung,
keine Kopie des vollständigen Chats.

## Bestehende Session fortsetzen

`PI_LITE_RESUME_SESSION=/absoluter/pfad/session.jsonl pi-lite` öffnet eine vorhandene
Session beim Start. pi-lite dafür aus deren ursprünglichem Workspace starten.
Keine zweite Instanz oder TUI parallel auf derselben Session schreiben lassen.
Der private lokale Fortsetzungsstarter ist nicht Teil des Repos.
