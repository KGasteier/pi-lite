# Merge-Gate: gemessene Basis

## Bezug und Nachweise

Basis ist `eb534bb328c246726fb43c56c8f3955ee89d55bb`, nicht die sich weiterbewegende Integrationsbranch-Spitze. Messungen vom 28.10.2026 unter Linux x86_64, Rust/Cargo 1.99.0 und Node 22.22.1. Der S6-Worktree enthält gegenüber dieser Basis keine Änderungen unter `copi/`, `shared/`, `src-tauri/`, `ui/` oder `scripts/`.

Die ursprünglichen Basisläufe sind durch `context.log` belegt: `/tmp/v3-gate.s6.3hn3BuQK/` und `/tmp/v3-gate.s6.oXhVNEmg/` nennen HEAD `eb534bb` sowie ausschließlich untracked `test/`. Logs sind lokale, flüchtige Nachweise, keine Lieferdateien.

| Prüfung | Basiswert | Befehl / Nachweis |
|---|---|---|
| Clippy | **24 eindeutige**, 41 rohe Warnungen; Exit 0 | `cargo clippy --offline --locked --workspace --features copi/cocopi --message-format=json` aus `copi/`; `clippy.log`, `clippy-metrics.json` der Basisläufe; Definition in `test/clippy-baseline.json`. Gleiche Diagnosen der beiden include-Binaries werden dedupliziert. |
| Rust-Tests | **378 bestanden, 0 fehlgeschlagen, 6 ignoriert**, Exit 0 | `cargo test --offline --locked --workspace --features copi/cocopi` aus `copi/`; `rust-tests.log` der Basisläufe. 104 App-Tests laufen je Binary, daher doppelt in der Summe. |
| JS-Tests | **22 Testskripte: 21 grün, 1 bekannter Fehler** | `node scripts/test-*.cjs` einzeln aus Root; `js-tests.log` der Basisläufe. `scripts/test-statusline-net.cjs` scheitert beim Zugriff auf den fehlenden Match `NET_PLACEHOLDER` in `view.rs`. |
| JS-Syntax | **15 Dateien**, Exit 0 | `node --check ui/*.js` einzeln; `js-syntax.log` des Basislaufs. |
| Rust-Dateien (ganzes Repository) | **61 insgesamt; 57 nicht formatkonform** | Einzeldateimessung aus unveränderten `git show eb534bb:<pfad>`-Inhalten in einem Temp-Verzeichnis: pro Datei `rustfmt --check --edition 2021 --config skip_children=true -- <pfad>`. Ergebnis `/tmp/I1-S6-fix-fmt-baseline.json`. |
| Alter Workspace-Formatcheck | **48 unterschiedliche Dateien mit Diffs**, Exit 1 | `cargo fmt --check` aus `copi/`; `fmt.log` der Basisläufe. Anderer Umfang als die 61 Repository-Dateien: ohne Tauri, mit geladenen Modulen. |

Testaufteilung: App `104 + 104`, md-core `12`, md-egui `4`, pi-backend `82`, Hub-Integration `1`, SSH-Integration `3`, pi-net `68`; ignoriert `2 + 2` in den App-Binaries sowie `2` Live-SSH-Tests. Ein bestandener Hub-Integrationstest kann bei fehlendem isoliertem Mock-Hub früh zurückkehren; die Zahl allein belegt keinen Live-Hub. Keine echten Modelle wurden aufgerufen.

## Verhalten des Gates

Aufruf: `bash test/v3-gate.sh <worktree> [hub-worktree]`.

- `fmt`: Basis ist `git merge-base HEAD "${V3_GATE_BASE:-cocopi-v3}"`. Geprüft werden nur gegenüber dieser Basis geänderte/neue vorhandene `*.rs`, einschließlich uncommitteter und nicht ignorierter untracked Dateien. Gelöschte Dateien entfallen. `skip_children=true` verhindert das Nachladen unveränderter Untermodule. Gesamtzahl und Auswahlzahl sind Info, nicht Erfolgsaussage für den gesamten Formatbestand. Fehlende Basisreferenz schlägt fehl.
- `js-tests`: Jeder Test läuft weiterhin. Bekannter Fehlschlag → `KNOWN`, Exit 0, sofern kein neuer Fehler vorliegt. Neue Fehlschläge → `FAIL`. Ein wieder grüner oder entfernter Baseline-Test erzeugt den Hinweis „aus Baseline streichen“. Baseline wird nicht automatisch erweitert. Ein anderer Fehler im selben bekannten Testskript ist mit dieser pfadbasierten Baseline nicht unterscheidbar; Logs prüfen.
- `mobile/test/*.mjs`: sofern vorhanden gemeinsam mit `node --test`; keine Baseline-Ausnahme. Für den ausgewählten ersten Hub wird `COCOPI_V1_TEST_MODULE=<hub>/lib/hub-v1.mjs` explizit gesetzt, damit isoliertes HOME den S3-Pooltest nicht unbemerkt überspringen lässt. TAP-Skips werden als SKIP statt PASS ausgegeben.
- `deploy/r2/test_scripts.py`: sofern vorhanden mit `python3`; keine Baseline-Ausnahme. Fehlende optionale Tests heißen `SKIP`, nicht `PASS`.
- Clippy sperrt bei mehr als 24 eindeutigen Warnungen. Die 24 historischen Diagnosen wurden für I3-O1 aus **beiden** erhaltenen Basislogs (`3hn3BuQK`, `oXhVNEmg`, je 41 rohe/24 eindeutige Warnungen, HEAD `eb534bb`) rekonstruiert und in `clippy-baseline.json` ergänzt; Grenzwert und Bezug bleiben unverändert. Vergleich als Multimenge aus Meldung, Code, Primärdatei und Quelltextanker (ohne Zeilennummern): bloße Zeilenverschiebung ist nicht neu, neue gleiche Stellen verbrauchen je einen Baseline-Eintrag. Geänderter Quelltextanker kann eine bestehende Warnung als neu ausweisen. `clippy-new-warnings.log` und der Schluss von `clippy.log` nennen neue Warnungen als `Datei:Zeile:Code`. Neue Diagnosen werden unabhängig vom Anzahl-Gate sichtbar; eine ersetzte Warnung bei gleicher Anzahl wird dadurch nicht automatisch zum Gatefehler. Keine Behauptung „warnungsfrei“.
- Rust: zusätzlich zum Workspace ausdrücklich `cargo test --offline --locked -p pi-backend --features cocopi` (Abholfach-Kryptografie).
- `i2c-szenarien`: eigener Lauf mit Einzelergebnis für Desktop-Upload/Konfliktimport/Freigabe, Node→Rust-Entschlüsselung, expliziten Großupload-Header und Node-Zwangs-Check-out/Zweitgerät/72-h-Löschung. `COCOPI_PI_HUB` ist der erste Eintrag aus `V3_GATE_HUBS`, ersatzweise der erste CLI-Hub. Leere Rust-Testfilter, ignorierte Tests und stille Node-Cross-Test-Skips sind **FAIL**, nicht PASS. Ohne gewählten Hub ist diese Stufe ausdrücklich SKIP. Die Node-Simulation nutzt die reale Hub-Kryptografie, einen Fake-OIDC-Issuer, Slot-/Leeren-Testdoubles und eine injizierte Uhr; kein echter Konten-/Dienst-/Zeitänderungstest.
- Anwendungsdaten werden in temporäre HOME-/XDG-Verzeichnisse umgeleitet. Cargo bleibt offline/locked, Target pro Worktree. Optionaler Hub nur Mock, keine Verwendung des laufenden Dienstes.

Ein grünes Gate ist keine Prozess-/Remote-only-, Sicherheits-, UI-, Plattform- oder Pilotabnahme. Insbesondere ist die bestehende Basis noch ohne S2/S3-Integration; für deren Funktionsnachweise sind die jeweiligen Rollenberichte und weitere Tests maßgeblich.

## Selbsttest

`python3 test/self-test.py` prüft Auswahl gegen Merge-Base (auch vorausliegende Basisreferenz), unveränderte/gelöschte/neue Pfade mit Leerzeichen, fehlende Referenz und leere Auswahl; bekannte/neue/wieder grüne JS-Fehler; optionale Mobil-/R2-Prüfungen; Fehlerfortsetzung, isoliertes HOME, Target und Lock; Clippy-Metriken und Spawn-Beobachtungshelfer. Cargo/Node/rustfmt sind dabei Stubs, kein Produktfunktionstest.
