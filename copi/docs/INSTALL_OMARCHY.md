# copi auf einer frischen Omarchy-VM installieren

copi benötigt kein WebKit, Tauri, Electron oder Python. Der Quellcode liegt derzeit im
Branch `copi` des pi-lite-Repositories. ARM64 und x86_64 müssen jeweils nativ gebaut werden:
eine ARM64-Binary läuft nicht direkt auf einer x86_64-VM.

## 1. Grundwerkzeuge und Grafik

Auf Omarchy/Arch (im eigenen Terminal):

```bash
sudo pacman -S --needed base-devel git rustup nodejs npm mesa libglvnd libxkbcommon wayland libx11 libxcursor libxi libxrandr
rustup default stable
```

Falls Rust bereits über mise oder rustup installiert ist, keine zweite Toolchain installieren:
`rustc --version` und `cargo --version` prüfen. Die aktuellen Cargo-Abhängigkeiten benötigen
eine aktuelle stabile Rust-Version; der Installer prüft nicht vorab die genaue Mindestversion.
Cargo meldet eine zu alte Toolchain beim Build.

In der VM braucht egui/eframe eine funktionierende OpenGL/EGL-Grafikumgebung.
Bei QEMU/virgl: virtuelle GPU/3D-Beschleunigung und aktuelle Gast-Mesa-Treiber prüfen.
Ein erfolgreicher Build oder CPU-Screenshot beweist nicht, dass virgl im echten Fenster stabil ist.

## 2. pi installieren und separat einrichten

```bash
npm install -g @mariozechner/pi-coding-agent
pi --version
pi
```

In pi den gewünschten Provider anmelden/konfigurieren. Benutzerdefinierte Endpunkte und
Erweiterungen sind **nicht** Teil von copi und müssen separat eingerichtet werden.
Keine fremden Zugangsdaten oder Sessions kopieren. copi startet `pi` aus dem `PATH`;
dieser muss auch aus dem grafischen Launcher erreichbar sein. Wenn das Terminal pi findet,
der Launcher aber nicht, die PATH-Einrichtung des grafischen Logins prüfen.

## 3. Quellcode holen, prüfen und installieren

```bash
git clone --branch copi https://github.com/KGasteier/pi-lite.git
cd pi-lite/copi
CARGO_BUILD_JOBS=2 cargo test --locked
bash scripts/install.sh
```

Der Installer baut mit `--release --locked` und installiert nur für den aktuellen Benutzer:

- `~/.local/bin/copi`
- Launcher-Eintrag `copi`, Kategorie `X-Agent`
- skalierbares coπ-Icon (mit Unterstrich)

Er beendet keine laufenden Fenster. Erst nach erfolgreichem Build wird die installierte
Binary ersetzt. Launcher ggf. erneut öffnen; es werden keine Hyprland-Regeln verändert.

```bash
~/.local/bin/copi
COPI_DEMO=1 ~/.local/bin/copi
```

Die Demo startet keinen Agenten. Test ohne Fenster:

```bash
~/.local/bin/copi --shot /tmp/copi-demo.png
```

## 4. Optionale Funktionen

| Werkzeug/Konfiguration | Wofür? | Prüfung derzeit |
|---|---|---|
| `pi` samt Provider-Anmeldung | Agent, Modellliste, KI-Hilfe | Installer warnt nur bei fehlendem Befehl; Anmeldung wird nicht getestet |
| `pdftotext` (`sudo pacman -S poppler`) | PDF-Text als Material | Erst bei PDF-Verarbeitung geprüft; saubere Fehlermeldung |
| VSCodium (`vscodium-electron`) oder VS-Code-kompatibler Editor | Externer Editor, Datei:Zeile | Erst beim Öffnen geprüft; `COPI_EDITOR_CMD` überschreibt den Befehl (z. B. `codium` oder `code`) |
| `curl` | Abfrage der neuesten pi-Version | Optional; fehlender Befehl bedeutet keine Versionsabfrage |
| `xdg-open` (`xdg-utils`) | Weblinks | Erst beim Klick geprüft |
| `kill` | Unix CLI-Prozessgruppen abbrechen | Nicht vorab geprüft; auf Omarchy normalerweise vorhanden |
| Medienrouten + Schlüssel im angegebenen Environment | Audio-/Videoanalyse | Routen lokal validiert, Endpoint/Modell bei expliziter Analyse geprüft |
| Vorlesen-Erweiterung für pi | Sprachausgabe | Verfügbarkeit über pi-Befehlsliste erkannt; nicht mitgeliefert |
| `update-desktop-database` (`desktop-file-utils`) | Launcher-Datenbank aktualisieren | Optional im Installer |

KI-Hilfsmodell in der Vorschau über **KI-Hilfsmodell auswählen…** aus der geladenen Modellliste
wählen. Die Auswahl bleibt lokal gespeichert; sie ändert nicht das Hauptmodell und stellt
keine Provider-Anmeldung her. Jede KI-Erklärung verlangt separate Upload-Zustimmung.

Medienrouten bevorzugt im copi-Konfigurationsordner, mit Legacy-Fallback auf pi-lite;
Schema siehe `../../examples/media-routes.json`. Provider-Schlüssel nicht ins Repository schreiben.
Es gibt keinen stillen Providerwechsel und keine automatisch ausgewählten Modell-IDs.

## Grenzen und nicht vorab geprüfte Abhängigkeiten

Es gibt noch keinen vollständigen Start-Selbsttest. Insbesondere GPU/EGL, grafischer PATH,
Provider-Zugang, optionale Werkzeuge, Speech-Backend und reale Medienanfragen werden nicht
vollständig vom Installer geprüft. Netzwerk wird für den ersten Cargo-Build und npm benötigt;
`--offline` funktioniert nur mit bereits geladenen Abhängigkeiten.

Die Entwicklung und Builds sind hier unter Linux/ARM64 geprüft. Windows/macOS-Zweige sind
vorhanden, aber nicht gleichwertig live getestet. Auch unter Omarchy bleibt die gemeldete
„Application Not Responding“-Situation ein offener Live-Prüfpunkt; keine Stabilitätsgarantie.

## Binary-Größe und Rust zur Laufzeit

Die hier gebaute Linux/ARM64-Release-Binary ist rund **8,47 MB (8,08 MiB)** groß
(Stand 10.10.2026, vor der kleinen Statuszeilenkorrektur). Schriften und UI sind eingebettet;
Größe kann mit Architektur und Build-Version abweichen. Icon/Launcher benötigen nur wenige KB.

**Rust/Cargo wird nur zum Bauen oder Weiterentwickeln benötigt**, nicht zum Ausführen einer
passenden fertigen Binary. Die rund 5 GB im lokalen `target/` sind Build-Artefakte und keine
Laufzeitinstallation. Die Binary ist nicht völlig autark: pi (einschließlich dessen Runtime),
Systembibliotheken und Grafiktreiber bleiben externe Abhängigkeiten. Ihre Gesamtgröße ist
installationsabhängig. Es gibt derzeit keine vorgefertigten, plattformübergreifenden Release-Pakete.

## Aktualisieren

```bash
cd pi-lite
git switch copi
git pull --ff-only
cd copi
bash scripts/install.sh
```

Danach copi manuell neu starten. pi selbst und seine Erweiterungen separat aktualisieren.
