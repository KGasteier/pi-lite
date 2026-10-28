# I1-S6-fix · Abschlussbericht

Stand: 28.10.2026 · Worktree `/home/medienhaus/Work/pi-lite-wt/s6`, Branch `v3/s6`, sauber. Kein Push/Merge, keine fremden Produktdateien geändert.

## Commits / Dateien

- `1487575` — baselinebezogene Format-/JS-Prüfung: `test/v3-gate.sh`, neu `test/gate-checks.py`, neu `test/js-baseline.txt`, `test/self-test.py`.
- `3ae1576` — sichtbares `KNOWN` und Formatzahlen, zusätzliche Selbsttestfälle; neu `test/BASELINE.md`, neu `coord/BUGS.md` (B1–B5).
- Vorhandener Checkpoint `a66f63a` bleibt erhalten; `test/no-local-spawn.sh`, `test/gate-metrics.py` und `test/clippy-baseline.json` unverändert.

## Ergebnis und Nachweis

- `python3 test/self-test.py`: **10 Tests grün**, Exit 0; `/tmp/I1-S6-fix-self-test.log`.
  - Format: echte Temp-Gitrepos, Merge-Base bei vorausliegender Basisreferenz, committierte und uncommittierte Änderungen, neue Datei mit Leerzeichen, gelöschte/unveränderte Datei, leere Auswahl, ungültiges `V3_GATE_BASE`.
  - JS: bekannt rot → KNOWN/Exit 0; neuer Fehler daneben → FAIL/Exit 1; bekannt jetzt grün → „aus Baseline streichen“; ungültige Baseline abgewiesen.
  - Optionale Mobil-/R2-Prüfungen mit Stubs: vorhanden/grün und fehlgeschlagen; fehlend → SKIP. Bestehende Lock-/Isolations-/Spawnhelfer-Tests weiterhin grün.
- `bash -n test/v3-gate.sh test/no-local-spawn.sh` und `git diff --check`: Exit 0.
- **Echtes Gate auf eigenem Worktree**, nach letztem Commit:
  `PATH=$HOME/.cargo/bin:$HOME/.local/share/mise/shims:$PATH CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s6 CARGO_BUILD_JOBS=3 bash test/v3-gate.sh "$PWD"`
  → **Exit 0**, `/tmp/I1-S6-fix-gate-final.log`, detaillierte Logs `/tmp/v3-gate.s6.yhe04r9h/`.
  - fmt PASS: 61 Rust-Dateien insgesamt, **0 ausgewählt**; Merge-Base `a66f63a`. Kein Rust-Produktcode dieses Auftrags geändert.
  - Clippy PASS: **24 unique / 41 roh**, entspricht Baseline; nicht warnungsfrei.
  - Rust-Tests PASS: **378 bestanden / 0 Fehler / 6 ignoriert**.
  - copi-check PASS; JS-Syntax PASS (15 Dateien).
  - JS-Tests **KNOWN**, 22 Skripte, nur `scripts/test-statusline-net.cjs` rot; keine neuen Fehler.
  - Mobil, R2 und Hub **SKIP**, auf diesem Worktree nicht vorhanden bzw. kein Hub-Worktree angegeben.
- Basiswerte auf `eb534bb` in `test/BASELINE.md`: ursprüngliche Basislogs mit HEAD-Beleg ausgewertet; Rust-Produktdateien des Worktrees unverändert gegen Basis. Ergänzende frische Einzeldateimessung aus `git show eb534bb:<pfad>` in Temp: **57/61** Rust-Dateien nicht formatkonform (`/tmp/I1-S6-fix-fmt-baseline.json`). Der alte `cargo fmt --check` aus `copi/` meldete **48** unterschiedliche Dateien; anderer Umfang, kein Widerspruch.

## Grenzen / offene Fragen

- Kein Sicherheits-, Remote-only-Prozess-, UI-, Plattform-, Persona- oder Pilotnachweis; keine Screenshots in diesem Auftrag. Gate-Grün bedeutet hier keine S2/S3-Produktabnahme.
- Optionaler echter Mock-Hub nicht erneut gestartet; Rust-Hubtest kann ohne separaten Mock früh zurückkehren. Keine echten Modellaufrufe oder Nutzerkonfigurationen verwendet.
- Die JS-Baseline ist wie beauftragt pfadbasiert: andere Fehler innerhalb desselben bekannten Skripts lassen sich so nicht unterscheiden. Testlog bleibt sichtbar; keine automatische Baseline-Erweiterung.
- B3–B5 sind ausschließlich Sammelverweise auf „Vor Pilot beheben“ des S3-Reviews. Andere Merge-/Vor-I2-Auflagen bleiben gültig; Fixstände anderer Rollen hier nicht unabhängig abgenommen.

## Änderungsbitten

- **K:** Commits übernehmen; `coord/BUGS.md` aus diesem Worktree integrieren. Anschließend Gate auf den Integrationskandidaten wiederholen, optional separaten Hub-Worktree angeben. Mobil/R2 werden dort automatisch ausgeführt, sofern vorhanden.
- **K/Test-/UI-Besitzer:** B1 (`NET_PLACEHOLDER`-Strukturtest) passend zum aktuellen Produkt korrigieren, dann aus `test/js-baseline.txt` entfernen. B2 nach Dateibesitz aufteilen; keine pauschale Neuformatierung durch S6 erfolgt.
- **S3/S4:** B3–B5 mit separaten Nachweisen vor Pilot bearbeiten; Details ausschließlich im referenzierten Sicherheitsreview.
