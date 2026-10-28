# Dein Auftrag: S3 · Netz, Sicherheit, Anbindungen — I1-S3-netz (≤ 150 min)

Rolle S3 (§5, §8). Worktrees: pi-lite `/home/medienhaus/Work/pi-lite-wt/s3` (Branch `v3/s3`) und
pi-hub `/home/medienhaus/Work/pi-hub-wt/s3` (Branch `v3/s3`, Basis `master@3dae8ca`).
Zuerst im pi-lite-Worktree `git merge --ff-only cocopi-v3` ausführen (holt docs/v3 und coord/FREEZE-I1.md).
**Verbindlich:** `/home/medienhaus/Work/pi-lite/coord/FREEZE-I1.md` + `docs/v3/KONZEPT.md` (A1–A4) + `docs/v3/ACCEPTANCE.md`.

Du besitzt: `copi/crates/pi-backend/**`, `copi/crates/pi-net/**`, `shared/protocol/**` (pi-lite) und im Hub-Worktree
`server.mjs`, `lib/**` (neu, erlaubt), `test/**`, `docs/PROTOKOLL-V1.md`. Nicht: `copi/app/**` (S2), `ui/`, `mobile/`.

Reihenfolge (nach jedem Punkt Checkpoint-Commit + Heartbeat):
1. **A2 Spawn-Sperre (T1, zuerst, S2 wartet darauf):** `pi-backend/src/mode.rs` + `winshim::pi_command(purpose, args)
   -> Result<Command, SpawnBlocked>` exakt nach KONZEPT A2/FREEZE. Alle 4 Aufrufer in pi-backend anpassen
   (`rpc.rs`, `cli.rs` 2×, `preview.rs`); Fehler als normale Fehlermeldung nach oben reichen. `backends.rs`: bei RemoteOnly kein
   lokaler Eintrag/Autoeinfügen. Tests: `remote_only_blocks_pi_command`, `hybrid_allows_pi_command`, `missing_mode_file_is_remote_only`,
   `corrupt_mode_file_is_remote_only`, `only_winshim_spawns_pi` (Quelltext-Scan über `copi/**/*.rs`, außer winshim.rs und Tests),
   `remote_only_backends_has_no_local`. **Hinweis an S2** über die geänderten Signaturen im Bericht + Heartbeat-Zeile
   „A2 fertig <commit>“. `copi/app` kompiliert danach evtl. nicht (Aufrufer in app/) — das ist S2s Teil; prüfe mit
   `cargo test --offline -p pi-backend`. Wenn app/ nur durch die Signatur bricht, liste die Stellen exakt auf.
2. **A1 Protokoll v1:** `shared/protocol/v1/PROTOKOLL.md`, `schema.json` (JSON Schema), `examples/*.json`
   (mindestens: hallo, willkommen, sessions_liste, session_zustand je Zustand, fehler zu_viele_offen, fehler
   protokoll_zu_alt, puls, einladung_einloesen req/resp, koppeln_start/einloesen/bestaetigen, geraete_liste, widerruf).
   HTTP-Endpunkte konkret (Pfad, Methode, Auth) + SSE-Ereignisse. Heartbeat „A1-v1 fertig <commit>“.
3. **Hub (pi-hub-wt/s3), Mock-fähig, nur Node-Bordmittel:** Protokoll v1 neben der alten API unter `/v1/…`:
   Handschlag, Geräte-Register (Datei 0600 im `HUB_STATE`-Verzeichnis), Einladung erzeugen (CLI `node server.mjs admin
   einladung --slot account01 --name Mira` o. ä. → gibt URL aus), Einlösen (sha256, atomar, Drosselung 5/min, Sperre nach 10),
   Ed25519-Challenge → Zugangstoken 12 h, Koppeln mit Desktop-Zustimmung, Widerruf (schließt offene SSE sofort),
   Sessiongrenze 2 offen / 1 arbeitet atomar (T3) mit `fehler{code:"zu_viele_offen"}`, Zustand ruht/arbeitet/…,
   `puls` 15 s, Vertrag `cocopi-slots-v1` (Env `HUB_SLOT`, `HUB_MAX_OPEN_SESSIONS`, `HUB_MAX_ACTIVE_SESSIONS`).
   Codes/Tokens nie loggen. Tests in `test/` mit `test/fake-pi.mjs` (kein echtes pi, kein Netz außer 127.0.0.1):
   wiederverwendeter Code, abgelaufener Code, fremder Slot, abgelaufenes Token, Widerruf trennt SSE, paralleles Einlösen
   (genau 1 Erfolg), dritte Session abgelehnt, zweite arbeitende abgelehnt, Code nicht im Log. `node test/smoke.mjs` bleibt grün.
4. **Rust-Client** in `pi-backend` (neues Modul `hub_v1.rs`): Einladung einlösen (Parser für Fragment/Query/Rohcode),
   Schlüssel erzeugen/speichern (0600, `copi_config_dir/devices/`), Anmelden (Challenge), Sessions listen, Zustand-SSE,
   Koppel-Code anfordern, Geräte listen/widerrufen. Testbar gegen einen gestarteten Hub-Mock (Test darf `node` mit dem
   Hub aus `/home/medienhaus/Work/pi-hub-wt/s3` auf freiem Port starten; wenn `node` fehlt → Test überspringt mit Meldung).
   Neue Abhängigkeit nur `ed25519-dalek` (schon im Lockfile) — Cargo.toml-Änderung mit `cargo check --offline` (ohne --locked).
5. Bericht: dazu eine kurze **Sicherheitsnotiz** (Bedrohungen, was getestet, was offen) in `shared/protocol/v1/SICHERHEIT.md`
   für den Opus-Review.

Nicht in I1: Tailscale/TLS-Vorschaltung (nur dokumentieren), Hostpinning-Code (dokumentieren), SSH-Änderungen.
Bericht nach `coord/runs/I1-S3-netz-bericht.md`.
