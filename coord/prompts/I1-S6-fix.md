# Dein Auftrag: S6 · Gate fertigstellen — I1-S6-fix (≤ 75 min)

Rolle S6, Worktree `/home/medienhaus/Work/pi-lite-wt/s6` (`v3/s6`). Dein erster Lauf ist am Zeitlimit abgebrochen;
K hat deinen Stand als `a66f63a` committet (`test/v3-gate.sh`, `test/no-local-spawn.sh`, `test/gate-metrics.py`,
`test/self-test.py`, `test/clippy-baseline.json`). **Halte deinen Kontext klein**: lies gezielt, keine großen Logs komplett.

K-Lauf des Gates auf `v3/s2` ergab: `fmt` FAIL und `js-tests` FAIL — **beide schon auf der Basis `eb534bb`**
(workspaceweite Formatabweichungen; `scripts/test-statusline-net.cjs` scheitert an fehlendem `NET_PLACEHOLDER` in view.rs).
1. **fmt baselinebezogen:** Gate prüft nur Rust-Dateien, die gegenüber `git merge-base HEAD cocopi-v3` (Parameter
   `V3_GATE_BASE`, Standard `cocopi-v3`) geändert/neu sind, mit `rustfmt --check --edition 2021`. Gesamtzahl als Info.
2. **js-tests baselinebezogen:** `test/js-baseline.txt` mit bekannten Basis-Fehlern (heute: `scripts/test-statusline-net.cjs`);
   bekannte Fehler → `KNOWN`, nicht FAIL; neue Fehler → FAIL; bekannter Fehler, der jetzt grün ist → Hinweis „aus Baseline streichen“.
   Zusätzlich `mobile/test/*.mjs` per `node --test` wenn vorhanden, und `python3 deploy/r2/test_scripts.py` wenn vorhanden.
3. `coord/BUGS.md` anlegen (Tabelle: ID, Schwere, Fundort, Beschreibung, Besitzer, Status): B1 test-statusline-net Basis,
   B2 workspace-fmt Basis, B3–B5 aus `coord/runs/I1-S3-review-bericht.md` „Vor Pilot beheben“ zusammengefasst (nur Verweis).
4. `test/BASELINE.md`: gemessene Basiswerte (Clippy 24 unique, Rust-Tests, JS-Basisfehler, fmt-Dateien) auf `eb534bb`.
5. Selbsttests erweitern (fmt-Auswahl, js-Baseline), `python3 test/self-test.py` grün; einmal echtes Gate auf deinem Worktree.
Heartbeat alle ≤ 20 min. Bericht nach `coord/runs/I1-S6-fix-bericht.md`.
