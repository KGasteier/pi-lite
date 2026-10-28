# Bericht I1-S1-inventar (S1 · Konzept, Synthese, Bedienbarkeit)

- **Worktree:** `/home/medienhaus/Work/pi-lite-wt/s1`, Branch `v3/s1`, Basis `eb534bb`.
- **Zeit:** 2026-10-28, 18:07 bis ca. 18:30 laut Heartbeat.
- **Grenzen:** Kein Push und kein Merge. Fremde Dateien wurden nicht geändert. Code wurde weder gebaut noch ausgeführt, weil der Auftrag reine Dokumentation war.

## Commits

| Commit | Inhalt |
|---|---|
| `8bd5afa` | `docs/v3/INVENTAR.md` (A5-Entwurf) |
| `0103ef3` | `docs/v3/KONZEPT.md` (A2, A3, Skizzen A1/A4) |
| `7b23e1e` | `docs/v3/FLOWS.md` (6 Flüsse, ASCII-Skizzen) |
| `8dfc1ff` | `docs/v3/ACCEPTANCE.md` (T1–T3, A1–A5) |
| `b9edd48` | `docs/v3/ORCA-SYNTHESE.md`; dazu QR-Fragment-Risiko in KONZEPT und ACCEPTANCE |
| `fdbde9f` | INVENTAR: Zählungen korrigiert |

`git status` ist danach sauber. Gestaged wurden nur Pfade unter `docs/v3/`.

## Ergebnisse (kurz)

### INVENTAR.md
- Feste Ortsliste: 9 Desktop-Orte und 5 Mobil-Orte.
- 60 Desktop-Funktionen: 14 als Dublette gestrichen, 21 „weg“ (Rest hat genau einen Ort).
- 23 pi-lite-Gruppen, davon bleiben 7 mobil.
- Abschnitt „Neu in v3“ und eine Streichliste.

### KONZEPT.md
- **A2:**
  - Mermaid-Diagramm mit zwei Zuständen, Standard RemoteOnly, fail closed.
  - **Eine Code-Stelle:** `pi_backend::winshim::pi_command(purpose, args) -> Result<Command, SpawnBlocked>`, dazu `pi_backend::mode::{current, init, set}`. Persistenz in `<copi_config_dir>/mode.json`.
  - Anzeige und Dialogtexte wörtlich.
- **A3:**
  - Ein Code-Format für Einladung, Koppeln und Bestätigen.
  - Gültigkeit: Einladung 24 h, Koppeln 5 min, Bestätigen 2 min.
  - Einmalig per atomarem Compare-and-set; nur der Hash wird gespeichert.
  - Ed25519-Gerätschlüssel je Slot. Widerruf an einer Stelle.
  - Alle Bildschirmtexte wörtlich.
- **A1-Skizze:** Versionierung, Kernnachrichten, `puls` und eine Zustandstabelle, die auf die heutigen Hub-Zustände abgebildet ist.
- **A4-Skizze:** Baum Person → Slot → Unix-Konto → Slice → Hub → Geräte, mit einer Bindungstabelle.

### FLOWS.md
- Die Flüsse F1–F6 als Tabellen, je Schritt mit Hauptaktion, Abschluss und Rückweg.
- 5 ASCII-Skizzen: Start, Arbeiten, Dateien, Geräte, Mobil.

### ACCEPTANCE.md
- 50 prüfbare Kriterien mit Testnamen-Vorschlag, dazu G1 (Tests, Clippy, Format) und G2 (keine Webview-Abhängigkeit).
- Kern von T1: der Test mit dem Fallen-Skript `pi` im PATH (`start_without_pi_in_path_spawns_nothing`) und `only_winshim_spawns_pi`.

### ORCA-SYNTHESE.md
- Die 8 Bausteine aus §4.3 (Mobil in 6a–6e aufgeteilt): je Baustein eine Entscheidung mit Orca-Datei als Beleg.
- Ergebnis: Kein Baustein wird wörtlich übernommen. Alles wird **nachgebaut**, außer Worktree (I1–I2 weg, Idee „Variante“ als Ordnerkopie für I3) und Telemetrie (weg).

## Nachweise (geprüft)

- **Spawn-Stellen:**
  - `grep -rn pi_command copi/` findet genau 4 Aufrufer: `rpc.rs:48`, `cli.rs:53`, `cli.rs:189`, `preview.rs:221`.
  - `Command::new("pi")` steht nur in `winshim.rs`.
  - Die übrigen `Command::new` (ssh, rsync, curl, open/xdg-open, Editor, pdftotext, Piper, taskkill/kill) starten kein pi.
- **Lokaler Fallback** belegt in `app/src/net.rs:588` (`vec![BackendCfg::local()]` bei Ladefehler) und `crates/pi-backend/src/backends.rs:83/89/92` (lokales Backend wird automatisch eingefügt).
- **pi-lite-UI:**
  - Alle 43 zitierten `ui/index.html`-IDs existieren, je einmal (per Schleife geprüft).
  - 33 verschiedene `invoke`-Befehle. Der Prompt sagt „rund 35“, das ist eine Abweichung, kein Fehler.
  - `send` trägt auch `extension_ui_response` und `abort` (`ui/app.js:319` und `:1016`).
- **Hub-Zustände** `starting/idle/busy/waiting/exited`: `pi-hub/server.mjs:81–125`.
- **Orca-Belege:**
  - Jede in ORCA-SYNTHESE zitierte Datei existiert in `/tmp/orca-review` @ `05aab3a185`.
  - Konstanten wörtlich gelesen: `MISSED_PROBE_LIMIT = 3`, `agentHibernationIdleMs: 30*60*1000`, `MOBILE_PROTOCOL_VERSION = 3`, `orca://pair?code=`.
  - Lizenz: MIT, Lovecast Inc.

## Ungeprüft / Annahmen

- **Desktop:** Keine Screenshots der heutigen Oberfläche erstellt, also kein `copi --shot`. Grundlage war allein der Quelltext. Ob alle Inventarzeilen sichtbar so wirken, wie beschrieben, ist ungeprüft.
- **Kürzel:** Strg+B, Alt+2 und Strg+Shift+B stammen aus Tooltips bzw. der Zusammenfassung und wurden nicht im Code verifiziert. R/F2 ist belegt (`view.rs:518`).
- **Konzeptwerte** sind Vorschläge ohne Prototyp:
  - 15 s Puls, 45 s bis „unterbrochen“, Gültigkeiten der QR-Codes;
  - Ed25519, Hub-Port `8700+NN`, `flock` für die CPU-Queue.
- Die Orca-Repo-URL für `THIRD_PARTY.md` ist nicht ermittelt.
- Die Testnamen in ACCEPTANCE sind **Vorschläge**. Keiner davon existiert bisher.

## Offene Fragen (an K bzw. Koordination)

1. **F4 Lehrenden-Sicht (Persona Aylin):** Heute leisten das Raster und der Fernzugriff diese Sicht, beide fallen weg. Vorschlag: ein Admin-Prüfskript (S4) statt einer App-Funktion. Das braucht eine Entscheidung.
2. **F3 „Neue Session“ mobil:** §4.2 verbietet nur „Projekte anlegen“. Vorschlag: in I1 weglassen und in I2 entscheiden.
3. **F1 Modellwahl für Lernende:** Soll sie sichtbar sein, begrenzt auf die Freigabeliste des Slots?
4. **QR-Code in Fragment oder Query:** Orca nutzt wegen Android-Kamera-Intents die Query. S5 soll das mit Tauri-Mobile prüfen. Bei Query gilt: Der Hub darf den Code nicht loggen (Test `qr_code_never_logged`).
5. **Zustand „bereit“:** Gemeint ist „fertig, schon angesehen“. Das ist ein sechster Zustandswert neben den fünf aus §4.3, aber ohne eigene Markierung. Bitte bestätigen.

## Änderungsbitten (fremde Dateien, nicht geändert)

- **An S2 (Desktop):**
  - E4-Sperre in `copi/crates/pi-backend/src/winshim.rs` nach KONZEPT A2 umsetzen; neues Modul `mode.rs`.
  - `copi/app/src/net.rs:588`: kein Rückfall auf `BackendCfg::local()`.
  - `backends.rs:83/89/92`: bei RemoteOnly kein lokaler Eintrag.
  - Startkarte (`view.rs` `start_card`): pi-Version und „pi update“ entfernen (INVENTAR #26). Dasselbe gilt für das KI-Hilfsmodell (#12–14, #41) und die pi-CLI-Befehle (#34).
  - Dublette `net_bar_inner` (Backends-Liste in Leiste **und** in `coco_panel`) entfällt mit INVENTAR #45–49.
  - `THIRD_PARTY.md` mit dem Orca-Eintrag anlegen (Text in ORCA-SYNTHESE, §Lizenz).
- **An S3 (Hub):**
  - Bei der Ausarbeitung von A1 (`shared/protocol/v1`) die Skizze in KONZEPT A1 als Ausgangspunkt nehmen.
  - Einlöse-Endpunkt nach A3.
  - Zustandsberechnung „ruht“ und Hibernation nach ORCA-SYNTHESE #3.
- **An S4 (R2):** Bitte das Kontomodell (Skizze A4) gegenprüfen: Port-Schema, Slice-Grenzen, CPU-Lock. Das Prüfskript soll die Probe enthalten, ob die Grenzen greifen (Orca-Muster).
- **An S5 (Mobil):**
  - Mobil-Ortsliste und Liste der verbleibenden `invoke`-Befehle prüfen (INVENTAR §2).
  - Fragment-Frage beim QR-Code klären.
  - M-SE bleibt in I1 read-only.
