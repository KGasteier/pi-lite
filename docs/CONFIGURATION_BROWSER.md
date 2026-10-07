# Konfigurationsübersicht und Editor

Die Seitenleiste hat einen Ansichtswechsel: Sessions, Extensions/Plugins, Skills,
pi-Agent-/Kontextdateien und andere Harnesses. ☰ bleibt auch bei breiten Fenstern
sichtbar und blendet dort die Seitenleiste ein/aus; bei schmalen Fenstern öffnet es das Overlay.

## Entdeckung (kein Nachweis aktiver Ressourcen)

Gezielt durchsucht werden:
- pi-Agent-Verzeichnis (`PI_CODING_AGENT_DIR` oder `~/.pi/agent`), Arbeitsordner `.pi`
- `skills`, `extensions`, `agents`, `prompts`, Kontext-/System-MD-Dateien
- globale/project `settings.json`: Paketdeklarationen (nur Namen/Pfad; keine JSON-Inhalte)
- README-Dateien direkt installierter npm-Pakete
- `~/.agents/skills` und `.agents/skills` entlang der Arbeitsordner-Eltern
- AGENTS-/CLAUDE-Dateien entlang der Arbeitsordner-Eltern
- bekannte globale Verzeichnisse für Claude Code, OpenCode, OpenClaw, Codex, Gemini
- bekannte Projektverzeichnisse `.claude`, `.opencode`, `.codex`, `.gemini`

Es ist **keine vollständige Festplattensuche** und keine exakte Nachbildung der
pi-Ressourcenauflösung. Geladene, deaktivierte und nur entdeckte Ressourcen sind
noch nicht automatisch unterscheidbar. Benutzerdefinierte Skill-/Extension-Pfade,
CLI-Erweiterungen, Git-Pakete und alternative OpenClaw-Workspaces werden nicht vollständig erfasst.
Vorläufig sind AGENTS-/CLAUDE-Kontextdateien in der pi-Kontextansicht zusammengefasst;
ihre Herkunft ist am Pfad erkennbar, ihre tatsächliche Priorität wird nicht berechnet.

Traversal ist tiefen-/eintragsbegrenzt. Sessions, auth.json, Git, node_modules,
Build-Verzeichnisse und Claude-Historien werden nicht rekursiv durchsucht.

## Eingebauter Markdown-Editor

Ein Klick auf eine entdeckte Markdown-Datei öffnet einen Texteditor im Fenster.
Andere Dateitypen bleiben reine Übersicht. Der Editor startet schreibgeschützt: Ein
Schloss-Symbol unten rechts (🔒) sperrt Text und „Speichern“; erst nach Klick und
Bestätigung („wirklich bearbeiten?“) wird freigegeben (🔓). Das schützt Neulinge vor
versehentlichen Änderungen wichtiger Dateien. Speichern ausdrücklich per Knopf oder
Strg/Cmd+S; Esc schließt (mit Rückfrage bei ungespeicherten Änderungen).
Bearbeitung ändert weder automatisch laufende pi-Instruktionen noch andere Harnesses.

Backend prüft den Pfad erneut gegen das Inventar, beschränkt Lesen/Schreiben auf
Markdown bis 2 MB und verweigert Speichern, wenn der ursprüngliche Inhalt inzwischen
extern geändert wurde. Der Vergleich verhindert gewöhnliche Konflikte, ist aber kein
atomarer Dateilock. Es gibt derzeit keine automatische Sicherung vor dem Speichern.

## Prüfung

`cargo test` enthält Tests für Dateifilter, Ausschluss von Sessions/Zugangsdaten,
Lesen/Speichern und Abweisung eines externen Schreibkonflikts.
`node --check ui/app.js` und Release-Build erfolgreich.
Neue Ansichten, Klickweg, Fokussteuerung und Editor wurden noch nicht live geprüft.
