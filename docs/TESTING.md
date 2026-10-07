# Tests

Es gibt **keine automatisierten Tests**. Alles wurde von Hand gegen ein echtes `pi` geprüft
(echte Sessions in `~/.pi/agent/sessions`, echte Modellaufrufe).

## Testaufbau (Hyprland/Wayland)

- Fenster im **Vollbild** testen, nicht floatend: `hyprctl dispatch 'hl.dsp.window.fullscreen({ mode = "fullscreen" })'`
  (dieses System nutzt Lua-Dispatch; klassische `fullscreen 0`-Syntax funktionierte nicht).
- Fokus über Adresse setzen: `hyprctl dispatch "hl.dsp.focus({ window = \"address:$A\" })"`
  (`focuswindow class:…` war unzuverlässig).
- Eingabe mit `wtype`, Screenshots mit `grim -g "0,0 1440x904"`.
- **Vor jeder Eingabe die aktive Fensterklasse prüfen.** Mehrfach landete sonst Text im falschen Fenster
  (z. B. dem Agent-Terminal).
- Kein Maus-Tool vorhanden (ydotool, wlrctl, dotool) → nur Tastatur.
- Extension-Dialoge mit `examples/extensions/rpc-demo.ts` aus der pi-Installation:
  `PI_LITE_ARGS="-e …/rpc-demo.ts" ./pi-lite` (Befehle der Demo siehe deren Quelltext).

## Ergebnis

| Bereich | Stand |
|---|---|
| Streaming, Thinking, Tool-Blöcke, Senden | getestet |
| Abbrechen, `steer` | implementiert, nicht gezielt geprüft |
| Markdown inkl. **Tabelle**, Code-Highlighting | getestet |
| Sessionliste, Öffnen, „+ Neu“ | getestet |
| Sessionnamen (aus `session_info`) | getestet |
| **Modellwechsel**, **Thinking-Wechsel** | getestet (Wechsel steht als `model_change` / `thinking_level_change` in der Session-Datei) |
| Umbenennen | getestet |
| Fork | getestet |
| Löschen inkl. `confirm`-Dialog (Nein und Ja) | getestet |
| Dialoge `editor` und `confirm` (Löschen, Fork-Auswahl) | getestet |
| Dialoge `select`/`input` von Extensions, `notify`, Status, Widgets | implementiert, **nicht gezielt geprüft** (nur teilweise mit `rpc-demo.ts` gesehen) |
| Tastatur: `Strg+B`, Pfeile, `M`, `Backspace`, `Esc`, Fokusrückgabe | getestet |
| Kontextmenü per **Rechtsklick** | **ungetestet** (kein Maus-Tool; Menü per Tastatur geöffnet) |
| macOS, Windows | **ungetestet** (u. a. `Cmd+B`, `Backspace`, Pfade) |
| Lange Sessions, Bilder, Retries, pi-Absturz | ungetestet |
| `confirm` durch eine Extension (statt durch die App) | ungetestet |
| Launcher (`gtk-launch pi-lite`) | getestet, Prozess endet sauber |
