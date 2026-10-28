# Dein Auftrag: S6 · Debugging, Tests — I1-S6-gates (≤ 100 min)

Rolle S6 (§5, §8). Worktree `/home/medienhaus/Work/pi-lite-wt/s6`, Branch `v3/s6`.
Du schreibst nur `test/` (neu, Repo-Root), `copi/app/src/tests.rs` (nur wenn nötig) und
`/home/medienhaus/Work/pi-lite/coord/BUGS.md` (direkte Datei, nicht committen — K committet sie).

Liefere:
1. `test/v3-gate.sh` — das Merge-Gate, das K vor jedem Merge in `cocopi-v3` auf einem Worktree ausführt:
   `cargo fmt --check` (Workspace copi), `cargo clippy --offline --workspace --features copi/cocopi` (Warnungen
   zählen, nicht abbrechen — Vergleich gegen Baseline-Zahl), `cargo test --offline --locked --workspace --features copi/cocopi`,
   `cargo check --offline -p copi --bin copi` (ohne Feature), Node-Tests aus Repo-Root
   (`node --check ui/*.js`, `for f in scripts/test-*.cjs`), optional pi-hub-Tests wenn Pfad als Argument
   (`node test/smoke.mjs` im Hub-Worktree, nur Mock). Ausgabe: eine Zusammenfassungszeile je Gate
   (OK/FEHLER/Anzahl), Exitcode ≠ 0 bei Fehler. Lange Ausgaben in Logdatei, nur Zusammenfassung auf stdout.
   Aufruf: `test/v3-gate.sh <worktree> [hub-worktree]`. Eigenes CARGO_TARGET_DIR je Worktree-Name.
2. `test/no-local-spawn.sh` — Prozessprüfung „kein lokaler Spawn im Remote-only“: startet die zu prüfende
   cocopi-Binary headless (`COPI_SCENE=… <bin> --shot /tmp/x.png`) bzw. später eine Remote-only-Szene, unter
   einem PATH, in dem `pi` und `node` Attrappen sind, die bei Aufruf eine Markerdatei schreiben; Prüfung: Marker
   existiert nicht. (Heute wird es für die Normal-Szene vermutlich *fehlschlagen* oder nichts beweisen — dokumentiere
   ehrlich, welche Szene/Variable nach T1 nötig ist; das Skript soll dann ohne Änderung nutzbar sein, Szene als Argument.)
   Zusätzlich ein Rust-Test-Vorschlag (Text im Bericht) für die zentrale Spawn-Sperre, den S3 umsetzt.
3. **Baseline** auf dem unveränderten Stand (`eb534bb`, dein Worktree): Gate einmal laufen lassen, Ergebnis
   (Testzahlen, Clippy-Warnungsanzahl, fmt-Abweichungen) in `test/BASELINE.md`. Vorhandene Fehler sind nicht
   deine Aufgabe zu beheben, sondern als Einträge in `coord/BUGS.md` (Schwere hoch/mittel/niedrig, Reproduktion,
   vorgeschlagener Besitzer S2/S3/S5).
4. `coord/BUGS.md` anlegen (Kopf: Tabelle ID | Schwere | Kurz | Reproduktion | Besitzer | Status).

Hinweis Ressourcen: R2 hat 12 Kerne, andere Agenten bauen parallel: `CARGO_BUILD_JOBS=3`.
Checkpoint-Commit nach jedem grünen Teil. Bericht nach `coord/runs/I1-S6-gates-bericht.md`.
