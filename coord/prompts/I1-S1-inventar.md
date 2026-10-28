# Dein Auftrag: S1 · Konzept, Synthese, Bedienbarkeit — I1-S1-inventar (≤ 90 min)

Rolle S1 (§5, §8). Worktree `/home/medienhaus/Work/pi-lite-wt/s1`, Branch `v3/s1`.
Du schreibst nur `docs/v3/` (KONZEPT.md, FLOWS.md, INVENTAR.md, ACCEPTANCE.md, ORCA-SYNTHESE.md).

Lies: Tutorial Kap. 1–6, 10–12, 14–17; `copi/app/REMOTE_MODE.md`; `docs/COCOPI_V2.md`; `docs/ROADMAP-COCOPI.md`;
die UI-Struktur von cocopi grob über `copi/app/src/view.rs`, `net_view.rs`, `coco_view.rs`, `material_ui.rs`
(Überschriften, Buttons, Panels; nicht jede Zeile); `ui/index.html` + `ui/app.js` grob für pi-lite;
Orca nur Doku/README + `src/main/pi/` Dateinamen in `/tmp/orca-review`.

Liefere (kurz, präzise, Deutsch, Tabellen):
1. `docs/v3/INVENTAR.md` — **A5-Entwurf**: alle heutigen Funktionen von cocopi-v2 (Desktop) und pi-lite-UI
   (Mobilbasis) mit heutigem Ort/Orten; je Funktion genau **ein** v3-Ort, Begründung, Spalte „Desktop/Mobil/weg“.
   Dubletten markieren und Streichung vorschlagen. Weglassen ist gültig.
2. `docs/v3/KONZEPT.md` — Abschnitt **A2 Moduslogik** (Zustandsdiagramm Remote-only ↔ Hybrid als Text/Mermaid,
   die *eine* Code-Stelle als Vorschlag mit Funktionssignatur in `pi-backend`, persistente Einstellung, Anzeige)
   und **A3 QR-Ablauf** (ein Ablauf für Login/Koppeln/Bestätigen: Inhalt des QR, Gültigkeit, Einmaligkeit,
   Geräte-Token je Person+Slot, Widerruf, Bildschirmtexte wörtlich). Dazu Skizze A1 (Nachrichtenliste, Zustände
   ruht/arbeitet/wartet auf dich/fertig/Fehler, Versionierung) und A4 (Slot↔Unix-Konto↔Hub-Instanz↔Gerät) als
   Vorschlag für S3/S4.
3. `docs/v3/FLOWS.md` — Nutzerflüsse Anmelden, Erstes Ergebnis, Dateien, Handy koppeln, Hybrid umschalten,
   Fehler: je Schritt eine Hauptaktion, sichtbarer Abschluss, Rückweg; ASCII-Skizze je Hauptansicht.
4. `docs/v3/ACCEPTANCE.md` — Akzeptanzkriterien I1 (T1, T2, T3, A1–A5) prüfbar formuliert (Testname-Vorschlag).
5. `docs/v3/ORCA-SYNTHESE.md` — je Baustein aus Prompt §4.3: übernehmen/nachbauen/weglassen + Begründung
   + Orca-Datei als Beleg.

Prüffrage aus §3 bei jeder Entscheidung. Checkpoint-Commit nach jeder Datei. Bericht nach `coord/runs/I1-S1-inventar-bericht.md`.
