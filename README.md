**Deutsch** · [English](README.en.md)

> **Branch cocopi:** Rust/egui-Prototyp für direkten P2P-Austausch. Einstieg:
> [Installation und Zwei-Personen-Test](docs/COCOPI_QUICKSTART.md) ·
> [PDF-Übersicht](docs/cocopi-first-build.pdf) ·
> [Stand und Grenzen](docs/COCOPI.md).
> Noch kein MLS-/authentik-Gruppennetz und nicht für vertraulichen Lehreinsatz freigegeben.

# pi-lite

Ein schlankes Desktop-Frontend für [pi](https://github.com/earendil-works/pi).
Tauri 2, System-Webview und Vanilla-JavaScript: kein Electron, kein Webserver.

**Prototyp:** entwickelt unter Omarchy/Arch Linux (aarch64). Installation auf einer
frischen Omarchy-Maschine und x86_64 noch nicht geprüft. macOS/Windows sind nicht
als fertige Installationsziele unterstützt. Neueste UI-Funktionen teilweise noch
nicht live geprüft; siehe [Teststand](docs/TESTING.md).

## Installation auf Omarchy

Voraussetzung: eine funktionierende, konfigurierte pi-Installation, `pi` im PATH,
Git und Internet für Paket-/Rust-Downloads. Für die aktuellen RPC-Funktionen wird
pi 1.0.4 empfohlen. Kein API-Schlüssel wird mitgeliefert.

```bash
git clone git@github.com:KGasteier/pi-lite.git
cd pi-lite
./scripts/setup-omarchy.sh
```

Das private Repository erfordert eine GitHub-Einladung und passende Anmeldung.
Der Installer prüft pi, installiert fehlende Arch-Systembibliotheken mit `sudo
pacman`, richtet bei Bedarf Rust über vorhandenes mise oder pacman ein, testet und
baut das Projekt. Anschließend installiert er Binary, Wrapper, Icon und
Desktop-Eintrag unter `~/.local`. Im Omarchy-App-Launcher nach **pi-lite** suchen.
Er verändert keine Hyprland-Regeln und kopiert keine pi-Zugangsdaten.

Systempakete benötigen ggf. dein sudo-Passwort. Der Rust-Release-Build dauert auf
ARM einige Minuten; mehrere GB freier Speicher sind erforderlich. Bei einem
teilaktualisierten Arch-System zunächst das normale vollständige Systemupdate
vornehmen. Der Installer führt kein ungefragtes komplettes Systemupdate aus.

Bereits gebaut? `./scripts/install.sh` installiert nur die Benutzerdateien.
`PREFIX` überschreibt den Installationspfad. Updates: `git pull`, dann Setup erneut.

## Funktionen

Streaming-Chat, Markdown/Code, Thinking und Tools; Sessionliste mit Öffnen,
Umbenennen, Löschen und Fork; Modell/Thinking-Wahl; Workspace-Auswahl; Ressourcen-
Browser und Markdown-Editor; Eingabe-History; Copy-/Anfrageübernahme-Knöpfe;
Tastaturhilfe (F1); [Slash-Befehlsauswahl im Eingabefeld](docs/SLASH_COMMANDS.md);
Tokenanzeige und optionale KI-Erklärungen. VSCodium-Anbindung:
Workspace und referenzierte Dateien per Klick im externen Editor öffnen, ohne
zweiten Agenten (siehe [Editor-Anbindung](docs/EDITOR_INTEGRATION.md)).
[Session-Bookmarks](docs/SESSION_BOOKMARKS.md): kleine persistente Markierungen,
Sortierung nach Zeit oder Bookmarks und Filter „nur Bookmarks“ in der Seitenleiste.
Arbeitsanzeige oben rechts mit animierter Pixel-Schlange vor „arbeitet …“.
[Material hinzufügen](docs/MATERIAL.md): Dateien/URLs für die nächste Anfrage,
Bildübergabe, lokaler PDF-Auszug und manuelle Verarbeitung-/Modellauswahl.
[Audio-/Videoanalyse](docs/MEDIA_ANALYSIS.md): isolierter OpenRouter-Aufruf über
bestätigte Routen mit Zustimmung, ohne Wechsel des Hauptmodells. Einrichtung
nötig; parallele Spezialjobs und feste Kostenlimits noch nicht implementiert.

## Optionale KI-Hilfe

„Zweck?“ und die Token-Erklärung verwenden ein separates, toolfreies pi-RPC.
Provider und Modell müssen in der Umgebung gesetzt werden, z. B. dein bei pi
konfigurierter Haiku-Provider und die dort gültige Modell-ID:

```bash
export PI_LITE_HELP_PROVIDER='dein-provider'
export PI_LITE_HELP_MODEL='deine-haiku-modell-id'   # neueste verfügbare Haiku-Version empfohlen
pi-lite
```

Für Desktopstarts können dieselben Export-Zeilen in
`~/.config/pi-lite/env.sh` stehen (lokale Shell-Datei, nicht committen). Der
Launcher lädt diese Datei, wenn vorhanden.
Ohne Konfiguration bleibt der Chat nutzbar; KI-Hilfe zeigt einen erklärenden Fehler.
Daten werden nur nach Zustimmung übertragen. Datei-Erklärungen können Auszüge
und Pfade enthalten; lokale Cache-Dateien enthalten möglicherweise vertrauliche
Texte. Cache-Löschung über den Textlink unten in der Vorschau. Keine Credentials im Repo ablegen.

## Optionale Sprache

Eine bereits vorhandene, kompatible lokale `vorlesen`-Extension kann durch den
Installer mit Backup für pi-lite angepasst werden. Kein TTS-Download, keine
universelle TTS-Installation. Opt-out: `PI_LITE_SETUP_SPEECH=0`.
Details und Voraussetzungen: [Sprache](docs/SPEECH.md).

## Zusammenarbeit

Änderungen in Feature-Branches, Pull Requests nach `main`. Tests:

```bash
node --check ui/app.js
node scripts/test-history.cjs
node scripts/test-preview.cjs
(cd src-tauri && cargo test --locked)
```

UI-Dateien sind eingebettet: nach Änderungen neu bauen/installieren. Keine privaten
Sessions, Schlüssel, Provider-Konfigurationen, Screenshots oder Buildartefakte
committen. Siehe [CONTRIBUTING.md](CONTRIBUTING.md).

## Dokumentation

[Tastatur](docs/KEYBOARD.md) · [Workspace](docs/WORK_FOLDERS.md) ·
[Tokenanzeige](docs/TOKEN_USAGE.md) · [Konfiguration](docs/CONFIGURATION_BROWSER.md) ·
[Architektur](docs/ARCHITECTURE.md) · [Tests](docs/TESTING.md) ·
[Drittanbieter](THIRD_PARTY.md)

## Nutzungsrechte

Die Lizenz des eigenen Projektcodes ist noch nicht festgelegt. Das private Repo
ist zur Zusammenarbeit mit eingeladenen Kollegen bestimmt; eine öffentliche
Weiterverteilung erfordert eine ausdrückliche Lizenzentscheidung. Drittanbieter-
Bibliotheken behalten ihre jeweiligen Lizenzen.
