# VSCodium als angeschlossener Editor

pi-lite bleibt die Agentenoberfläche. VSCodium öffnet nur Dateien/Ordner, ohne einen
zweiten pi-Prozess oder PiLot zu starten. Voraussetzung: `vscodium-electron` im PATH
(die auf dieser Omarchy-Installation vorhandene Variante).

## Bedienung

- **↗ Editor** neben Workspace öffnet den ausgewählten Ordner in normalem VSCodium.
- In Assistentenausgaben erhalten erkannte lokale Dateiverweise in Inline-Code und
  Markdown-Links eine **↗ VSCodium**-Aktion. Relative Pfade beziehen sich auf den
  Arbeitsordner der laufenden Session, auch wenn bereits ein anderer Workspace für
  die nächste Session ausgewählt ist. Keine Erkennung beliebiger Fließtextpfade.
- Read/Write/Edit-Toolblöcke erhalten ebenfalls eine Dateiaktion, auch im geladenen
  Verlauf. Aufklappen zeigt sie unter dem Ergebnis.
- Der interne Datei-Editor bietet **↗ VSCodium**. Bei ungespeicherten Änderungen
  wird darauf hingewiesen: VSCodium öffnet die gespeicherte Datei, nicht den Entwurf.
  Der reine Prompt-Editor hat diese Aktion nicht.
- `ui/app.js:630:5` und `ui/app.js#L630` öffnen die betreffende Zeile.

Nur ein ausdrücklicher Klick startet den Editor. Pfade werden als Argumente ohne
Shell übergeben, müssen existieren und werden kanonisiert. Absolute Dateien
außerhalb des Workspaces sind erlaubt (z.B. Ressourcen aus ~/.pi). Fehlende Pfade
zeigen eine Fehlermeldung; keine Dateien werden automatisch angelegt. Weblinks
bleiben Weblinks. Es werden keine Inhalte automatisch an KI übertragen.

`--reuse-window` nutzt ein normales VSCodium-Fenster; die isolierte PiLot-
Vergleichsumgebung mit eigenem user-data-dir wird nicht angesprochen. Gleichzeitige
Bearbeitung derselben Datei sollte vermieden werden; der interne Speicherkonflikt-
Check bleibt bestehen.

## Teststand

Parser-Test: `node scripts/test-editor.cjs`; Rust-Test für Pfadauflösung (fehlende
Dateien, Leerzeichen, Optionszeichen und Shell-Metazeichen). Syntax, vorhandene
JS-Tests, Rust-Tests und Release-Build geprüft. Echte WebKit-Klicks, Editorstart
und Zeilensprung bleiben manuell zu prüfen. Keine Hyprland-Regel geändert:
Vollbild kann den Wechsel zwischen den beiden Fenstern verdecken.
