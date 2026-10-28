# I1 · Eingefrorene Absprachen (K, 28.10.2026, nach S1-Entwurf und S4-Befund)

Grundlage: `docs/v3/KONZEPT.md` (A1–A4), `docs/v3/INVENTAR.md` (A5), `deploy/r2/README.md` §9 (S4).
Wo dieses Blatt abweicht, **gilt dieses Blatt**. Änderungen nur per Antrag an K.

## A2 Moduslogik — eingefroren wie KONZEPT A2, mit Besitzklärung
- `pi_backend::mode` (neu) und `winshim::pi_command(purpose, args) -> Result<Command, SpawnBlocked>`: **Besitzer S3** (pi-backend).
  API-Signaturen exakt wie KONZEPT A2. `mode.json` in `copi_config_dir`, fehlt/kaputt ⇒ `RemoteOnly`.
- Testhilfe: `mode::set_for_test(Mode)` nur `#[cfg(test)]`/`test_support`; Umgebungsvariable `COPI_MODE_FILE` darf den Pfad
  für Tests/Szenen überschreiben (nie den Modus selbst).
- `backends.rs` (S3) und `app/src/net.rs:588` (S2): kein automatischer lokaler Eintrag/Rückfall bei RemoteOnly.
- UI (Statuszeile, Einstellungen › Modus, Bestätigungsdialog, Texte wörtlich): **S2**.

## A3 QR-Ablauf — eingefroren wie KONZEPT A3, mit diesen Entscheidungen
- URL-Form: Erzeuger schreiben **Fragment** `https://<host>/c#1.<art>.<code>`. Parser (Desktop, Mobil) akzeptieren zusätzlich
  `?c=1.<art>.<code>` und den reinen Code `XXXX-XXXX-XXXX-XXXX` (+ Art aus Kontext). Hub loggt Codes **nie** (Test).
- Host-Platzhalter: Konfigurationswert `hub_url` je Slot; in I1 Mock `http://127.0.0.1:<port>`.
- Gerät: Ed25519 (Desktop Rust `ed25519-dalek` 3.0.0, schon im Lockfile; Node `crypto` ed25519). Desktop-Schlüssel in I1 als
  Datei 0600 unter `copi_config_dir/devices/` (Keychain = I2/I3, als offener Punkt markieren).
- Zugangstoken: kurzlebig 12 h, an `geraet_id` gebunden, nur im `Authorization`-Header, nie in URL oder Log.
  (Heutiges `?token=` für SSE: in Protokoll v1 ersetzt durch Header bzw. kurzlebiges SSE-Ticket.)
- QR-Bild am Desktop: Crate `qrcodegen = "1.8.0"` (MIT, ohne Abhängigkeiten) ist freigegeben (S2, nur `copi/app`),
  gerendert als egui-Textur. Kein Kamera-Scan am Desktop in I1.
- Prüfwort (Ahorn · Kiesel) beim Koppeln: I1 optional, I2 Pflicht.

## A1 Protokoll v1 — Rahmen eingefroren, Detail baut S3 in `shared/protocol/v1/`
- Ausgangspunkt KONZEPT A1. Transport: **HTTP + SSE** (wie heutiger Hub), JSON. Kein WebSocket in I1.
- Pflicht: `protokoll`-Zahl, Handschlag, Zustände ruht/arbeitet/wartet/fertig/bereit/fehler (bereit = intern, ohne Markierung,
  von S1 bestätigt), Fehlerobjekt `{code, was, tun}`, `puls` 15 s / 45 s.
- S3 liefert `shared/protocol/v1/PROTOKOLL.md` + `schema.json` + Beispiel-Nachrichten (`examples/*.json`), die S2/S5-Tests nutzen.
  Erste Fassung ist Pflicht-Checkpoint („A1-v1 fertig“ im Heartbeat), danach nur additiv.

## A4 Kontomodell — eingefroren (Kompromiss S1/S4, Tutorial-Begriffe)
| Größe | Wert |
|---|---|
| Slot-ID (sichtbar, stabil) | `account01` … `account10` (Tutorial); `account10` = Reserve |
| Unix-Konto | gleich der Slot-ID: `account01` … |
| UID/GID | 20001 … 20010 (S4-Vorschlag) |
| Hub | systemd-User-Dienst `cocopi-hub@accountNN`, `127.0.0.1:88NN` (8801 … 8810) |
| Hub-Vertrag | `cocopi-slots-v1`: `HUB_SLOT`, `HUB_MAX_OPEN_SESSIONS=2`, `HUB_MAX_ACTIVE_SESSIONS=1`, `HUB_GLOBAL_HEAVY_JOBS=1` |
| Geräte-Register | im Hub des Slots, nur Geräte dieses Slots |
| Speicher | ext4-Userquote hart 10 GB (`/`) + 70 GB (`/mnt/data`) = 80 GB (S4-Befund) — Vorschlag, Freigabe Mensch offen |
| Grenzen | Vorschläge S4 (CPU 100 %, MemoryHigh 2 GiB/Max 2,5 GiB, TasksMax 256) — Freigabe Mensch offen |
- S4 benennt `cocopi-sNN` → `accountNN` um (Folgeauftrag). Globale CPU-Queue: `flock` auf gemeinsame Datei, Detail I2.

## A5 Inventar — eingefroren wie `docs/v3/INVENTAR.md`, mit Antworten auf S1-Fragen
1. Lehrenden-Sicht (Aylin): **kein** App-Ort in I1; Admin-Prüfskript (S4) in I2.
2. „Neue Session“ mobil: in I1 weg, Entscheidung I2.
3. Modellwahl für Lernende: sichtbar in D-EI, nur Modelle der Freigabeliste des Slots (Liste kommt vom Hub; I1: fest im Mock).
4. QR Fragment/Query: siehe A3 (Fragment erzeugen, beides parsen).
5. Zustand „bereit“: bestätigt (intern, ohne Markierung).
