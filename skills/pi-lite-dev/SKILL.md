---
name: pi-lite-dev
description: Entwicklung und Pflege von pi-lite, einem Tauri-2-Desktopfrontend für pi auf Omarchy. Verwenden bei Änderungen an pi-lite, Installer, RPC, Chat, Sessions, Vorschauen, Tastaturbedienung oder Fortsetzung der bisherigen Entwicklung.
---

# pi-lite weiterentwickeln

Dieser Skill ist ein portables Markdown-Arbeitsprotokoll, kein ausführbarer Agent.
Er kann von jedem Coding-Agent gelesen und befolgt werden. Relative Projektpfade
beziehen sich auf das Repository, nicht auf den persönlichen Installationsort.

## Einstieg

1. Repository lokalisieren; `git status --short`, `git log -5 --oneline` prüfen.
2. README.md, CONTRIBUTING.md und docs/HANDOFF.md lesen.
3. Betroffene Fach-Dokumentation in docs/ lesen. Für pi-API-Fragen die Dokumentation
   der tatsächlich installierten pi-Version lesen, keine API erraten.
4. Gewünschte Änderung knapp planen, vorhandene Nutzeränderungen erhalten.

## Architektur und Prüfung

- Tauri 2/Rust in src-tauri; Vanilla-JS in ui; pi als JSONL-RPC-Kindprozess.
- UI wird eingebettet: jede UI-Änderung braucht Release-Build und Neuinstallation.
- `node --check ui/app.js`, `node scripts/test-history.cjs`,
  `node scripts/test-preview.cjs`, `cargo test --locked` in src-tauri.
- `cargo build --release --locked` in src-tauri, dann scripts/install.sh.
- Automatisierte Tests beweisen nicht die reale WebKit-Bedienung. Live-Teststatus
  ausdrücklich dokumentieren. Kein ferngesteuertes Tippen während Nutzerarbeit;
  vor Eingabesimulation aktive Fensterklasse prüfen. Keine fremden Fenster toggeln.

## Leitplanken

Kein Electron/Webserver. Keine ungefragten Änderungen an Hyprland-Konfiguration.
Keine privaten Sessions, Credentials, lokalen Providerdateien oder Screenshots im
Git. Keine automatische KI-Übertragung in Vorschauen: integrierte Zustimmung.
Modellidentität nicht aus CLAUDE.md oder Selbstbehauptungen ableiten. KI-Hilfsmodell
via PI_LITE_HELP_PROVIDER/PI_LITE_HELP_MODEL, lokale env.sh nicht committen.
History im RAM, explizites Speichern im Konfigeditor, Konfliktprüfung erhalten.

## Abschluss

Dokumentation und Tests aktualisieren, Änderungen verständlich committen. Remote
und Datenschutz vor Push prüfen; nur mit Nutzerauftrag veröffentlichen. README
Deutsch/Englisch konsistent halten. docs/HANDOFF.md mit Stand, offenen Prüfungen
und nächsten Schritten fortschreiben. Keine Lizenzentscheidung ohne Nutzerauftrag.
