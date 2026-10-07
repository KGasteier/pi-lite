# Architektur

```
┌────────────── pi-lite (Tauri 2, ein Prozess) ───────────────┐
│  Webview (System)            Rust-Backend (src-tauri)       │
│  ui/index.html, app.js  ⇄    start_pi / send / list_…       │
│                              │ stdin  ▲ stdout (JSONL)      │
└──────────────────────────────┼────────┼─────────────────────┘
                               ▼        │
                         pi --mode rpc  (Kindprozess)
```

## Grundidee

pi-ui (hyperpuncher) ist ein Bun-Binary mit HTTP-Server, das im Browser läuft. pi-lite ersetzt
Server und Browser durch ein natives Fenster. Als Schnittstelle dient der RPC-Modus von pi
(`pi --mode rpc`): eine JSON-Zeile pro Befehl auf stdin, eine JSON-Zeile pro Event/Antwort auf stdout.
Doku: `docs/rpc.md`, `rpc-commands.md`, `rpc-extension-ui.md`, `session-format.md` der pi-Installation.

## Backend (`src-tauri/src/main.rs`, ~180 Zeilen)

| Befehl | Aufgabe |
|---|---|
| `start_pi(cwd, args)` | Startet `pi --mode rpc [args] [PI_LITE_ARGS]` im Ordner `cwd`. stdout-Zeilen gehen unverändert als Event `pi-line` ans Frontend, stderr als `pi-stderr`. |
| `send(line)` | Schreibt eine JSON-Zeile an pi. |
| `list_sessions()` | Liest `~/.pi/agent/sessions/*/*.jsonl` (bzw. `PI_CODING_AGENT_*`), liefert Pfad, cwd, Titel, Name, mtime; max. 200, neueste zuerst. Cache nach Pfad+mtime. |
| `delete_session(path)` | Löscht eine Session-Datei. Prüft: kanonisierter Pfad liegt unter dem Sessions-Root und endet auf `.jsonl`. |

Wichtige Details:

- **Zeilentrennung nur an LF.** Die RPC-Doku verlangt das (kein Split an U+2028/2029).
- **Generationszähler (`Pi.generation`)**: Beim Neustart wird der Zähler erhöht, *bevor* der alte Prozess
  beendet wird. Der Lese-Thread des alten Prozesses meldet `pi-exit` nur, wenn seine Nummer noch aktuell ist.
  Ohne das erschien nach „+ Neu“ oder einem Fork fälschlich „pi wurde beendet“.
- **Sessionnamen** stehen als `session_info`-Zeilen irgendwo in der Datei (letzte gewinnt). Deshalb wird
  die Datei komplett gelesen, aber nur die Kopfzeile, die erste Nutzernachricht und `session_info` geparst.
- Beim App-Ende (`RunEvent::Exit`) wird pi beendet.

## Frontend (`ui/`)

Kein Framework, kein Bundler. `withGlobalTauri` stellt `window.__TAURI__` bereit; `app.js` nutzt nur
`invoke` und `listen`. Der Code ist in Abschnitte gegliedert (Rendering, Event-Handler `handle`,
Extension-Dialoge, Modelle, Sessions, Start).

- **Streaming:** `message_update`-Events füllen einen Textblock; Markdown wird auf 80 ms gedrosselt
  gerendert (`marked` → `DOMPurify` → `highlight.js`).
- **Tool-Aufrufe** erscheinen als aufklappbare Blöcke (`tool_execution_*`).
- **Request/Response:** `request(obj)` hängt eine `id` an und wartet auf die passende `type:"response"`-Zeile.
- **Extension-Dialoge:** `extension_ui_request` mit `select|confirm|input|editor` wird in eine Warteschlange
  gestellt und als Overlay (`#modal`) gezeigt; Antwort als `extension_ui_response` mit gleicher `id`.
  `notify`, `setStatus`, `setWidget`, `setTitle`, `set_editor_text` sind fire-and-forget.
  `ask(ev)` nutzt dieselbe Warteschlange für lokale Dialoge (Umbenennen, Löschen, Fork-Auswahl).
- **Fokus:** Nach einem Dialog kehrt der Fokus zum vorherigen Element zurück (`returnFocus`).
- **Layout:** Unter 860 px wird die Seitenleiste zum Overlay (☰). Hintergrund: Tiling-WMs wie Hyprland
  können Fenster sehr klein machen (709×426 gemessen).

## Sessions und Befehle

| Funktion | pi-RPC |
|---|---|
| Neue Session | Neustart von pi ohne `--session` |
| Session öffnen | Neustart mit `--session <pfad>`, dann `get_messages` (cwd der Session wird übernommen) |
| Umbenennen | `set_session_name` (nur aktive Session) |
| Fork | `get_fork_messages` → Auswahl → `fork {entryId}` → `get_messages` |
| Modell / Thinking | `get_available_models`, `set_model`, `set_thinking_level` |
| Senden / Abbrechen | `prompt` bzw. `steer` (wenn pi arbeitet) / `abort` |

## Sicherheit

- DOMPurify bereinigt gerendertes Markdown.
- `delete_session` akzeptiert nur `.jsonl` unterhalb des Sessions-Ordners.
- Capabilities: nur `core:default`; kein Dateisystem- oder Shell-Plugin.
- `PI_LITE_ARGS` wird von der Umgebung gelesen, nicht vom Frontend – das Frontend kann es nicht setzen.
  (`start_pi(args)` akzeptiert aber `--session`-Pfade aus dem Frontend; das Frontend ist eigener, nicht fremder Code.)

## Entscheidungen

| Entscheidung | Begründung |
|---|---|
| Tauri 2 statt Electron | ~3,7 MB statt >100 MB; System-Webview |
| Kein Frontend-Build | klein, anpassbar, kein Node nötig; Nachteil: UI ist im Binary eingebettet, jede Änderung braucht `cargo build` |
| `pi --mode rpc` statt SDK | gleiche Sprache-unabhängige Schnittstelle, kein Node-Embedding |
| Umbenennen nur aktiv, Löschen nur inaktiv | Umbenennen ist ein RPC-Befehl der laufenden Session; die aktive Datei wird von pi gehalten |
| Eigenes Overlay statt `<dialog>` | Warteschlange, Fokusrückgabe und Tastatursteuerung kontrollierbar |
| Responsives Layout statt WM-Regel | Tiling ist gewolltes Verhalten; der Nutzer-Hyprland bleibt unangetastet |
