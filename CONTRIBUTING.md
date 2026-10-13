# Zusammenarbeit

Feature-Branch erstellen, Änderungen testen und per Pull Request nach main.
Neue UI-Funktionen mit Tastatur und Maus prüfen; ungeprüfte Plattformen/Wege klar
benennen. Keine echten Sessions für Löschtests verwenden. Keine Credentials,
privaten Chatlogs, lokalen Einstellungen oder Screenshots ins Repository übernehmen.

## Feature-Parität (verbindlich)

copi und pi-lite bleiben funktional gleichwertig. Jede Änderung oder Erweiterung
muss in beiden Oberflächen umgesetzt und getrennt geprüft werden. Bestehende
Abweichungen werden als offene Aufgaben dokumentiert, nicht stillschweigend als
fertige Parität bezeichnet. Laufende Sessions bleiben bei Installation erhalten.

## Checks

```bash
node --check ui/app.js
node scripts/test-history.cjs
node scripts/test-preview.cjs
bash -n scripts/install.sh scripts/setup-omarchy.sh
(cd src-tauri && cargo test --locked && cargo build --release --locked)
```

`./scripts/install.sh` installiert den gebauten Stand. Installeränderungen auf einem
sauberen System oder einer VM prüfen; keine sudo-Passwörter in Skripten speichern.
UI und Dokumentation konsistent aktualisieren. Der derzeitige Installer unterstützt
Omarchy/Arch; weitere Plattformen benötigen eigene verifizierte Setup-Pfade.

Die GitHub-Historie beginnt mit einem bereinigten Snapshot; die vorherige lokale
Entwicklungshistorie wurde aus Datenschutzgründen nicht veröffentlicht.
