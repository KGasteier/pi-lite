# I2-S7-sync · Abschlussbericht

Stand **28.10.2026, 22:24 Uhr**. Implementierungsstand in eigenen Worktrees,
beide Branch `v3/s7`, beide Arbeitsbäume sauber. Keine Merges/Pushes,
Installationen, Netzwerk-/Modellaufrufe außerhalb eigener Loopback-Testserver,
keine echten Konten/Config-/Sessiondaten angefasst. Keine fremden Rollenquellen
verändert. Testintegration kopiert S3-Quellen lesend in ein temporäres Verzeichnis.

## Ergebnis

### Gemeinsame Regeln / Rust

- `shared/sync/{klassen.json,faelle.json,SYNC.md}`: Reihenfolge der Klassen,
  19 gemeinsame Fälle, vier Bereiche mit lokalem Alias `Verläufe`, A7-Vertrag,
  Signatur-/Abholfachformat, Konfliktbenennung, Grenzen.
- `copi/crates/pi-sync/{Cargo.toml,src/lib.rs}`: Workspace-Crate ohne HTTP;
  bewusst gewählter cocopi-Ordner, vier Bereiche, private `.sync`-Unterordner,
  Zustand und lokales Protokoll, Auswahl je oberstem Projekt/Eintrag mit
  persönlicher/flüchtiger/großer Dateizahl und Bytezahl. Streaming `Read`/`Write`
  für gzip-ustar, SHA256, maximale Größen/Dateizahl, kein Ausführen von Dateien.
- `upload` nimmt nur persönliche Dateien, `upload_explicit` zusätzlich bewusst
  gewählte relative Dateien (auch groß/flüchtig). Server: additiv
  `X-Cocopi-Gross: bestaetigt`; Gesamtbudget weiter 2 GiB pro Belegung.
- Import sichert Archiv zuerst nach `.sync/archiv`, prüft SHA256, entpackt in
  privates Staging und prüft alle Zielpfade vor Rückspielen. Kein Überschreiben:
  abweichende Rückläufer `Name (R2 TT.MM. HH-MM).ext`, nummeriert bei Wiederholung;
  Konfliktliste in `.sync/konflikte`. Identische Dateien werden nicht verdoppelt.
- Symlinks/Hartlinks als Quellen ausgeschlossen, keine Archivlinks/Geräte/FIFOs,
  keine `..`/absoluten/Windows-Laufwerks- oder reservierten Gerätenamen,
  keine Punkt-/Leerraum-Endungen; Archiv-Groß/Kleinschreibungsduplikate abgewiesen.

**Cargo.lock bewusst geändert:** 26 Zeilen hinzugefügt: `pi-sync`, bereits
gecachtes `tempfile 3.27.0` und dessen bestehende Abhängigkeiten, `const-oid`
Featureabhängigkeit bei digest. `sha2="0.11"` bereits im Lockfile; keine Downloads.
`copi/Cargo.toml` nur um Workspace-Mitglied ergänzt. Kein App-/Backendcode geändert.

### Hub-Sync

- `lib/sync-v1.mjs`: `createSync({dir,roots})`, `handleSync(req,res,ctx)` vor
  allgemeinem JSON-Leser. Desktopauth, Originprüfung; Mobilgeräte ausgeschlossen.
- Upload gzip-ustar privat zwischenlagern, Header/Prüfsumme/UTF-8/Typ/Pfad/Größe/
  Zähler prüfen, dann atomarer Rename in neues Ziel. Persistentes Uploadmanifest
  mit Bytebudget und Hashes. Große Uploads nur additive Bestätigung.
- Vorschau persönlich/flüchtig/groß und neu/geändert/unverändert; Archiv enthält
  persönliche Rückläufer, nie unveränderte Uploads; Zusatz-/Ausschlussauswahl.
  Metadaten zuerst, gzip-Stream mit SHA256 danach. Max. acht Slot-Archive.
- Signierter Empfang-Dienstweg `POST /v1/sync/dienst/archiv[/:id]`:
  echte Loopback-Socketadresse, gepinnter Ed25519-Empfangsschlüssel, aktive
  Belegungsbindung, Zweck/Slot/Check-in, ±60 s und persistente Nonce-Replaysperre.
  Keine bloße Loopback-Authentisierung und keine Modellrechte.
- `empfang/klassen.json`, `empfang/test/sync-faelle.json` sind erlaubte Kopien;
  Prüfsummenvergleich zu pi-lite, wenn dort vorhanden; Env `HUB_SYNC_KLASSEN`.

### Empfang

- `empfang/{empfang,pool,oidc,ausweis}.mjs`, `README.md`, Tests/Fake-Issuer.
  CLI `node empfang/empfang.mjs`, geforderte Env-Konfiguration, Loopbackbindung.
- Pool 10 Slots, synchrone gesperrte Transaktionen, privates JSON mit fsync/Rename,
  reserviert/aktiv getrennt, FIFO ohne Rollenpriorität, 90-s-Warteverfall;
  Persistenz über Neustart. Kein Zurückgeben ohne Freigabe + Leeren + Prüfnachweis.
- OIDC Discovery/JWKS begrenzt und 10 min gecacht, RS256/ES256 mit `node:crypto`,
  iss/aud/azp/exp/iat/nbf/nonce/Signatur, Rolle nur aus authentik-Gruppen.
  Tokenhash-Replaysperre, keine Roh-ID-Tokens im Zustand. Gastcodes einmalig,
  von authentisierter Seminarleitung, Rolle immer `gast`.
- Flache kanonische Ed25519-Dokumente `{...Nutzfelder,signatur}`, private PEM 0600,
  öffentliche Schlüssel raw 32 Byte Base64url, Geräteausweis 12 h.
- Empfang unterstützt S3-Challenge pro Anfrage **und additiv** Anmeldung mit
  Challenge/Bearer. Tokens werden bei Check-out gelöscht und zusätzlich an die
  aktuelle Belegung gebunden. Status/Erzwingen/Einladung rollenbeschränkt.
- Normales Check-out: signierter Archivbau → Clientabruf am Hub → Hashbestätigung
  → Hubfreigabe → Leeren → Prüfen → Pool frei. Normale Archivbytes laufen nicht
  über den Empfang. Wiederholung nach Leeren-Fehler versucht Leeren erneut.
- Zwangs-Check-out: Leerlauf >8 h (Verbindungspuls verfällt nach 90 s), Tagesende
  oder Leitung; Archiv **vor** Freigabe mit X25519/HKDF-SHA256/AES-256-GCM an den
  Geräteschlüssel verschlüsselt und dauerhaft gespeichert; ≤72 h. Metadaten als
  AAD, Hashprüfung, Rückabruf nur gleicher pid. Große persönliche Rückläufer
  werden bei Zwangs-Check-out mitgesichert; >2 GiB blockiert statt Datenverlust.
- Logs nur Zeit/Ereignis/HMAC-pid, keine Namen/Inhalte/Tokens; Tagesdateien sieben
  Tage, Bereinigung auch im Sweep. Keine Shell bei slotctl, nur feste Aktion/Slot.
  S4-Prüfformat `{slot,zustand:"leer"}` unterstützt, Fake `{slot,leer:true}` ebenso.

## Commits (grüne Schritte)

pi-lite, Basis `e8f000e`, Endstand **`fb67990`**:
1. `334604d` gemeinsame Sync-Regeln/Fälle/Dokumentation.
2. `a84489b` pi-sync, Workspace und Lockfile.
3. `ef488c2` Pfadschutz und Dienstvertrag.
4. `55fddd7` ausdrückliche Zusatzauswahl/portable Pfade.
5. `fb67990` Größenregel am Bereichspfad angeglichen.

pi-hub, Basis `e2eabfa`, Endstand **`320035b`**:
1. `1f038a3` Streaming-Sync/Manifest/Pfadangriffstests.
2. `064ae2a` Empfang/Pool/OIDC/Ausweise/Fake-Issuer.
3. `e20ffc6` flache Signaturen, kompatible Anmeldung, signierte Dienstarchive.
4. `320035b` Routerintegration/Tagesabschluss/Pfadschutz/Tests.
Alle Hubcommits mit `KGasteier <90152038+KGasteier@users.noreply.github.com>`.
Nur explizite eigene Pfade gestaged. K führt Integration durch.

## Nachweise (ausgeführt)

**Rust, ausschließlich offline**, `CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s7`,
`CARGO_BUILD_JOBS=3`:
```
cargo check --offline --manifest-path copi/Cargo.toml -p pi-sync
cargo test --offline --manifest-path copi/Cargo.toml -p pi-sync
cargo clippy --offline --manifest-path copi/Cargo.toml -p pi-sync -- -D warnings
```
Check/Test/Clippy grün. Zusätzlich das erzeugte eigene Testbinary direkt gestartet:
**5 bestanden** (19 gemeinsame Fälle, Auswahl/Stream/Import/Konflikt, Pfad-/Hash-/
Archivlinkangriffe, Zielsymlink unverändert, Vorfahrenlink/Hartlink nicht übertragen).
Logs `/tmp/I2-S7-{check,rust,rust-direct,clippy}.log`.

**Node**, pi-hub S7:
```
node --test empfang/test/empfang.mjs test/sync.mjs
PI_HUB_S3=/home/medienhaus/Work/pi-hub-wt/s3 node --test empfang/test/integration.mjs
node test/v1.mjs
```
- **14/14** Empfang/Sync: 20 parallele Check-ins → 10 verschiedene belegte Slots
  und 10 FIFO-Wartende; Nachrücken/Neustart; 90-s-Verfall; beide JWT-Algorithmen
  und ungültige Claims/Rollen; Verschlüsselung/Manipulation/Ablauf; gesperrt bei
  Leeren-Fehler; Fake-slotctl-Argumente; Challenge/Bearer/Replay; signiertes
  Dienstarchiv, falsche Belegung; Tagesende/Leerlauf/72-h-Löschung;
  Manifest/Rückläufer/Archive/Pfadangriffe/Symlink/atomarer abgelehnter Upload.
- **1/1 Integration** mit lesend kopierten S3-Quellen **`8658ec2`**:
  echter S3-Router + S7-Standardadapter, temporäres HOME, echte signierte Belegung,
  Dienstarchiv, Empfang-Hashbestätigung/Freigabe, danach Zwangs-Abholfach. Lokale
  Fake-OIDC/JWKS und Fake-Leeren; Modellstart im Adapter ausdrücklich verboten.
- **43/43** bestehende S7-Basis-v1-Prüfungen inkl. Fake-pi-RPC/Stop grün.
  Das ist nicht die erweiterte 50er-S3-Suite (S3 berichtet diese separat).
- Logs `/tmp/I2-S7-{node,integration,v1}.log`; keine privaten Daten/Logs committed.
- `git diff --check` in beiden Worktrees grün; beide abschließend sauber.

## Abgleich / offene Entscheidungen und Grenzen

1. **S3-Integration auf Quellstand nachgewiesen**, kein K-Merge selbst ausgeführt.
   Frühere Meldungen in `I2-S7-sync-schnittstellen.md` und erster Hälfte des
   Abgleichs sind historisch: Verschachtelte Signatur und fehlender Archivweg
   inzwischen behoben. S8/S3 auf flachen Vertrag hingewiesen.
2. **K-Änderungsbitte: signierter Recovery-/Statusvertrag.** S3 lehnt gleiche
   Belegung/Freigabe nach Erfolg als Replay ab. Verlorener HTTP-Erfolg vor lokaler
   Speicherung kann Slot dauerhaft sicher sperren. Kein riskantes automatisches
   Wiedervergeben. Reservierungen nach Aktivierungsabbruch ebenso nicht blind
   freigeben. Vertrag additiv mit K einfrieren; keine fremden Dateien geändert.
3. **S4:** Proxy muss `/v1/belegung`, `/v1/freigabe` und `/v1/sync/dienst/*`
   auch unter Slotpräfix von außen sperren. S7 allein kann einen Loopback-Proxy
   nicht von internem Empfang unterscheiden; Signatur schützt zusätzlich.
   Echter sudo-/systemd-/UID-Leeren-Nachweis und Quotenprüfung noch offen.
4. **Speicher:** erzwungenes Archiv/verschlüsseltes Abholfach noch vollständig im
   Empfang-RAM (bis 2 GiB plus Chiffrat/Base64/JSON); vor Pilot streamingfähig
   machen oder strikteres Betriebslimit festlegen. Große Dateigrenze/Explizitwahl
   funktional vorhanden, kein realer 200-MiB-/2-GiB-Lasttest ausgeführt.
5. Import schützt statische Pfad-/Linkangriffe und Überschreiben; kein Anspruch
   auf racefreie Sandbox gegen gleichzeitig mutierenden lokalen Angreifer/gleichen
   UID-Prozess. Auch Hub kann ohne dirfd/openat keine solche harte Garantie geben.
   Streaming-Writer/Reader sind minimaler v1-ustar-Teilumfang, kein allgemeiner tar.
6. Gast-Abholfach hat keinen erneuten Gast-pid-Wiederherstellungsweg; regulärer
   OIDC-pid funktioniert. Code/Schlüsselverlust braucht eigenen Recoveryvertrag.
   Replaymarker im Hub überleben Neustart, nicht S4s vollständiges Home-Leeren.
7. Keine echte authentik-/Systembrowser-/PKCE-UI-Kette (S3/S2), kein TLS/Proxy,
   keine echten Arbeitskonten, keine Desktop-/Mobil-/Mac-/Windows-UI-Abnahme,
   keine Modelle, kein Release-/Workspace-Gesamtbuild/Installation. Onlinebetrieb
   und harte DSGVO-/Lösch-/Sicherheitsabnahme werden ausdrücklich nicht behauptet.

## Nächste drei Schritte

1. K: S3+S7 integrieren, Lockfile additive Änderungen zusammenführen, S6-Gates
   auf gemeinsamem Commit wiederholen (nicht nur lose Worktree-Kombination).
2. K/S3/S4: Replay-/Recoveryvertrag, internen Proxyschutz und echten Leernachweis
   mit sicherem Dry-Run prüfen; große Abholfächer vor Pilot streamen/begrenzen.
3. S2: produktive Browser-/Empfangs-/Auswahl-/Archivimportkette mit SHA256-Callback
   und ausdrücklicher Großdateibestätigung verbinden; dann separater Gerätetest.
