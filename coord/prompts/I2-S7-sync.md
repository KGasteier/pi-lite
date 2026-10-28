# Dein Auftrag: S7 · Empfangsdienst, Pool und Sync — I2-S7-sync (≤ 160 min)
Neue Rolle S7. Worktrees: pi-lite `/home/medienhaus/Work/pi-lite-wt/s7` (`v3/s7`), pi-hub `/home/medienhaus/Work/pi-hub-wt/s7`
(`v3/s7`; Commits dort mit `git -c user.name=KGasteier -c user.email=90152038+KGasteier@users.noreply.github.com`).
Build-Verzeichnis `target-s7`.
1. `shared/sync/klassen.json` + `shared/sync/SYNC.md` (A7, Klassen, Regeln, Formate, Beispiele, Konfliktbenennung).
2. pi-hub `empfang/`: `empfang.mjs` (Dienst, `node empfang/empfang.mjs`, Env `EMPFANG_PORT=8800`, `EMPFANG_STATE`,
   `EMPFANG_SLOTS=account01..account10` mit Hub-URLs `http://127.0.0.1:88NN`, `EMPFANG_OIDC_ISSUER`, `EMPFANG_OIDC_CLIENT_ID`,
   `EMPFANG_TAGESENDE`, `EMPFANG_SLOTCTL` (Pfad zum Leeren-Befehl; im Test ein Fake-Skript)), `pool.mjs` (atomare Vergabe,
   FIFO-Warteschlange, Verfall 90 s, Persistenz mit Datei-Rename), `oidc.mjs` (Discovery, JWKS-Cache, RS256/ES256 mit
   `node:crypto`, iss/aud/exp/iat/nonce, Gruppen→Rolle), `ausweis.mjs` (Ed25519-Schlüssel 0600, kanonisches JSON, Belegung,
   Freigabe, Geräteausweis), Zwangs-Check-out (Leerlauf/Tagesende/Seminarleitung) mit X25519+ChaCha20-Poly1305 bzw.
   `crypto` ECDH x25519 + AES-256-GCM verschlüsseltem Abholfach (Format in SYNC.md), Logs ohne Namen (HMAC-Pseudonym).
   `empfang/test/fake-oidc.mjs` (Issuer mit JWKS, Token-Ausgabe für Tests, **für alle nutzbar**), `empfang/test/*.mjs`:
   20 parallele Check-ins auf 10 Slots → 10 Belegungen ohne Doppelvergabe, 10 Wartende in Reihenfolge; Nachrücken nach
   Check-out; abgelaufenes/fremdes Token; Rolle aus Gruppen; Abholfach-Ver-/Entschlüsselung; Leeren-Fake wird mit
   richtigem Slot aufgerufen; Neustart des Empfangs verliert keine Belegung.
3. pi-hub `lib/sync-v1.mjs`: `handleSync(req,res,ctx)` für `hoch`, `vorschau`, `archiv`, `archiv/:id` nach A7, tar-Prüfung
   (eigener minimaler ustar-Leser/-Schreiber + `zlib`; keine neue npm-Abhängigkeit), Pfadangriffe (`..`, absolut, Symlink,
   Hardlink, Gerät, übergroß), Manifest, Klassen aus `shared/sync/klassen.json` (Pfad per Env `HUB_SYNC_KLASSEN`, Kopie im
   Hub-Repo erlaubt mit Prüfsummen-Test gegen pi-lite-Datei, wenn vorhanden). Tests `test/sync.mjs`.
4. Rust-Crate `copi/crates/pi-sync` (Workspace-Mitglied, nur vorhandene Crates aus `Cargo.lock`/Registry-Cache:
   `flate2`, `sha2`, `walkdir`, `serde_json`, `tempfile`; prüfe mit `cargo check --offline` ohne `--locked`, melde
   `Cargo.lock`-Änderung): Ordner `~/cocopi` anlegen (Struktur A7), Klassifizierung (gleiche Regeln, Test mit gemeinsamen
   Fällen `shared/sync/faelle.json`), Auswahlmodell für Check-in (Bereiche/Projekte mit Größe, Dateizahl, flüchtig
   ausgelassen), tar.gz-Erzeugung als Stream, Archiv sicher entpacken ohne Überschreiben (Konfliktname), `.sync/zustand.json`.
   Keine HTTP-Logik (die liegt in pi-backend bei S3) — nur `Read`/`Write`-Schnittstellen.
Heartbeat-Datei `coord/heartbeat/s7`. Bericht `coord/runs/I2-S7-sync-bericht.md`.
