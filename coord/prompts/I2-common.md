## I2-Kontext (gilt zusätzlich)
Lies **zuerst** `/home/medienhaus/Work/pi-lite/coord/FREEZE-I2.md` (E7–E10, A6–A9: Pool, Empfang/authentik, Sync,
Slot-Leeren, Mitsteuern) und den Abschnitt 4.4 im Entwicklungsprompt. Diese Absprachen sind verbindlich; Abweichungen
nur als Bitte im Bericht. Bestehende v1-Verträge (`shared/protocol/v1/`) bleiben gültig und werden **nur additiv** erweitert.
I2-Zuständigkeiten (je Datei genau ein Besitzer):
- **S3** Empfang-Protokoll + Hub-Belegung: pi-hub `empfang/` *nicht*; S3 besitzt `lib/hub-v1.mjs`, `lib/devices-v1.mjs`,
  `lib/belegung-v1.mjs` (neu), Rust `copi/crates/pi-backend/src/{hub_v1.rs,empfang.rs(neu),oidc.rs(neu)}`,
  `shared/protocol/v1/*` (Abschnitt Empfang/Belegung/Sync-Endpunkte dokumentieren).
- **S7** Sync + Empfangsdienst: pi-hub `empfang/**` (Dienst, Pool, Warteschlange, OIDC-Prüfung, Zwangs-Check-out),
  `lib/sync-v1.mjs`, Rust-Crate `copi/crates/pi-sync/**`, `shared/sync/**`. Hub-Worktree `/home/medienhaus/Work/pi-hub-wt/s7`.
  Einbindung der Sync-Routen in `lib/hub-v1.mjs`: S7 exportiert `handleSync(req,res,ctx)`; **S3** ruft es für `/v1/sync/*` auf.
- **S2** Desktop-Abläufe Check-in/Sync-Auswahl/Check-out/Warteschlange in `copi/app/src/desktop.rs` + `desktop/{hub,checkin,sync_ui}.rs`.
- **S8** Seminarbereich (Chat/DM/Mitsteuern) in `copi/app/src/desktop/seminar.rs` und `copi/crates/pi-net/src/ausweis.rs` (neu);
  Einbindung in `desktop.rs` nur über **eine** Zeile/Methode, die S2 bereitstellt (`fn seminar_tab(&mut self, ui)` ruft
  `seminar::Seminar::ui`). Weitere pi-net-Änderungen additiv.
- **S4** `deploy/r2/**` (Pool, System-Units, `cocopi-slotctl`, Empfang-Unit, Reverse-Proxy-Vorlage, authentik-Anleitung,
  `EINRICHTEN-MIT-PI.md`).
- **S5** `mobile/**`, `ui/transport.js` (Andocken an eingecheckten Rechner, Ergebnisse ansehen, Mitsteuern-Hinweis).
- **S6** `test/**`, `coord/BUGS.md` (I2-Gate: Pool-Rennen, Leeren-Nachweis, Sync-Pfadangriffe).
Gemeinsame Mocks: Fake-OIDC-Issuer (JWKS + signierte id_tokens) baut **S7** in pi-hub `empfang/test/fake-oidc.mjs`, nutzbar für alle.
Bis S3/S7 liefern, arbeiten S2/S5/S8 gegen In-Memory-Adapter nach FREEZE-I2 und stellen später um.
Merge fremder Stände nur aus `cocopi-v3` (K integriert) oder ausdrücklich genannten Branches.
