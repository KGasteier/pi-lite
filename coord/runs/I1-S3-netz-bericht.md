# I1-S3-netz · Abschlussbericht

**Stand:** 28.10.2026, ca. 19:12 MEZ. Beginn laut Run 17:42:57, innerhalb des 150-Minuten-Budgets. Implementierung und lokale Mocknachweise geliefert; **keine öffentliche oder echte R2-/UI-Abnahme**.

## Branches und Checkpoints

pi-lite `/home/medienhaus/Work/pi-lite-wt/s3`, `v3/s3`, vor Beginn fast-forward von `cocopi-v3` bis `e0d9280`:

1. `2001d5f` – zentrale Remote-only-Startsperre und sichere Backendliste (A2).
2. `bc34d2d` – HTTP/SSE-Vertrag v1, Schema und 21 Beispiele (A1-v1).
3. `01ebced` – Ed25519-Hubclient, gestarteter lokaler Integrationstest, Sicherheitsnotiz; bestehende Clippy-Warnungen in `instances.rs`/`material.rs` mechanisch behoben.
4. `574b6e1` – beschädigte Schlüssel und fremde Einladungsziele ablehnen.
5. `be2a2bd` – Hubtestpfad für CI über `COCOPI_TEST_HUB_DIR` parametriert.

Hub `/home/medienhaus/Work/pi-hub-wt/s3`, `v3/s3`, Basis `3dae8ca`:

1. `60eb4a9` – Geräteanmeldung, Einmalcodes, Registry und atomare Slotgrenzen.
2. `5cd00e1` – Prozessstart reservieren, Rückfragen anzeigen, Fake-pi-RPC-Prozess testen.
3. `2d0f471` – zusätzliche Rollback-, Challengeablauf- und Einladungsersatztests.
4. `365cadc` – servergebundene Bestätigungsbeschreibung ausliefern und testen.

Nur explizit besessene Quell-/Dokupfade committed. Kein Push, Merge, Reset, Stash, Release oder Installation. Keine App-/UI-/PWA-Datei geändert. Keine Nutzerprozesse/-dienste beendet. Hub-Gitidentität fehlte: Commit nur mit `git -c user.name=… -c user.email=…` aus vorhandener pi-lite-Identität, keine globale Änderung.

## Lieferung / API für S2 und S5

### A2 – fail-closed

`pi_backend::mode::{Mode,init,current,set,SpawnPurpose,SpawnBlocked}`, `winshim::pi_command(purpose,args) -> Result<Command,SpawnBlocked>`.

RemoteOnly bei fehlender/kaputter Datei; `COPI_MODE_FILE` überschreibt ausschließlich den Dateipfad. `set` persistiert privat/atomar, dann erst Modusänderung im Speicher. Vier produktive Callstellen (Chat, CLI, Version, Helper) angepasst; `pi_version` liefert jetzt `Result<String,String>`. Gate greift auch vor gecachtem Versionsergebnis und bevor RPC einen bestehenden Prozess beendet. Kein lokaler Backend-Eintrag bei RemoteOnly.

**S2 muss `mode::init()` beim Appstart aufrufen**, UI/Bestätigung aus A2 liefern und `app/src/net.rs` ohne lokalen Fallback halten. Uninitialisiert bleibt sicher RemoteOnly. In `copi/app` wurden beim ersten Abgleich keine direkten `pi_command`-/`pi_version`-Aufrufe gefunden. S2-Abnahme/Workspace-Build steht separat aus.

### A1/A3 – Protokoll und Client

Kanonisch: `shared/protocol/v1/PROTOKOLL.md`, `schema.json`, `examples/*.json`; Sicherheitsgrenzen: **`shared/protocol/v1/SICHERHEIT.md`**. HTTP/JSON + Header-authentisiertes SSE, Puls 15 s, Ausfallhinweis nach 45 s ist Client/UI-Aufgabe. Version 1, sechs Sessionzustände, deutsche `{code,was,tun}`-Fehler, Replay mit Snapshot bei Ringlücke. Additiv nach A1-v1: Route `sessions/:id/antwort` dokumentiert. Pending-Rückfrage steckt zusätzlich im `session_zustand.frage` (optionales Feld).

`pi_backend::hub_v1`:

- `parse_invitation(input,context)` akzeptiert Fragment, Query und Rohcode; Codes normalisiert, Adresse geprüft. HTTPS außer numerischem Loopback, keine URL-Credentials/Redirects oder stillen Hubwechsel.
- `DeviceKey::load_or_create(name)` → `copi_config_dir/devices/<name>.json`, private Datei 0600, Ed25519. `at(root,name)` für isolierte Tests. Privatschlüssel/Token ohne Debug; keine Fremddeps außer erlaubtem `ed25519-dalek=3.0.0`.
- `HubV1::new`, `hello`, `redeem`, `login`, `pairing_status`, `sessions`, `open_session`, `send`, `stop`, `rest`, `view`, `answer`, `start_pairing`, `approve_pairing`, `devices`, `revoke`, `events(last_id)`.
- `events` ist **blockierend**, für einen UI-Worker gedacht; `next_event` liefert Paket und optionale Replay-ID. EOF/Fehler nicht durch automatischen Prompt-Neuversand beheben. Token nach Ablauf über Geräteschlüssel erneut anmelden. Nach Koppeln `pairing_status` aufrufen, um die Geräte-ID lokal zu speichern, danach `login`.

Kein Traits-/UI-Adapter in `copi/app` implementiert (S2-Besitz). `pi-net` musste für dieses HTTP/SSE-Protokoll nicht geändert werden.

### A4 – Hub

Node-Bordmittel, v1 neben Altbetrieb. `HUB_SLOT=account01…account10` schaltet v1 ein; im Slotbetrieb ist `/api` gesperrt, sonst könnten gemeinsame Alttokens die Gerätegrenzen umgehen. Ohne `HUB_SLOT` alter Smoke grün. `HUB_STATE` privat je Slot setzen; bei altem `.json`-Pfad v1-Unterverzeichnis daneben. CLI:

```sh
HUB_SLOT=account01 HUB_STATE=/tmp/eigener-teststate HUB_PORT=18801 node server.mjs admin einladung --slot account01 --name Mira
```

Registry mit Dateisperre und atomarem privatem Austausch. Nur Codehashes gespeichert, e=24 h/k=5 min/b=2 min, Single-use, fünf Fehlversuche/Minute je Sender, Sperre nach zehn bekannten Codefehlern. Neue Einladung/Kopplung ersetzt ungenutzten Vorgänger gleicher Aktion. 60-s-Ed25519-Challenge, 12-h-Token, unmittelbarer SSE-Widerruf. Kopplung registriert Handy erst nach Desktopzustimmung; weiterer Desktop bei vorhandenem Handy braucht Handybestätigung.

Zwei offene Sessionprozesse, eine aktive Reservierung einschließlich Start/Rückfrage; zweiter Auftrag wird erklärt abgewiesen. Zustand persistiert, Neustart lässt alle `ruht`, keine automatischen Agentenstarts/Sessionlöschungen. Allein ein Stubvertrag für `HUB_GLOBAL_HEAVY_JOBS=1`, **keine globale CPU-Queue**. Hubdetails: `docs/PROTOKOLL-V1.md`.

## Tatsächlich ausgeführte Prüfungen

Alle Builds/Tests offline bzw. localhost, keine echten Modellaufrufe. Cargo mit PATH-Vorgabe, `CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s3`, `CARGO_BUILD_JOBS=3`.

| Prüfung | Ergebnis / Log |
|---|---|
| `cargo test --offline --locked --manifest-path copi/Cargo.toml -p pi-backend` | **92 Unit + 4 Integration bestanden**, 2 bestehende echte Live-Tests ignoriert; `/tmp/I1-S3-client.log` |
| `cargo clippy --offline --locked --manifest-path copi/Cargo.toml -p pi-backend --all-targets -- -D warnings` | grün; `/tmp/I1-S3-clippy.log` |
| `node test/v1.mjs` (Hub) | **31 benannte Prüfungen bestanden**; `/tmp/I1-S3-hub.log` |
| `node test/smoke.mjs` (Hub) | alle bestehenden Prüfungen bestanden; `/tmp/I1-S3-smoke.log` |
| JSON Schema Draft 2020-12, lokal vorhandenes Python `jsonschema` | Schema selbst und alle 21 Beispiele validiert, keine Installation |
| `git diff --check` in beiden Worktrees | grün |
| `cargo fmt --all --check` | **nicht grün**, bestehende workspaceweite Formatabweichungen; `/tmp/I1-S3-fmt.log`; keine fremden Dateien formatiert. Neue `hub_v1.rs` und eigene `mode.rs` separat rustfmt-formatiert. |

Rust-Hubintegration startet tatsächlich `node server.mjs` auf eigenem Loopback-Port mit Mock-Harness und Temp-State: Einladung, Ed25519, gebrauchten Code ablehnen, Sessionliste, SSE, Öffnen, Handy zunächst nicht registriert, Zustimmung, Handylogin, Widerruf/401. Node fehlt → begründetes Skip; Hubpfad über `COCOPI_TEST_HUB_DIR`, sonst benachbarter `pi-hub-wt/s3`-Worktree relativ zum Crate. Fehlender Default-Hub → begründetes Skip, explizit gesetzter ungültiger Pfad → Testfehler. **S5/CI bitte `COCOPI_TEST_HUB_DIR` auf den integrierten Hubstand setzen.**

Zusätzlich Node-Test gegen echten Hub-Kindprozess mit `test/fake-pi.mjs` über normalen PiInstance-Adapter: get_state → bereit, Prompt → wartet, ID-gebundene Antwort → fertig, Ruhe, alte API im Slot gesperrt. Kein echter pi-/Modellprozess. Unit/HTTP-Negativtests umfassen Slots, Codehash/0600, Ablauf/Verbrauch, Rate-/Codesperre, parallele Einlösung und Sessionreservierung, ungültige Signatur/Challengeablauf/-wiederverwendung, Tokens, Rollback, neuer Code, Zustimmung/Ablehnung, SSE-Ende bei Widerruf und Loggeheimnisse.

## Offene Punkte / Antrag an K

1. **Ownership Cargo.lock:** Cargo hat genau eine Zeile `"ed25519-dalek",` beim Paket `pi-backend` in `copi/Cargo.lock` ergänzt (bereits vorhandene Version/Abhängigkeiten unverändert). Datei liegt außerhalb S3-Allowlist und wurde **bewusst nicht committed**; sie bleibt als einzige uncommittete Änderung. **K bitte diese eine Zeile zusammen mit dem Client übernehmen**, sonst scheitert `--locked` im integrierten Stand. Kein lockfile-Reset vorgenommen.
2. **Abnahme-Widerspruch:** Auftrag fordert zweite aktive Session **abweisen**, älteres T3 erwähnt Queue. Implementiert Abweisung gemäß explizitem I1-Auftrag; K bitte T3 passend entscheiden/berichtigen. Globale schwere-Job-Queue bleibt I2.
3. S2/S5 bitte API/Schema übernehmen, Modus initialisieren und echte Oberflächenfälle abnehmen. `docs/HANDOFF.md`, Hub `docs/STATUS.md`/`TODO.md` nicht S3-owned: bitte dort Schnittstellen-/Teststand nachtragen.
4. Vor Freigabe: TLS/Tailscale, Proxy/Origin/IP-Limits, Unixkonten/Quoten, R2-Instanzlayout, Keychain/Recovery/Hostpinning, unabhängiger Security-Review; Windows/macOS-Build und Schlüsselrechte, echte Mobilgeräte/Funkloch/Push. Öffentlicher Hub ausdrücklich nicht freigegeben.
5. Noch keine vollständige Historie nach Neustart, kein automatischer Promptretry oder Exactly-once; Prozesse/RAM werden nicht wiederbelebt. Nutzer muss unklaren Ausgang vor neuem Auftrag prüfen. Auth-Zustände instanzlokal; kein Multiworkerbetrieb.

Sicherheitsnotiz ist die maßgebliche Grenze zwischen getesteter Funktion und offenem Betrieb. Keine privaten Konfigurations-/Session-/Modellinformationen ins Repo übernommen.
