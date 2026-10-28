# I2-S3-belegung · Abschlussbericht

Stand: 28.10.2026, 22:18 Uhr. Arbeit ausschließlich in `v3/s3`, pi-lite `/home/medienhaus/Work/pi-lite-wt/s3`, pi-hub `/home/medienhaus/Work/pi-hub-wt/s3`; keine Pushes/Merges, Installationen, echten Konten oder Dienste geändert.

## Ergebnis und Checkpoints

- Hub **`229f68e`**: Signierte Belegung/Freigabe, Gerätewiderruf, bestätigtes Prozessende, Senderanzeige, optionaler Sync.
- pi-lite **`fe56fbb`**: Empfang-Client, öffentliche Signaturprüfung und OIDC-Loopback.
- Hub **`b1eef55`**: Beide Empfangssignaturformate, vollständiger `createSync`-Kontext, signierter S7-Dienstarchivpfad.
- pi-lite **`9b7aad5`**: Empfang-Challengevertrag abgeglichen, additive Protokolldokumentation/Schema/neun Beispiele.
- Abschluss pi-lite **`211036f`**: Gerätebindungs-Negativtests, aktueller flacher S7-Erzeugervertrag dokumentiert.
- Abschluss Hub **`8658ec2`**: Loopback-/Forwarded-Header-Schutz und Senderanzeige getestet.
- Beide Arbeitsbäume nach diesen Checkpoints sauber (`git status --short` ohne Ausgabe).

### Hub

Ed25519 über kanonisches JSON, Pin `HUB_EMPFANG_PUBKEY`, eigener `HUB_SLOT`, sichere Ganzzahlen; falsche Signatur/Slot/abgelaufene Belegung werden verworfen. Check-in-Replaymarker persistent gehasht, auch nach Neustart und Freigabe. Belegung registriert genau signierte Geräte; Zugriff nur bei gültiger aktiver Belegung. Freigabe widerruft alle Geräte, Tokens/Challenges/SSE, sperrt neue Aufträge, beendet gehaltene pi-Kinder mit SIGTERM, nach **10 Sekunden SIGKILL**, und bestätigt erst nach Exit aller Kinder. Nach weiteren 5 Sekunden ohne Nachweis Fehler statt Erfolg. Sessionregister und Puffer danach geleert. Kein Ersatz für S4s UID-weite Prozess-/Dateibereinigung.

Interne Belegung/Freigabe nur über echten Socket `127.0.0.1` bzw. dessen IPv4-Mapping; gefälschte Forwarded-Header helfen nicht. Proxy muss diese Pfade **auch unter Slotpräfix** blockieren. `von:{person,rolle}` ist Anzeige, kein Rollen-/Identitätsbeweis.

S7-`handleSync` wird vor dem JSON-Leser aufgerufen. Lazy `createSync` erhält private Ablage und die vier gefrorenen Wurzeln; fehlt Modul/Ablage, 501. Vorhandene Importfehler werden nicht verschluckt. S7s signierte `/v1/sync/dienst/archiv[/:id]` erhalten ohne Geräte-Bearer den Empfangspin, Slot, aktive Geräteablage und Uhr; Hub prüft zusätzlich echten Loopback. S7 prüft Signatur, 60-s-Zeitfenster, persistent einmalige Nonce und Belegungsbindung. Kein beliebiger Dateizugriff/Modellrecht. **S4 muss auch `/v1/sync/dienst/*` am Proxy sperren.**

### Rust

Öffentliche Pfade ohne Änderung an fremder Crate-Wurzel:

- `pi_backend::hub_v1::empfang::{Empfang,verify_signed,canonical_json}`
- `pi_backend::hub_v1::oidc::{Discovery,Login,pkce_challenge}`

Empfang nutzt denselben gehärteten Agent (kein Proxy, keine Redirects, Antwort-/Zeitlimits), Check-in 201/202, FIFO-Polling, Checkout/Bestätigung und Status. `uebernehmen` bindet Belegung und Ausweis an bestätigten Empfangspin, Gerät, Person/Rolle, Ablauf, Slotpfad; danach je geschützter Anfrage frische Empfang-Challenge mit X-Cocopi-Headern. Kein Token/OIDC-Inhalt in Debugausgaben.

OIDC: Discovery-Issuerbindung, validierte sichere Endpunkte, kein HTTPS-Downgrade, PKCE-S256-Schnittstelle, sichere zufällige Verifier/state/nonce, System-Browser nur per App-Callback, einmaliger Rückweg `127.0.0.1:0`, Host/Pfad/state-Prüfung, begrenzte Header/Lesezeit, deutsche minimale HTML-Seite, Code-Tausch ohne Clientsecret. Empfang prüft JWT/JWKS/nonce und Rolle; Desktop vertraut keinem selbst decodierten Token.

**Verbleibende Integrationsgrenze:** SHA-256 wird als Callback aus einer geprüften Bibliothek verlangt. Keine zusätzliche Abhängigkeit und kein eigener Hash implementiert. Der RFC-PKCE-Test prüft bekannte SHA256-Bytes/Kodierung, nicht einen eingebauten Hash. Antrag an K: direkte, bereits transitive Abhängigkeit `sha2="0.10"` plus Erlaubnis für Cargo.toml/Lockfile. Bis zur Freigabe/echtem Callback nicht als vollständig integrierte Anmeldung ausweisen. S2s produktive Browser-/Empfangsanbindung bleibt eigener Integrationsschritt.

### Endgültiger S7-Abgleich / S6-Blocker

S7 änderte während des Abgleichs den Erzeuger wieder auf **flache** signierte Nutzdaten. Das funktioniert mit S3 unverändert; **beide** Formate werden geprüft: flach `{...inhalt,signatur}` und strikte Alternative `{inhalt,signatur}` ohne zusätzliche Hüllfelder. Dokumentation entsprechend korrigiert. Rust prüft echte in Node erzeugte Signaturen aus Beispielen. S7s Challenge-pro-Anfrage-Vertrag funktioniert ebenfalls; dessen spätere optionale Bearer-Anmeldung muss der Client nicht verwenden. Alle drei von S6 gemeldeten Vertragsblocker (Hülle, Challenge, fehlender Sync-Kontext) sind damit adressiert.

**S7-Bitte Idempotenz:** Wiederholung derselben Belegung/Freigabe bleibt absichtlich `verbraucht` statt erneut 200, gemäß explizitem Replay-Auftrag. Keine stillschweigende Änderung dieses Vertrags; falls Empfang nach verlorenem HTTP-Erfolg idempotentes Recovery braucht, bitte K separat einen signierten Status-/Recoveryvertrag einfrieren lassen.

## Nachweise

- `node test/v1.mjs`: aktuell **50 Prüfungen grün**, ohne optionales Modul inklusive 501-Fallback.
- Zusätzliche **Temp-Kopie** (kein S7-Worktree verändert): S3-Hub + lesend kopierter S7-Sync/Empfangstand, derselbe Test **50 grün**. Prüft echten HTTP-Sync-Kontext, Vorschau, Archivmetadaten, gzip-Download/Hash, signierten Dienstauftrag ohne Bearer, manipulierte Signatur und Nonce-Replay. S7-Basis `e20ffc6` plus damalige lokale Änderungen, also ausdrücklich kein bloßer Commitstandnachweis.
- In derselben Temp-Kopie `node --test empfang/test/empfang.mjs`: **8 Tests grün** (Fake-OIDC RS256/ES256, Pool/Warteschlange, Verschlüsselung/Leeren-Mock, Dienstarchiv, Empfang-HTTP). Kein echter Root-/Leervorgang.
- `CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s3 CARGO_BUILD_JOBS=3 cargo test --offline --locked --manifest-path copi/Cargo.toml -p pi-backend hub_v1 -- --test-threads=1`: **16 Tests grün**, keine Warnungen im letzten Lauf. Empfang-Pin/Gerät/Rolle/Slotpfad/Ablauf, Node/Rust-Signaturen, Challengeheader, 201/202, lokale Discovery/Token-/Redirectmocks, falscher state/einmaliger Callback/Timeout, RFC-Verifiergrenzen und bisherige Hubtests.
- Python `jsonschema` Draft 2020-12: Schema und **alle 32 Beispiele gültig**, einschließlich neun neuer Beispiele; erneut geprüft 22:19 Uhr.
- Hubtest ergänzt Anzeige-von ohne Rechte und negative Rollenformatprüfung, gefälschte Loopback-Forwarded-Header, Fake-pi auch unmittelbar vor Freigabe lebend; SIGTERM/SIGKILL separat mit eigenen Testkindern.
- Logs `/tmp/I2-S3-{cargo-test,hub-test,S7-hub-test,S7-empfang-test}.log`; flüchtige Testprotokolle, nicht ins Repo übernommen.

## Sicherheits-/Abnahmegrenzen

1. **Replay über vollständiges Home-Leeren:** Marker liegen nach A8 im mitgelöschten HUB_STATE. Sie schützen Neustart/Freigabe, nicht das völlige Neuaufsetzen des Homes. Stärkerer Schutz braucht außerhalb liegenden Marker oder Slot-Epoche (K/S4-Entscheidung). Keine unautorisierte Systemablage eingeführt.
2. Kein echtes authentik, externer Netzwerkzugriff, System-Browser, TLS-Proxy, R2-Leeren, echte Toolprozessbäume, UI/Mobilgerät oder Modellchat getestet. SIGKILL-Testfristen werden nur im Unit-Test verkürzt; Produktion bleibt 10 s.
3. `verify_signed` prüft Herkunft/Integrität, vergibt allein keine Rechte. S8 prüft Zweck, Rolle, P2P-Schlüsselbindung und Zeit; bestätigter Pin nicht blind aus Empfangshallo übernehmen.
4. Kein vollständiger Workspace-/Release-/UI-Build behauptet; nur zielbezogene Offline-Debugtests. Keine Abhängigkeiten hinzugefügt, Cargo.toml/Lockfile unverändert.
5. Nur eigene Testkinder beendet, keine anderen Agenten-/Nutzerprozesse. Hub-Gitidentität per einmaligem `git -c user.name/email` aus vorhandener pi-lite-Konfiguration übernommen, keine Konfiguration geändert.
