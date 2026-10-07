# Roadmap / bekannte Lücken

## Lücken
- Keine Baumansicht der Session (Verzweigungen), `clone` fehlt
- Keine Bilder (Einfügen, Anzeigen)
- Ordner-Dialog fehlt (Arbeitsordner nur als Textfeld)
- Sessionliste lädt max. 200 Einträge; bei sehr vielen großen Dateien dauert der erste Aufruf
- Kein Retry-/Kompaktierungs-Status, kein Verhalten bei pi-Absturz außer Hinweis „pi wurde beendet“
- Themes: nur dunkel; RPC unterstützt Theme-Wechsel von Extensions nicht
- `custom()`-Dialoge von Extensions sind in RPC nicht verfügbar
- Fenstergröße/-position wird nicht gespeichert
- Alle Texte sind deutsch; keine i18n

## Ideen
- Automatisierte Tests: Rust-Unit-Tests für `read_info`/`delete_session`; Smoke-Test mit Mock-`pi`
- Plattformbuilds über GitHub Actions (Linux, macOS, Windows), AppImage/.dmg/.msi
- Suche in der Sessionliste
- Einstellungen (Schriftgröße, Standard-Arbeitsordner) als kleine JSON-Datei
