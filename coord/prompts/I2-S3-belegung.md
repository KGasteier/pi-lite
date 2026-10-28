# Dein Auftrag: S3 · Belegung, Empfangs-Client, OIDC — I2-S3-belegung (≤ 150 min)
Worktrees: pi-lite `/home/medienhaus/Work/pi-lite-wt/s3`, pi-hub `/home/medienhaus/Work/pi-hub-wt/s3` (beide aktuell).
1. Hub: `lib/belegung-v1.mjs` + Routen nach A6: `POST /v1/belegung` und `POST /v1/freigabe` (Ed25519 vom Empfang,
   `HUB_EMPFANG_PUBKEY`, kanonisches JSON, Replay-Schutz über `checkin_id`, Slot muss `HUB_SLOT` sein). Freigabe: alle Geräte
   widerrufen, alle pi beenden (SIGTERM, 10 s, SIGKILL), erst dann 200. Nur von 127.0.0.1 erreichbar (Proxy blockt diese
   Pfade, dokumentieren). `senden` additiv `von:{person,rolle}` (nur durchreichen/anzeigen, keine Rechte daraus).
   Für `/v1/sync/*` S7s `handleSync` aufrufen, falls `lib/sync-v1.mjs` existiert, sonst 501 `nicht_verfuegbar`.
   Tests in `test/v1.mjs`: falsche Signatur, fremder Slot, Replay, Freigabe beendet Prozesse (fake-pi), Geräte danach 401.
2. Rust `pi-backend`: `empfang.rs` — Client für A6 (`hallo`, `checkin` 201/202, `warten`, `checkout`, `erhalten`, `status`)
   über denselben gehärteten Agent wie `hub_v1` (kein Proxy, Limits); Ausweis-/Belegungsprüfung
   `verify_signed(json, pubkey)` mit kanonischem JSON (öffentlich für S8). `oidc.rs` — PKCE S256 (Verifier 43–128 Zeichen),
   `state`/`nonce`, Autorisierungs-URL aus Discovery, Loopback-Listener `127.0.0.1:0` (nimmt genau einen Redirect an,
   prüft `state`, antwortet mit kleiner deutscher HTML-Seite „Du kannst zu cocopi zurückkehren“), Code-Tausch gegen
   Token-Endpoint. Browser-Öffnen nur über Callback (App ruft System-Browser). Tests gegen lokalen Mock-Issuer
   (Rust-Testserver oder S7s `fake-oidc.mjs`, wenn vorhanden).
3. `shared/protocol/v1/PROTOKOLL.md` + `schema.json` + `examples/`: Abschnitte Empfang (A6), Belegung, Sync-Endpunkte
   (Verweis auf S7-Dokument `shared/sync/SYNC.md`), Fehlercodes neu: `pool_voll`(202, kein Fehler), `keine_berechtigung`(403),
   `ausweis_ungueltig`(401), `belegung_ungueltig`(403), `nicht_verfuegbar`(501), `archiv_fehlt`(404).
Heartbeat, Checkpoints; Bericht `coord/runs/I2-S3-belegung-bericht.md`.
