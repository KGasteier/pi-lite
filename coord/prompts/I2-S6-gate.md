# Dein Auftrag: S6 · I2-Gate und Angriffstests — I2-S6-gate (≤ 100 min)
Worktree `/home/medienhaus/Work/pi-lite-wt/s6`. Halte deinen Kontext klein.
1. `test/v3-gate.sh`: zusätzliche Schritte, jeweils SKIP wenn nicht vorhanden: `cargo test -p pi-sync`, pi-hub
   `node empfang/test/*.mjs` und `node test/sync.mjs` (Hub-Worktree-Argument darf mehrfach angegeben werden oder
   `V3_GATE_HUBS="a b"`), `python3 deploy/r2/test_scripts.py`.
2. `test/i2/` Szenario-Tests (Node, gegen gestartete Dienste aus pi-hub-Worktree, Fake-OIDC, fake-pi, Temp-Slots):
   Ende-zu-Ende Check-in → Upload Projekt → Vorschau → Archiv → Check-out → Slot-Fake „leer“; 20 parallele Check-ins;
   Sync-Angriffe (`../`, absolute Pfade, Symlink, 3 GiB-Header); Abholfach nur mit richtigem Schlüssel; Mitsteuern ohne
   Zustimmung blockiert (sobald S8 eine Schnittstelle hat, sonst als TODO markiert).
   Die Dienste kommen später von S3/S7: schreibe Tests gegen den Vertrag FREEZE-I2 und lasse sie SKIP melden, solange die
   Dateien fehlen (`/home/medienhaus/Work/pi-hub-wt/s7/empfang/empfang.mjs`).
3. `coord/BUGS.md` fortführen. Bericht `coord/runs/I2-S6-gate-bericht.md`.
