# Todo-Plan: Natives Rust-Frontend (Evaluierung)

Status: **zurückgestellt / nur Evaluierung**. Keine Portierung beschlossen.
Stand der Bewertung: 2026-10-08.

## Ausgangslage (gemessen)

- Frontend: ca. 2.060 Zeilen JS/HTML/CSS + Vendor (marked, highlight.js, DOMPurify, 216 KB)
- Rust-Backend: ca. 860 Zeilen; Release-Binary 4,9 MB (nutzt System-WebKitGTK)
- RAM-Momentaufnahme (RSS, mehrere Instanzen/Prozesse): pi-lite 136–364 MB,
  WebKitWebProcess ca. 202 MB, WebKitNetworkProcess ca. 46 MB,
  pi-Agent (Node) je 80–155 MB. **pi bleibt bei jeder UI-Variante unverändert.**

## Bewertung

Machbar, aber großer Umbau. Erwarteter Nutzen vor allem RAM/Startzeit, nicht
Bedienqualität. Empfehlung: derzeit nicht portieren, da Funktionen schnell wachsen
und Web-UI dafür am produktivsten ist.

Geschätzte Toolkit-Optionen (Binary / UI-RAM, **nicht gemessen**):

| Toolkit | Binary | UI-RAM | Bemerkung |
|---|---|---|---|
| GTK4 (gtk4-rs/relm4) | 3–8 MB | 40–90 MB | System-GTK, natives Wayland, beste Text-/A11y-Basis |
| iced | 10–20 MB | 50–120 MB | Elm-Architektur; Textauswahl/Markdown begrenzt |
| egui | 8–15 MB | 50–100 MB | schnell gebaut; schwache Textauswahl/Formatierung |
| Slint | 5–10 MB | 30–70 MB | Lizenzmodell vorab prüfen |

Erwartung: ca. 150–300 MB weniger Gesamt-RAM, schnellerer Kaltstart (Schätzung).

Vorteile: kein WebKit-Prozess, keine WebKitGTK-Eigenheiten (Super-Taste, Fokus,
Hover), durchgehend Rust ohne JS-Brücke, nativeres Omarchy/Wayland-Verhalten.

Einschränkungen/Risiken: Markdown-Chat (Tabellen, Code-Highlighting, Links,
Textauswahl über Antworten) aufwändig mit pulldown-cmark/syntect; Streaming langer
Chats braucht Virtualisierung; langsamere Styling-Iteration; alle Popups, Slash-
Liste, Wizards, Vorschauen, Editor, Material-Chips, Kontextmenüs, Tastatur und
Toasts neu; bestehende Node/DOM-Tests entfallen; neue Toolkit-Eigenheiten (IME,
Barrierefreiheit, Fokus).

Aufwand (Schätzung): 3–6 Wochen konzentriert mit Agentenunterstützung bis
Funktionsgleichheit, plus Live-Test/Feinschliff. Risiko mittel bis hoch.

## Todos

- [ ] **Zuerst:** Speicherverbrauch der heutigen WebKit-App analysieren (auffällige
      bis 364 MB im pi-lite-Prozess): Instanzen zählen, Chatlänge, Vorschau-Caches,
      Bilder/Base64 in Material, WebKit-Cacheeinstellungen. Reproduzierbar messen.
- [ ] Messprotokoll festlegen: Kaltstart, RSS/PSS (`smem`/`/proc/*/smaps_rollup`)
      nach Start, nach langem Chat, im Leerlauf; gleiche Session für alle Varianten.
- [ ] Nur falls RAM/Start weiterhin relevant: 1–2-Tage-Prototyp, bevorzugt GTK4/relm4,
      Umfang: pi-RPC-Streaming, Markdown-Antworten mit Codeblöcken, Eingabe, Sessionliste.
- [ ] Prototyp gegen Messprotokoll vergleichen; Textauswahl, IME, Tastatur, Fokus,
      Scrollen langer Chats und Screenreader live prüfen.
- [ ] Go/No-Go-Entscheidung dokumentieren. Bei Go: schrittweiser Plan, Web-Frontend
      bleibt bis Funktionsgleichheit Hauptversion; kein Big-Bang-Wechsel.

## Modellwahl für die Arbeit (Empfehlung)

- Architektur, Portierungsplan, schwierige Teile (Markdown-Rendering, Virtualisierung,
  Toolkit-Entscheidung, Reviews): stärkstes verfügbares Claude-Opus-Modell.
- Gleichförmige Einzelumsetzungen (Dialoge, Listen, Knöpfe): aktuelles Claude-Sonnet.
- Kombination: Opus plant/prüft, Sonnet setzt um. Tatsächliche Verfügbarkeit in der
  pi-lite-Modellauswahl prüfen; Modellnamen hier nicht verifiziert.
