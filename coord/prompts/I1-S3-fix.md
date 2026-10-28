# Dein Auftrag: S3 · Nachbesserung nach Sicherheitsreview — I1-S3-fix (≤ 100 min)

Rolle S3, Worktrees wie zuvor: pi-lite `/home/medienhaus/Work/pi-lite-wt/s3` (`v3/s3`, Stand `64e7154`),
pi-hub `/home/medienhaus/Work/pi-hub-wt/s3` (`v3/s3`, Stand `365cadc`). Commits im Hub mit
`git -c user.name=KGasteier -c user.email=90152038+KGasteier@users.noreply.github.com`.
**Lies zuerst** `/home/medienhaus/Work/pi-lite/coord/runs/I1-S3-review-bericht.md` (Opus-Review) und
`/home/medienhaus/Work/pi-lite/coord/runs/I1-S2-desktop-anfrage.md`.

Pflicht (Merge-Auflage), je mit dem im Review genannten Regressionstest:
1. **H1** Jede e-Einlösung bei vorhandenem nicht widerrufenem Gerät braucht Bestätigung; `geraeteart` vom Client entscheidet das nie.
2. **H2** b-Code/URL nie per SSE/Replay; nur in der HTTP-Antwort an den einlösenden Desktop.
3. **H3** Ereignisse nur bei `s.inst === inst`; `ruhen` zählt als offen bis zum echten Exit (oder Wiederöffnen bis dahin abweisen).
4. **Fehlercodes für S2:** `hub_v1` liefert typisierte Fehler, z. B. `pub struct HubError { pub code: String, pub was: String, pub tun: String }`
   mit `Display`, oder die vereinbarte Textform `[code] was tun` — wähle die typisierte Variante, wenn sie additiv bleibt
   (bestehende `Result<Value,String>`-Methoden dürfen bleiben; ergänze z. B. `HubV1::last_error_code()` oder `*_typed`-Varianten nicht nötig,
   wenn du die String-Form mit `[code]`-Präfix wählst). Dokumentiere die Entscheidung in `shared/protocol/v1/PROTOKOLL.md`.
   Zusätzlich N5: Client bildet bekannte Codes auf lokale Texte ab, Servertext nur als Rückfall.
5. **M3** Signatur mit Kontextbindung: `"cocopi-v1-anmelden\n"+hub_origin+"\n"+slot+"\n"+geraet_id+"\n"+challenge_id+"\n"+nonce`
   (Client + Hub + PROTOKOLL.md + Beispiele + Test „Signatur für Hub A wird von Hub B abgewiesen“).
6. **M5** Start-Timeout (z. B. 60 s, `HUB_START_TIMEOUT_MS`), danach `fehler` + Prozess beenden; `ruhen` mit erzwungenem Stopp erlaubt.
7. **N2/N3** Rust-SSE Lese-Timeout ~45 s; ureq-Agent ohne Umgebungs-Proxy.
8. Wenn Zeit bleibt: **M4** (`HUB_STATE` Eigentümer/Rechte/Symlinks, `wx`+Zufallsname für Temp-Dateien) und **M1** (`hallo` schreibt nicht, veraltete Sperre erkennen).

Prüfungen wie im ersten Bericht (pi-backend Tests + clippy `-D warnings`, `node test/v1.mjs`, `node test/smoke.mjs`, Schema-Validierung).
Formatiere nur eigene neue/geänderte Dateien mit `rustfmt --edition 2021 <datei>`; kein workspaceweites `cargo fmt`.
Heartbeat „FIX fertig <pi-lite-commit> <hub-commit>“. Bericht nach `coord/runs/I1-S3-fix-bericht.md` (Tabelle Befund → Commit → Test).
