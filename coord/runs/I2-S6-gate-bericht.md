# Abschlussbericht · I2-S6-gate

**Stand:** 28.10.2026, 22:16 +01:00 · **Branch:** `v3/s6` · **HEAD:** `f2fcf47`
**Arbeitsbaum:** `/home/medienhaus/Work/pi-lite-wt/s6`, sauber nach Abschluss.

## Ergebnis

S6-Testauftrag implementiert, offline geprüft und in sechs eigenen Commits abgelegt.
Das Mehr-Hub-Gate beendet sich mit **Exit 0**, meldet fehlende Komponenten aber
weiter ausdrücklich als **SKIP**. Das ist **keine uneingeschränkte I2-/Pilotabnahme**.
Gegen den S7-Hub bestehen **22 Node-Tests ohne SKIP/TODO**; darin führt der
Mitsteuerungscheck zusätzlich **zwei unabhängige S6-Rust-Angriffstests** gegen den
öffentlichen S8-Automaten aus. Ohne explizites `I2_SEMINAR_DIR` bleibt dieser
Check bewusst TODO. Keine fremden Rollenquellen geändert, kein Merge/Push.

## Geliefert

### Gate (`test/v3-gate.sh`)
- Mehrere positionale Hub-Pfade und/oder `V3_GATE_HUBS="a b"`, kanonisiert und
  dedupliziert; eigene Checks/Logs je Hub und Provenienz mit HEAD/Status.
- Optional `cargo test --offline --locked -p pi-sync`; fehlt die Crate im geprüften
  pi-lite-Worktree, SKIP statt Fehlbehauptung.
- Je Hub Smoke, jede Datei `empfang/test/*.mjs`, `test/sync.mjs` und S6-I2-Suite;
  fehlende Prüflinge SKIP. R2-Pythoncheck bleibt optional integriert.
- TAP-`skipped`/`todo` wird als unvollständige I2-Abnahme erkannt, nicht PASS.
- Offline Cargo, eigenes `../target-s6`, drei Jobs, isolierte Anwendungshomes
  und modellfreies Hub-/pi-Setup bleiben erhalten.

### Angriffssuite (`test/i2/`)
- Echter Empfang-HTTP-Handler, Sync-/Geräte-HTTP-Handler und Fake-OIDC/JWKS aus
  S7; zufällige Loopback-Ports, temporäre Zustands-/Slotordner.
- Check-in → Projektupload → Vorschau → Archiv → Download/Hash/lokale Sicherung
  → Erhalten → nachweislich leere Testroots und Sync-State → freier Poolplatz.
  Vor korrekter Empfangsbestätigung keine Löschung; alter Token danach ungültig.
- Hub-Fake-pi als eigener modellfreier RPC-Kindprozess; `get_state` geprüft und
  Prozessende vor Leeren/Freigabe bestätigt.
- 20 parallele **HTTP**-Check-ins: zehn eindeutige Konten, zehn FIFO-Wartende,
  Promotion nach Rückgabe; zusätzlicher Pool-Kern-/Persistenz-/90-s-Test.
- Traversal, Absolutpfad, Windows-Laufwerk, Symlink, Hardlink, bestehender
  Ziel-Symlink und **3-GiB-Tar-Größenheader ohne große Allokation/Nutzlast**.
  Abweisung atomar, keine vorangestellten gültigen Dateien zurückgelassen.
- Abholfach nur für richtige Person/Signatur; Entschlüsselung nur mit richtigem
  X25519-Schlüssel, Manipulation und Ablauf abgewiesen, erfolgreicher Besitzerabruf
  und Empfangsbestätigung geprüft. Leeren-Fehler gibt das Konto nicht frei.
- OIDC RS256/ES256 plus negative Claims-/Signaturprüfungen.
- S8: erfundener Request, ausstehende Zustimmung, Nur-Ansicht, Ablehnung,
  falsche Session, Check-out und Beenden senden nichts ins MemoryNet. Erst
  ausdrückliche Zustimmung erlaubt den markierten Leitungs-Testprompt.
- `test/i2/README.md` dokumentiert Aufrufe und genaue Testdouble-/Abnahmegrenzen;
  `coord/BUGS.md` enthält Befunde B6–B9 und nachgezogene Vertragskorrekturen.

## Verifiziert

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/share/mise/shims:$PATH
python3 test/self-test.py
I2_SEMINAR_DIR=/home/medienhaus/Work/pi-lite-wt/s8 node --test test/i2/*.test.mjs
I2_SEMINAR_DIR=/home/medienhaus/Work/pi-lite-wt/s8 CARGO_BUILD_JOBS=3 \
  V3_GATE_TIMEOUT=600 bash test/v3-gate.sh "$PWD" \
  /home/medienhaus/Work/pi-hub-wt/s3 /home/medienhaus/Work/pi-hub-wt/s7
```

- **12/12** Python-Gate-Selbsttests; Syntaxprüfung und `git diff --check` bestanden.
- Direkte S7-I2-Suite: **22/22**, null Fehler/SKIP/TODO.
- Gesamtgate: fmt, Clippy, Rust-Workspace-Tests, copi check, JS-Syntax,
  mobile (**25 Tests**), R2-Python (**23 Tests**) bestanden.
- Clippy: 37 rohe / 20 eindeutige Warnungen, Baseline 24; keine neue Überschreitung.
- JS-Tests: **KNOWN** bekannter Basisfehler (B1), nicht als fehlerfrei dargestellt.
- `pi-sync` im S6-Worktree fehlt: **SKIP**. S7s neu entstehende Rust-Crate hier
  nicht hineingemergt und nicht als durch diesen Gate-Schritt getestet behauptet.
- Beide Hub-Smokes bestanden.
- S3: Empfang-/Sync-Prüflinge fehlen, daher SKIP; S6-I2 dort **4 bestanden,
  18 SKIP** (lokale Fixtures und S8-Automat ausführbar, Dienste fehlen).
- S7: Empfang **8**, Sync **5**, S6-I2 **22 Tests** bestanden. S7-Dateien waren
  während der Entwicklung teilweise uncommitted; Gate-Provenienz erfasst den
  Arbeitsbaum, kein eingefrorener integrierter Produktstand.

### Logs (lokal, flüchtig)
- `/tmp/I2-S6-gate-selftest.log`
- `/tmp/I2-S6-scenarios.log`
- `/tmp/I2-S6-consent.log`
- `/tmp/I2-S6-final-gate.log`
- Ausführliche letzte Gate-Logs/Provenienz: **`/tmp/v3-gate.s6.a212FvMr/`**

Letzte gelesene Fremdstände: Hub S3 `229f68e`, Hub S7 `064ae2a`, pi-lite S8
`6f5306b`. Fremde Arbeitsbäume aktiv; HEAD allein beschreibt deren Dateien nicht.
Gate lief auf S6 `c79b560`; danach nur BUGS-Nachprüfung als `f2fcf47` committet.

## Offene Befunde / klare Abnahmegrenzen

1. **B6, produktiver Check-out blockiert:** S7-Standard-`hubAdapter.archive()`
   verweigert weiterhin mit `archiv_adapter_fehlt`. Der isolierte E2E-Test injiziert
   ausdrücklich einen Archivadapter. S3s ursprünglich fehlendes `ctx.sync` wurde
   inzwischen ergänzt, aber hier nicht als zusammengeführter Hub nachgewiesen.
2. **B7, gemeinsame Abnahme offen:** ursprüngliche Signaturhüllen-/Challenge-
   Abweichungen wurden während der Arbeit korrigiert (S7 flaches JSON, S3-Rust
   inzwischen `/v1/empfang/challenge` mit Check-in-Payload). Noch kein echter
   Rust-Client gegen Empfang und S3-Belegung gegen S7 gemeinsam abgenommen.
3. **B8, Mitsteuerung:** Automatenangriffe grün, echte UI-/P2P-/Hub-Kette offen.
4. **B9 behoben:** Node-TAP-Schlüssel `skipped` statt irrtümlich `skip` im Gate;
   Regressionstest enthält `skipped 1`, `todo 0`.
5. Kein Systembrowser-/PKCE-/authentik-Livetest, keine echten Systemaccounts,
   systemd-, Quoten-, Root-/slotctl- oder produktiven R2-Löschoperationen.
   `clearSlot` und Belegung/Freigabe sind **Testdoubles nur in temporären Ordnern**.
   Kein genereller No-Retention-/Datenschutzkonformitätsnachweis. Rust-Client-
   Konfliktlogik wurde nicht durch das Testfixture-Rückspielen ersetzt/abgenommen.

## Commits (nur eigene Pfade)

1. `a0513cc` – optionale Gates und mehrere Hubs
2. `d22ddeb` – Sync-HTTP-Fluss und Archivangriffe
3. `ce79611` – Pool-Konkurrenz und OIDC
4. `d1bced0` – Empfang-E2E, Abholfach, Temp-Slots, BUGS/README
5. `c79b560` – unabhängige S8-Zustimmungsangriffe
6. `f2fcf47` – nachgezogene Vertragskorrekturen im Befundstand

**Nächste Schritte K/S3/S7:** sicheren Archiv-Service-Vertrag schließen; danach
zusammengeführten Hub + Rust-Client gemeinsam abnehmen. **S8/S3:** reale
Zustimmungs-/Prompt-Verdrahtung mit denselben Negativfällen prüfen. Erst nach
separater Freigabe echte R2-Leer-/Quoten-/Datenschutzabnahme.
