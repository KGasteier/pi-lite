# Dein Auftrag: S3-Review (Opus) · Sicherheitsreview I1 — I1-S3-review (≤ 60 min)

Du bist der unabhängige **Sicherheitsreviewer** für S3 (§5: „Review Opus 5.5 je Iteration“). Du **änderst keinen Code**.
Du schreibst nur `/home/medienhaus/Work/pi-lite/coord/runs/I1-S3-review-bericht.md` (+ Heartbeat `coord/heartbeat/s3-review`).

Prüfgegenstand (nur lesen; Tests ausführen ist erlaubt, nur localhost, Temp-Verzeichnisse):
- pi-lite `/home/medienhaus/Work/pi-lite-wt/s3` Branch `v3/s3`, Diff `git diff cocopi-v3...v3/s3`:
  `copi/crates/pi-backend/src/{mode.rs,winshim.rs,hub_v1.rs,backends.rs,rpc.rs,cli.rs,preview.rs}`, `shared/protocol/v1/**`.
- pi-hub `/home/medienhaus/Work/pi-hub-wt/s3` Branch `v3/s3`, Diff `git diff master...v3/s3` (`server.mjs`, `lib/**`, `test/**`).
- Verbindlich: `coord/FREEZE-I1.md`, `docs/v3/KONZEPT.md` A2/A3, `shared/protocol/v1/SICHERHEIT.md` (Selbstauskunft S3).

Prüffragen (je Befund: Schwere hoch/mittel/niedrig, Datei:Zeile, Angriff in einem Satz, Vorschlag, Testvorschlag):
1. E4: Gibt es **irgendeinen** Weg, wie die App `pi` (oder node/einen Agenten) lokal startet, ohne `pi_command` zu passieren,
   oder wie RemoteOnly umgangen wird (Env, kaputte Datei, Race beim Start, `set` vor `init`)?
2. A3: Einmaligkeit/Atomizität (auch über Hub-Neustart), Ablauf, Drosselung pro Absender (was ist „Absender“ hinter Proxy?),
   Code-/Token-Leaks in Logs, URLs, Fehlermeldungen, Registry-Datei, Prozessliste/Env.
3. Gerätebindung: Ed25519-Challenge (Replay, Bindung an Hub/Slot/Zweck, Nonce-Länge), Token-Ablauf, Widerruf (wirklich sofort,
   auch laufende SSE und gecachte Tokens), fremder Slot, zweiter Desktop braucht Handybestätigung — Umgehung?
4. Sessiongrenze T3: Race bei parallelem Öffnen/Senden, Zählung nach Neustart, `exited`.
5. Hub-Allgemein: Pfadausbruch (cwd/ROOTS/Session-Pfade), Body-Größen, SSE-Ressourcen (DoS durch viele Verbindungen),
   alte `/api` im Slotbetrieb wirklich gesperrt, Dateirechte 0600/0700, `HUB_STATE`-Symlinks.
6. Rust-Client: TLS/Loopback-Regel, Redirects, Schlüsseldatei-Rechte (auch Windows/macOS), Panics bei bösartigen Antworten,
   Größenlimits (Antwort, SSE-Zeile).
7. Was ist durch Tests belegt, was nur behauptet? Lies die Tests, führe `node test/v1.mjs` und
   `CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s3 cargo test --offline --locked --manifest-path copi/Cargo.toml -p pi-backend`
   aus (PATH vorher setzen, `CARGO_BUILD_JOBS=3`).

Ergebnis: Freigabe für Merge in `cocopi-v3` **ja / ja mit Auflagen / nein**, plus Liste „vor I2 beheben“ und „vor Pilot beheben“.
