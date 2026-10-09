# Tests

Automatisierte Struktur-/Logiktests (ohne echtes WebKit, ohne Netzwerk):

```bash
node --check ui/app.js
for t in scripts/test-*.cjs; do node "$t"; done
python3 scripts/test-media.py
(cd src-tauri && cargo test --locked)
```

Sie prüfen DOM-Struktur, CSS-Regeln, Hilfslogik und Rust-Funktionen, **nicht** die
reale Bedienung/Optik im WebKit-Fenster. Optionaler Optik-Vorabcheck: `ui/` nach
`/tmp` kopieren, `window.__TAURI__` per Stub-Skript ersetzen und mit
`chromium --headless=new --screenshot=… file:///…/index.html` rendern (Chromium ≠
WebKitGTK; ersetzt keinen Livetest). Die übrigen Ergebnisse unten wurden von Hand
gegen ein echtes `pi` geprüft.

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
| Pixel-Schlange, Bookmark-Filter, SVG-Session-Aktionen, flache Selects, Fokus-Overlay des Chatverlaufs (2026-10-09) | Strukturtests + Headless-Chromium-Screenshot; **WebKit live offen** |
| Material (Bilder/PDF), Medienanalyse, nativer Dateidialog | Strukturtests/Rust-Tests; **live mit Provider ungetestet** |
