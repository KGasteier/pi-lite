# Entwicklungsprozess

pi-lite entstand in einer einzigen langen Sitzung zwischen dem Nutzer und einem KI-Agenten
(pi/Claude) auf einem Arch/Omarchy-System (aarch64, Hyprland). Der Nutzer gab Ziel, Reihenfolge und
Einschränkungen vor; der Agent hat analysiert, gebaut und getestet. Dieses Dokument hält fest, wie es lief,
damit sich der Weg nachvollziehen und später ein Repository sauber daraus ableiten lässt.

## Ausgangslage

Der Nutzer verwendete `pi-ui` (hyperpuncher/pi-ui, Paket `pi-ui-bin` 0.55.0): ein 142-MB-Bun-Binary mit
HTTP-Server, das im Browser bedient wird. Frage: Geht das als Standalone-App ohne Webserver?
Ergebnis der Analyse: nicht ohne Umbau, da die Oberfläche Server-Endpunkte voraussetzt.

Daraus der Auftrag: ein **eigenes, schlankes GUI-Projekt** – ohne Electron, klein, anpassbar,
Mac/Windows/Linux – das sich wie pi-ui verhält.

## Randbedingungen (vom Nutzer)

- Kein sudo-Passwort: alles im Home-Verzeichnis (Rust per `mise`, Installation nach `~/.local`).
- Die Hyprland-Konfiguration des Nutzers wird nicht angefasst, auch keine Fensterregeln.
- Tests nicht floatend, sondern im Vollbild.
- Antworten auf Deutsch; Nicht-Getestetes ehrlich als ungetestet benennen.
- Der Nutzer bestimmt die Reihenfolge der Arbeitsschritte.

## Verlauf (grob chronologisch)

1. **Schnittstelle gefunden:** `pi --mode rpc` (JSONL über stdin/stdout) ersetzt jeden Webserver.
2. **Technik gewählt:** Tauri 2 mit System-Webview; Frontend in Vanilla-JS ohne Build-Schritt.
3. **Prototyp:** Fenster, Prozessstart, Streaming-Chat, Tool-Blöcke, Senden/Abbrechen/`steer`.
4. **Markdown und Highlighting** (marked, highlight.js, DOMPurify), Rendering gedrosselt.
5. **Sessions:** Liste, Öffnen mit `--session`, „+ Neu“, aktive Markierung.
6. **Modell-/Thinking-Auswahl**, Auto-Scroll nur am unteren Rand.
7. **Fenstergröße:** Hyprland kachelt das Fenster klein → responsives Layout mit ☰-Overlay.
8. **Sessionnamen** aus `session_info`, mit mtime-Cache.
9. **Extension-Dialoge** (`select/confirm/input/editor`, Toasts, Status, Widgets) mit Warteschlange.
10. **Umbenennen, Fork, Löschen** inkl. Pfadprüfung beim Löschen.
11. **Kontextmenü und Tastaturweg** (`Strg/Cmd+B`, Pfeile, `M`, `R`, `Backspace`), Fokusrückgabe nach Dialogen.
12. **Launcher** (`.desktop`, Icon, `scripts/install.sh`).

## Fehlerchronik und Lehren

| Problem | Ursache | Lösung / Lehre |
|---|---|---|
| „pi wurde beendet“ nach „+ Neu“/Fork, obwohl pi lief | Alter Lese-Thread meldete sein Ende, weil der Generationszähler erst *nach* `stop()` erhöht wurde | Zähler vor `stop()` erhöhen. *Beleg:* Fehler trat im anschließenden Lauf nicht mehr auf; ein gezielter Fehlschlag-Test existiert nicht. |
| Seitenleiste unerreichbar im schmalen Fenster | Hyprland-Tiling (709×426) | Responsives Layout; geschlossene Leiste `visibility:hidden` |
| Hover-Knöpfe (✎/🗑) taugen nicht als Hauptweg: auf dem Mac gibt es keine `Entf`-Taste, per Tastatur kaum erreichbar | Designfrage, kein Bug | Kontextmenü + Tastenkürzel (`Backspace`, `M`, `Shift+F10`) |
| Test-Eingaben landeten im falschen Fenster (mehrfach) | Fokus wechselt durch Tiling/neue Fenster; `focuswindow class:` unzuverlässig | Fokus per Adresse setzen und **vor jeder Eingabe die Fensterklasse prüfen** |
| Vollbild landete einmal auf einem anderen Fenster | `fullscreen` wirkt aufs aktive Fenster | Erst Fokus prüfen, dann umschalten; danach alle Fenster kontrollieren |
| „Line 4“ im Editor-Test wurde als Prompt gesendet | Testskript schloss den Dialog zu früh mit Return (Testfehler, kein App-Fehler) | Schrittweise mit Screenshots |
| Nach einem Dialog sprang der Fokus ins Eingabefeld | `closeDialog` fokussierte immer `#prompt` | `returnFocus` merkt das auslösende Element |

Allgemeine Lehren:

- Dokumentation lesen: Die pi-RPC-Doku verlangt LF-Trennung und beschreibt die Dialog-Protokolle genau.
- Jede Änderung am Frontend braucht einen `cargo build` (UI eingebettet). Ein Build dauerte zuletzt rund 4 Minuten (aarch64, LTO).
- Echte Tests erzeugen echte Sessions und kosten echte Modellaufrufe; Testreste aufräumen.
- Bedienungen, die man nicht mit der Maus prüfen kann (Rechtsklick), offen als ungetestet führen.

## Quellen für den Prozess

- Die komplette Unterhaltung liegt als pi-Session unter `~/.pi/agent/sessions/<workspace-session-directory>/`
  (Sessions mit „audacity“/„pi-lite“ im Titel; Dateien sind `.jsonl`). Sie enthält auch Unbeteiligtes
  (Audacity-Ersetzung) und lokale Pfade – vor einer Veröffentlichung sichten und ggf. nicht mitgeben.
- Kopie der Prozessdokumentation: dieses Verzeichnis. Der Code-Stand ist in Git festgehalten (`git log`).
