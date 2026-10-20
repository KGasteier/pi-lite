# Übergabe: pi-lite

## Aktueller Stand

copi-Fix 2026-10-20 (ANR-Dialog beim App-Wechsel): Hyprland zeigte „Application
Not Responding“, sobald copi auf einem anderen Workspace lag. Ursache eframe
0.36/glow unter Wayland: swap_buffers mit vsync wartet auf einen Frame-Callback,
den Hyprland unsichtbaren Fenstern nicht schickt; der Hauptthread beantwortet
keine Pings. Fix: `Main::ui` ruft `frame.winit_window().pre_present_notify()`,
winit drosselt damit Redraws, vsync bleibt an. Regressionstest
`~/.pi/agent/skills/cc-style/scripts/egui-anr-check.sh BINARY`: alte Binary
Dialog, neue keiner; 145 Workspace-Tests grün. pi-lite (Tauri) nicht betroffen.

Eingabe: Editor-Öffner als kleines Pfeilsymbol oben rechts im Eingabefeld
(#promptbox, wie Workspace-Editor-Symbol); Senden/Abbrechen 96px breit, Senden
größer (3fr/2fr). Struktur-Test + Chromium-Screenshot; WebKit live offen.

UI-Feinschliff 2026-10-09: Pixel-Schlange (8 Pixel, 2×4, CSS) vor dem Status
„arbeitet …“ oben rechts; Bookmark-Filter-Toggle in der Seitenleiste; Session-
Löschen/Umbenennen als dezente SVG-Symbole neben dem Bookmark; Cache-Löschen als
Textlink in der Vorschau; alle nativen Selects flach wie Header-Picker
(appearance:none); Material-Details kompakter; Fokusrahmen des Chatverlaufs als
Overlay (#logwrap::after), Scrolltext überlagert ihn nicht mehr. Hilfsmodell lokal
auf neueste Haiku-Version oder vergleichbar (lokale env.sh, nicht im Git). Strukturtests + Headless-
Chromium-Screenshot mit Stub; echte WebKit-Optik nicht live geprüft.
Veröffentlicht auf origin/main (inkl. Rust-Frontend-Evaluierung
docs/TODO_RUST_FRONTEND.md). Offen: WebKit-Livefeedback zu diesen Punkten, Material/
Medien live mit Provider, Medienrouten-Einrichtung (OpenRouter-IDs unverifiziert).

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

## Nachtrag 13. Oktober 2026

Beide Oberflächen: acht Bookmark-Farben, sichtbarer Session-Umbenennen-Button, lokaler Materialbrowser mit Name-/Datumssortierung und DL_Omarchy-Erkennung, Bild-/Text-/Markdown-/PDF-Erstseitenvorschau. Standard sind Originalreferenzen; kein Material-Kopieraufruf gefunden, Agentenaktionen nicht technisch gesperrt. Beide Release-Binaries installiert und Hashgleichheit geprüft; Livebedienung offen. Quellcode-Paritätsmatrix und tatsächliche Lücken: docs/FEATURE_PARITY.md. Details: docs/CORRECTIONS-2026-10-12.md.

## Menü-/Sidebar-Korrekturen

Beide Frontends: Header-Umbenennen entfernt, Edit-Button dauerhaft auf jeder Sessionkarte. /skill:-Unterliste aus get_commands; nur von pi gemeldete Skills, keine behauptete Vollständigkeit deaktivierter Skills. pi-lite / ohne Pfeil/Play, Header kompakt einzeilig (sehr schmale Fenster horizontal scrollbar). Bookmark-Modi Alle/Zeit → Bookmarks/Zeit → Bookmarks/Farbindex+Zeit. Acht globale Kategorienamen lokal editierbar. Zeit meint vorhandenes modified, nicht reines Öffnungsdatum. Invertierte Prozent-Schätzung neben Laufzeit: lokale max30 abgeschlossene Läufe, Median ab3, initial180s; bei Überschreiten verbleibende Dauer längerer beobachteter Läufe, sonst neuer180s/Median-Horizont, +Markierung, ggf Rücksprung. Keine gemessene Gesamtfortschrittsanzeige. Rust-/JS-Tests und Release-Builds bestanden; reale Layout-/Bedienprüfung offen. Kein Push dieser Änderung.

## Farbmenü-Fokus und Warnhinweise – Nachtrag

Beide Oberflächen: / Befehle statt isoliertem Slash; /skill: alphabetisch einsortiert und kursiv abgesetzt. Session-Aktionen links von Farbe/Bookmark, Löschen separat (aktive Session weiterhin geschützt). Sidebar-Symbol vergrößert. copi-Farbpopup schließt nur außerhalb; Sidebar-Tastenkürzel bei Textfokus oder offenem Popup gesperrt. pi-lite Kategoriename mit Enter oder Außenklick speichern/schließen, Eingabetasten bleiben im Namensfeld. Rust-/JS-Tests, beide Release-Builds und Hashinstallation bestanden; reale Bedienprüfung offen.

Drei Nutzer-Screenshots vom 13.10. zeigen ANR, auch auf leerer Startkarte, nicht nur langem Chat. Keine Coredumps seit10:30, kein passender OOM-/GPU-/Panic-Befund in verfügbaren Journalen11:55–12:25; Momentaufnahme5.1GiB verfügbar, Swap2.0GiB benutzt. Keine aktive copi-Instanz beim Diagnoseabruf, daher kein live Threadstack. Ursache weiterhin unbekannt; nächste sinnvolle Diagnose: während tatsächlichem Hänger Prozess-/Threadzustände und optional mit Zustimmung kurz Thread-Backtrace erfassen. Kein Grafikconfig-Wechsel, keine Sessions beendet, keine privaten Bilder committet.

### Abschluss Kategoriename / feste Statusbreite

copi: Enter bei Fokus oder Fokusverlust des Kategorien-Textfelds speichert und schließt Popup. Befehlsbutton nutzt denselben btn-Helfer13px/28px wie übrige Kopfbuttons. Beide UIs: feste Zeit/Prozent-Breite, dezenter Mittelstrich. Rust-/JS-Tests und Release-Builds bestanden, atomar installiert und Hashgleichheit geprüft; Liveprüfung ausstehend, nicht gepusht.

### Header-Zentrierung und abgeschnittener Busy-Text

copi: expliziter34px-Headerbereich mit vertikaler Align::Center-Ausrichtung statt horizontal_wrapped (dies platzierte erste Buttons am oberen Rand). Während busy redundantes abgeschnittenes Statuslabel entfernt; Zeit/Prozent/Spinner bleiben. pi-lite gleicher Busy-Text ausgeblendet, flex-align:center erhalten. Beide Tests/Release-Builds erfolgreich, atomar installiert mit Hashgleichheit. CPU-Screenshot erstellt, kein echter Live-Test. Keine Pushfreigabe für diesen Nachtrag.

## Statuszeile und Netz-Seitenleiste – 13. Oktober 2026 (7. Tranche)

- Status (Phase · Laufzeit · ≈%) ist aus der Kopfzeile in eine eigene Zeile direkt über der Eingabe gewandert, flankiert von zwei Pixel-Schlangen (rechte gespiegelt). Links: Runde/Werkzeuge/Fehler/Warteschlange während des Laufs bzw. „Letzter Lauf m:ss · …“ danach; rechts: Pfad/Befehl des laufenden Werkzeugs. Logik rein und getestet: `copi/app/src/run_activity.rs` ↔ `ui/run-activity.js` (gleiche Texte, gleiche Fälle; `scripts/test-statusline-net.cjs`, Rust-Unit-Tests).
- Rechte Seitenleiste „Netz“ als ehrlicher Platzhalter (sendet/empfängt/speichert nichts). Türkiser Schalter oben rechts, Strg/Cmd+Shift+B; Standard aus, Sichtbarkeit lokal (copi Store `net_visible`, pi-lite `localStorage pi-lite-net-visible`). Kopfzeile bleibt volle Breite; Leiste liegt zwischen Kopfzeile und Statuszeile. pi-lite < 860 px: Overlay.
- Geprüft: Unit-/Strukturtests beider Frontends, Headless-Renderings (copi `COPI_SCENE=statusline`, pi-lite Chromium mit Tauri-Stub). **Keine Live-Prüfung.** Bekannt (unverändert): in copi liegt die Materialleiste unter der Eingabe, in pi-lite darüber.
- Nachtrag: copi zeigt keinen zusätzlichen Pixel-Spinner mehr am Ende des Verlaufs (redundant zur Statuszeile; pi-lite hatte keinen). Subagenten: Aufrufe des Werkzeugs `subagent` werden gezählt – laufend „Subagenten fertig/geplant“ (parallel: Ergebnisse mit `exitCode ≠ -1`, Kette: abgeschlossene Schritte, einzeln: 0 bis Ende), danach Gesamtzahl auch in „Letzter Lauf“; rechts Agentnamen statt Rohargumente. Grundlage: `partialResult.details {mode, results}` der lokalen Extension `~/.pi/agent/extensions/subagent`; andere Subagent-Extensions mit abweichendem Format zählen nur als gestartet.

## Bookmark-Persistenz / Mehrinstanz – 14. Oktober 2026

Fehler im Schreibmodell: copi speicherte bei jeder Einstellungsänderung seinen vollständigen fenstereigenen Store, pi-lite hielt lokale Bookmark-/Farbsammlungen ohne Aktualisierung. Ältere Fenster konnten fremde Änderungen löschen. copi jetzt gesperrtes Lesen/Delta-Zusammenführen/atomarer Austausch, regulärer Schreibpfad immer copi (Legacy nur lesen), periodisches Einlesen mit Erhalt lokaler Änderungen. pi-lite aktualisiert Sammlungen vor Zugriff, hört auf storage/Fokus. Kategorienamen ebenfalls pro Farbindex erhalten. Tests mit unabhängigen Instanzen, Datei-Neustart, Löschung und beschädigter JSON-Datei bestanden; alle JS-/Rust-Tests grün. Getrennte App-Ablagen bleiben bestehen. Nutzerfenster nicht beendet, reale Mehrfensterprüfung offen. Alte Binaries vor erneuter Bookmarkpflege regulär schließen: sie haben noch den alten Überschreibfehler. Keine privaten Konfigurationsinhalte in Tests/Doku übernommen.

## Gemeinsame Ablage copi / pi-lite

Bookmarks, Farbzuordnungen und Kategorienamen liegen jetzt gemeinsam in
`~/.config/pi-frontends/bookmarks.json` (macOS Application Support, Windows APPDATA).
Beide Apps verwenden `shared/bookmarks.rs`: Dateisperre, Delta je Session/Farbindex,
atomarer Austausch. pi-lite nutzt Tauri-Kommandos und localStorage nur als Cache;
Polling jede Sekunde übernimmt Fremdänderungen. copi liest ebenfalls regelmäßig.
Einmalige Migration übernimmt vorhandene App-Markierungen; gemeinsame Zuordnungen
haben Vorrang. Migrationsmarker verhindern Wiederbelebung gelöschter Bookmarks.
Filtermodus, Schriftgröße und andere UI-Einstellungen bleiben appbezogen.
Alle alten App-Fenster regulär schließen und neue Binaries starten, bevor Bookmarks
gepflegt werden: alte Versionen nutzen noch die getrennten Ablagen.
Tests: gemeinsamer Rust-Dateitest läuft in beiden Builds, JS-Brückentest
`test-shared-bookmarks.cjs`; reale gleichzeitige Fensterbedienung offen.

Statuszeile gestrafft: keine rechte Werkzeugdetail-Anzeige mehr (Befehl/Pfad steht
bereits im Verlauf); Phase, Laufzeit/Schätzung und kompakte Zähler bleiben.
Invertierter Chip: Regular statt fett, copi mit expliziter Figtree-Regular-Familie.
Getestet: alle JS-Tests, copi-Workspace und src-tauri; reale Layoutprüfung offen.

## cocopi · erster Rust-Prototyp

Branch von copi b3c89a3, separater cocopi-Binary, kein Ersatz laufender copi/pi-lite.
Frontendfeature und derselbe copi-Code; eigenständiges pi-net mit iroh1.3.0.
Echte direkte Kartenübertragung zweier lokaler Endpunkte getestet; keine MLS-/
authentik-/Seminarfreigabe. UI mit expliziter Zustimmung, Status weiter über Eingabe.
Release gebaut, separat installiert; CPU-Headless-Layout geprüft, kein Live-UI-Test.
Tauri vorbereitet durch serde-Kern, Adapter noch nicht implementiert.
Upstream-Abgleich explizit per copi/scripts/update-cocopi.sh, nicht unbeaufsichtigt.
Stand/Grenzen/Todos: COCOPI.md; Server/authentik/KI-API: COCOPI_DEPLOYMENT.md.

## cocopi veröffentlichen / Testanleitung

PDF-Übersicht `cocopi-first-build.pdf` (10 A4-Seiten), HTML-Quelle, Quickstart mit
Zwei-Personen-LAN-Test und separatem install-cocopi.sh ergänzt. Nur Branch cocopi
veröffentlichen; copi/main unverändert. PDF benennt fehlendes MLS/Relay/authentik
und nicht synchronisierte Räume deutlich. Referenzierte Konzept-PDF unverändert.

## Sidebar / Prozentzeichen – Korrektur

„Andere Agent Harnesses“ vorerst aus beiden Sidebar-Auswahlen entfernt; Backend-
Inventar unverändert für spätere Wiederaufnahme. Status-Regular hatte keine
Fallback-Schriften, daher fehlte das bereits im Quellcode vorhandene ≈. Jetzt
Regular mit eingebetteter Fallback-Kette; Regression prüft ≈-Glyphabdeckung.
JS-Sidebar-/Format-Test, Rust-Workspace, Tauri und Medientests bestanden.
Status bleibt über der Eingabe. Live-Bedienprüfung nach Neustart offen.
