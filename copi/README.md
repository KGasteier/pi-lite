# copi – native Oberfläche für pi (Rust + egui)

copi ist der Rust-/egui-Nachfolger von pi-lite ohne WebKit. Er entsteht auf dem Branch `copi` neben pi-lite; pi-lite bleibt bis zur Parität die stabile Oberfläche.

## Installation auf Omarchy

Reproduzierbare Anleitung, Pflicht-/optionale Abhängigkeiten und offene Prüfungen:
[INSTALL_OMARCHY.md](docs/INSTALL_OMARCHY.md). Benutzerinstallation: `bash scripts/install.sh`.

## Aufbau

| Crate | Zweck |
|---|---|
| `crates/md-core` | GUI-freier Markdown-Parser + Syntax-Highlighting (wiederverwendbar, z. B. Outliner) |
| `crates/md-egui` | Markdown-Darstellung für egui (`show_markdown`, `show_doc`, `DocCache`) |
| `crates/pi-backend` | pi-RPC-Prozess, Sessions, pi-CLI, Ordner, Editor-Öffner – ohne Tauri |
| `app` | Binary `copi` (Zustand `app.rs`, Darstellung `view.rs`, fensterloser Screenshot `headless.rs`) |

## Bauen und starten

```sh
cd copi
CARGO_BUILD_JOBS=2 cargo build --release -p copi
./target/release/copi                    # echtes Fenster, startet pi
COPI_DEMO=1 ./target/release/copi        # Demo-Daten, startet pi nicht
./target/release/copi --shot /tmp/c.png  # fensterloser Screenshot (immer Demo)
COPI_SCENE=slash ./target/release/copi --shot /tmp/c.png   # slash|picker|dialog|confirm|keyhelp|start|toast|inventory|longlog
```

`COPI_*`-Umgebungsvariablen fallen auf die `PI_LITE_*`-Varianten zurück. Zustand: `~/.config/copi/state.json`.

## Stand (Stufe 1–3, Live-Prüfungen teilweise offen)

Vorhanden:
- Eingebettete Figtree-Schrift (SIL OFL); proportionale UI-Größen und umbrechende Kopfzeile am Original orientiert
- RPC mit pi, Streaming-Markdown, Thinking-, Tool- und Shell-Blöcke, Abbrechen
- Sessions-Seitenleiste (Sortierung, Bookmarks, Umbenennen, Löschen, Kontextmenü)
- Kopfzeile: Workspace, VSCodium, Modell, Denkstufe, Fork, Schriftgröße
- Slash-Liste mit lokalen Befehlen, History, `!cmd`/`!!cmd`, `pi <sub>`, Fork/Clone
- Extension-Dialoge (select/confirm/input/editor), Statuszeilen, Widgets, Toasts
- Tokenzeile, Vorlesen-Umschalter, großer Eingabe-Editor, Tastenhilfe (F1)

## Codeprüfung

Korrekturen, verbleibende Optimierungen und Grenzen der Tests: [REVIEW.md](REVIEW.md).

## Paritäts-Checkliste (Stufe 3)

- [x] Floating-Vorschau für Sessions/Dateien; lokale Kurzinfo, KI nur nach Zustimmung (keine Bild-/PDF-Seitenansicht)
- [x] Interner Dateieditor mit Schreibschutz und Konfliktprüfung; Endpunkt-Assistent noch offen
- [x] Material-Anhänge und zustimmungspflichtige Medienanalyse (reale Routen nicht live geprüft)
- [x] Ordner-Browser
- [x] Inventar-Ansichten (Extensions, Skills, Agents, Harnesses)
- [x] Ausführliche Token-Erklärung
- [x] Log-Virtualisierung für lange Sessions; inkrementelle Session-Metadaten; begrenzter RPC-Schreibthread
- [x] Tastaturnavigation in Session-Karten (↑↓, R/F2, Entf, M) und Alt+3/4/5
- [x] Editor-Verweise (Datei:Zeile)
- [ ] Schmale Sprechblasen (shrink-to-fit)
- [ ] Echtfenster-Prüfung: IME, Textauswahl, Super+A, Alt+Ziffer, Stabilität unter virgl
