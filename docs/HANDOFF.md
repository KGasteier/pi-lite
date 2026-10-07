# Übergabe: pi-lite

## Aktueller Stand

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
