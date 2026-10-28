# I1-S5-fix · M3-Signatur und H1/H2-Abnahme

Stand: 28.10.2026, 20:44 CET. Worktree `/home/medienhaus/Work/pi-lite-wt/s5`, Branch `v3/s5`, abschließend sauber. Auftrag innerhalb 40 Minuten.

## Commits / Dateien

- Einstieg: `git merge --no-edit cocopi-v3` → Fast-forward `847e42b` auf `83fe1fb`.
- `8abb9b5` — cocopi v3 I1-S5-fix: Anmeldung an Hub und gespeichertes Konto binden
- `ac286c9` — cocopi v3 I1-S5-fix: Kontextwechsel und wartende Einladungen nachweisen
- Eigene geänderte Pfade: `ui/transport.js`, `mobile/app.js`, `mobile/device.js`, `mobile/sw.js`, `mobile/README.md`, `mobile/test/{device,integration-s3,v1,signature}.mjs`.
- Keine Hub-/Desktop-/Protokolldatei geändert, kein Push. Hub-Worktree lesend geprüft: `e2eabfa`, sauber.

## Ergebnis und Nachweis

- **M3:** Exakte UTF-8-Signatur `cocopi-v1-anmelden\n<hub_origin>\n<slot>\n<geraet_id>\n<challenge_id>\n<nonce>`, kein Schluss-LF. Ursprung ausschließlich `new URL(baseUrl).origin`, nicht aus Challenge/Serverantwort; Slot und Geräte-ID aus gespeichertem Konto. Erst nach bestätigter Einlösung werden Ursprung/Slot/Geräte-ID in IDB abgelegt. `hallo` und Challenge müssen zum gespeicherten Slot passen. Alte Registrierung ohne Kontext blockiert mit Hinweis auf erneutes Koppeln. App-Hüllen-Cache auf v2 erhöht.
- **Hub A/B:** Test `Integration M3: Signatur für Hub A wird von Hub B abgewiesen` reicht echte B-Challenge über Testserver A weiter: Anmeldung wird mit 401 `nicht_angemeldet` abgewiesen. Kontrolle mit B-Ursprung und identischem Schlüssel/Gerät funktioniert.
- **Signaturvektor:** `M3: Signaturbytes entsprechen dem deterministischen Protokollvektor` prüft `device.js`/WebCrypto gegen die gemeinsamen Protokollbeispiele. Vier weitere M3-Tests blockieren fremden hallo-Slot, Challenge-Slot, fehlenden Kontext und fremden gespeicherten Ursprung jeweils vor der Signatur. Mock-Challenge mit fremdem `hub_origin` beeinflusst Signatur nicht.
- **H1:** Integration gegen S3 `lib/hub-v1.mjs` @ `e2eabfa`: k-Kopplung sowie e-Einladung mit `geraeteart:handy` warten. Ohne bestehendes Handy bestätigt/ablehnt der Desktop; Ablehnung legt nichts an. Mit bestehendem Handy bleibt e-Einladung bis b-Bestätigung offen, Geräteliste vorher unverändert. Gemeinsamer Client-Einlöseweg pollt auch bei e, statt Pending als Ergebnis zu speichern.
- **H2:** Derselbe Integrationstest erhält b-Code/URL nur im Einlöse-HTTP-Callback. Tatsächlicher Live-SSE und Replay enthalten die wartende Anfrage, aber weder b-Code, URL noch `bestaetigung`-Objekt. b-Einlösung im Transport mit Authorization-Header geprüft. Kein Bezug von b-Codes aus SSE im Client.

Prüfbefehle:

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/share/mise/shims:$PATH
export CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s5 CARGO_BUILD_JOBS=3
node --test mobile/test/*.mjs
for f in ui/transport.js mobile/*.js mobile/test/*.mjs; do node --check "$f" || exit 1; done
git diff --check
```

**25 Tests bestanden, 0 fehlgeschlagen, 0 übersprungen.** Syntax und Diffcheck grün. Testlog `/tmp/I1-S5-fix-tests-final.log` (flüchtig). Kein Cargo-Build erforderlich. Eigene kurzlebige Loopback-Testserver, Temp-Ablage/IDB-Dummy, nur Mock-Adapter; keine Echtdaten, Modelle, Nutzerprozesse oder Dienste berührt.

## Grenzen / offene Fragen / Änderungsbitten

- Keine Browser-/Geräte-/PWA-/Kamera-/TLS-Abnahme, keine Screenshots. Node-WebCrypto/IDB-Dummy beweist keine Browser-IDB-Persistenz. Keine echten Modelle.
- Bereits bekannte I1-Grenze: Mobilansicht bietet b-Aktionsvorschau/Anzeige/Bestätigung noch nicht an. Der Transport kann b bestätigen (im Test nachgewiesen), die UI lehnt b-Eingabe weiterhin ehrlich ab. Neue e-Einladung bei vorhandenem Handy wartet deshalb sichtbar, braucht aber einen anderen unterstützten Bestätigungsclient. Für weiteres Handy den k-Code des angemeldeten Desktops nutzen. **S1/S3/K:** authentisierte Aktionsvorschau und b-UI als separaten A3-Auftrag festlegen; kein neuer Protokollanspruch in diesem Fix.
- Alte Nonce-only-Registrierungen brauchen bewusstes erneutes Koppeln; keine automatische Kontextmigration und keine automatische getrennte Schlüsselablage je Hub/Slot eingeführt.
- **K/S3:** Übergabe in fremden `docs/HANDOFF.md` bzw. Hub-STATUS/TODO bei Konsolidierung nachtragen (hier wegen Pfadbesitz nicht verändert). Normativer Vertrag bereits durch S3 aktualisiert; dieser Client verlangt zusätzlich die gespeicherte Slotbindung wie beauftragt.

Ergebnis: **M3/H1/H2 im beschriebenen Mock-/HTTP-Testumfang nachgewiesen**, keine Pilot-/Betriebsfreigabe.
