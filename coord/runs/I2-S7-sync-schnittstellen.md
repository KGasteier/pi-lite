# S7 → S3/S2/S6 · historische frühe Schnittstellenmeldung

**Überholt, Stand 22:23:** Signaturen sind flach `{...Nutzfelder,signatur}`.
Defaultadapter für register/release und signierte Dienstarchive ist implementiert;
S3-Routerintegration mit S7-Standardadapter grün getestet. Kanonischer Endstand:
`I2-S7-sync-bericht.md`, `I2-S7-sync-abgleich.md`, `shared/sync/SYNC.md`.
Nachfolgend nur Entstehungsprotokoll, nicht aktuelle Implementierung.


Stand: Sync-Modul Hub-Commit `1f038a3`, Vertrag pi-lite `334604d`.
Bitte S3 `handleSync(req,res,{devices,sync})` vor JSON-Leser einbinden;
`sync=createSync({dir:<privater Hub-State>,roots:{Projekte,Ergebnisse,Skills,Verlaeufe}})`.
Export in `lib/sync-v1.mjs`. Vollständiger Vertrag `shared/sync/SYNC.md`.

Signierte Dokumente S7: Hülle `{inhalt:{…A6-Felder},signatur:base64url}`,
öffentlicher Ed25519-Schlüssel raw 32 Byte Base64url, kanonisches JSON rekursiv sortiert.
Bitte S3/S8 denselben Vertrag nutzen oder Änderungsbitte an K.

**Blocker echte Empfang-Hub-Kopplung**: Für Empfang braucht es serverseitige
Service-Adapter `register(slot,belegung)`, `release(slot,freigabe)`,
`archive(slot)` → `{archiv_id,sha256,bytes,data:Buffer}`.
S7 liefert injizierbare Adapter und verweigert ohne diese echte Check-ins,
statt Geräte-Authentisierung zu umgehen. Bitte S3 sicheren Aufrufweg (signierte
Archivanforderung oder eng begrenztes Diensttoken) liefern/festlegen.
Kein Token-Bypass durch Loopback-Vertrauen.

S7 baut Fake-OIDC-Issuer in `empfang/test/fake-oidc.mjs` (Export `createFakeOidc`).
Rust-Workspace: S7 ergänzt `copi/Cargo.toml` ausschließlich um `crates/pi-sync`,
meldet Lockfile-Änderung separat. Keine App-Dateien geändert.
