# cocopi mobil · I1

Vanilla-JS-Mobiloberfläche, aus pi-lite `ui/` auf den eingefrorenen Umfang reduziert: Sessionliste (M-LI), lesender Verlauf (M-SE), dieses Gerät/Abmelden (M-EI), Kopplung per eingetipptem oder gescanntem Code (M-QR). Gemeinsamer Tauri-freier Transport: `../ui/transport.js`. Kein lokales pi, keine Telemetrie und keine Paketabhängigkeiten.

## Start lokal – Änderungsbitte an S3, noch keine vorhandene Hubroute

Der v1-Hub soll **auf demselben Ursprung** folgende öffentliche statische Dateien bereitstellen:

- `/m/` und `/m/index.html` → `mobile/index.html`;
- `/m/<App-Datei>` → explizite Allowlist aus `sw.js` (plus `sw.js` selbst), **nicht** Tests, README oder beliebige Repositorydateien;
- `/ui/transport.js` → `ui/transport.js`, MIME `text/javascript`;
- `/c` → statische Übergabeseite gemäß A3, die **nichts automatisch einlöst**, mit bewusstem Link nach `/m/` (Fragment bleibt erhalten). Keine Code-/Tokenlogs; Query-Variante nicht ins Accesslog schreiben.

Danach eigenen lokalen **Mock**-Hub auf `127.0.0.1:<freier Port>` mit `HUB_STATE=<Temp-Verzeichnis>` oder `off`, explizitem `HUB_ROOTS=<Temp-Projekt>`, `HUB_SLOT=account03` und Mock-Harness starten (konkrete CLI bei S3). `/m/` öffnen. Über den Admin eine Testeinladung ausstellen oder am Test-Desktop „Handy koppeln“ wählen. Es gibt keine eingebauten Zugangsdaten und keinen Demo-Token-Eingabetrick.

Ein bloßer statischer Server liefert die App-Hülle, aber **keine Anmeldung/Sessions**. Keine Verbindung zum Nutzerdienst, keine echten Sessions oder Modelle für Tests benutzen. Mobil im Betrieb ausschließlich HTTPS; HTTP akzeptiert der Transport nur für localhost/Loopback. Hub bleibt an Loopback gebunden. CSP und `referrer=no-referrer`, keine Fremd-CDNs.

## Protokoll und Schutz

Abgeglichen mit S3 `shared/protocol/v1/PROTOKOLL.md`/`schema.json` vom 28.10.2026:

- `POST /v1/hallo`, Einladung/Kopplung über `/v1/einloesen`; Koppeln wartet per `/v1/koppeln/:anfrage_id` auf Desktopfreigabe.
- Ed25519-Challenge über `/v1/anmelden/challenge` + `/v1/anmelden`; öffentliche Schlüssel/Signaturen Base64url. Token nur im RAM, ausschließlich im Authorization-Header, auch SSE per fetch-Stream (`/v1/ereignisse`), nie EventSource-URL/Cookies.
- `Last-Event-ID: <epoch>:<seq>`; Dubletten verworfen. Unangekündigte Sequenzlücke wird sichtbar unterbrochen. `neu_abgleichen` setzt den Cursor zurück; Sessionliste ersetzt die alte. Das ist **keine** Garantie vollständiger alter Chat-Historie.
- Puls 15 s vom Hub, Überwachung 45 s. Ohne Puls wird der Stand als veraltet angezeigt, selbst wenn Inhalte weiter eintreffen. 401 leert private Anzeige und beendet Streams. Nach erneutem Öffnen/„Jetzt neu verbinden“ neue Challenge, kein neues Koppeln nötig.
- QR-Parser akzeptiert Fragment `#1.k.<code>`, Query `?c=1.k.<code>`, gruppierten/kompakten Rohcode. Keine Übernahme eines Hosts aus dem QR. Code wird erst nach bewusster Hauptaktion gesendet. Eingangsfragment/-query vor weiteren Requests aus der Adresszeile entfernt.
- Kamera ausschließlich nach Klick, `BarcodeDetector` + `getUserMedia`, Video ohne Audio, Tracks bei Navigation/Verbergen stoppen. Fehlt die API oder Erlaubnis: Eintippen, **keine** Bibliothek.
- Gerätewiderruf nur für dieses Handy. Erfolg erst nach Hubbestätigung; bei Funkloch bleibt der Schlüssel für einen erneuten Widerrufsversuch erhalten. Fremde Geräte kann Mobil nicht verwalten.

## Ablage / Fallback

`device.js` erzeugt Ed25519 mit `extractable:false`; privater CryptoKey und öffentlicher Schlüssel werden per strukturiertem Clone in IndexedDB gespeichert. Nur öffentlicher Schlüssel verlässt das Gerät. Gerät-ID nach Hubfreigabe lokal gespeichert, Token nicht persistiert. **Kein** localStorage, Inhalts-/Vorschau-/API-Cache. IDB speichert keine Chats.

Wenn WebCrypto-Ed25519, sichere Umgebung oder IDB/Clone fehlen: Anmeldung ist mit Erklärung **gesperrt**. Kein exportierbarer Schlüssel, kein gemeinsames Token, keine eigene Kryptografie als Ersatz. Empfehlung aktueller Browser oder später native Hülle. Nicht-exportierbar heißt nicht gegen bösartigen Code desselben Ursprungs geschützt; XSS könnte Signaturen anfordern. Browser-/OS-Schutz und minimale Auslieferungs-Allowlist bleiben erforderlich.

Service Worker cacht **nur** die feste öffentliche App-Hülle beim Installieren. Kein `cache.put`, kein Laufzeitcache, kein API-Fallback; Header-/Query-Requests werden nicht abgefangen. Neue Shellversion braucht erhöhten Cache-Namen. App startet offline ohne Inhalte und zeigt keinen lokalen Agenten als Ersatz.

## Nachweise (Node 22, keine Installation)

```sh
export PATH=$HOME/.local/share/mise/shims:$PATH
node --test mobile/test/*.mjs
for f in ui/transport.js mobile/*.js mobile/test/*.mjs; do node --check "$f" || exit 1; done
git diff --check
```

Integrationstest importiert optional S3s `lib/hub-v1.mjs` aus `/home/medienhaus/Work/pi-hub-wt/s3` (überschreibbar `COCOPI_V1_TEST_MODULE`). Er startet nur seinen eigenen HTTP-Server auf zufälligem Loopbackport, mit temporärer Ablage und einem kleinen Mock-Adapter, **nicht** `server.mjs` oder den Nutzerdienst. Fehlt das Modul, wird der Test ausdrücklich übersprungen.

Tests decken QR, SSE-Framing/Replay/Dubletten/Lücken/Neuabgleich/Pulsausfall/401, Challenge/Kopplung/Widerruf, Zustandstexte, Shell-Allowlist und CryptoKey-Vertrag (Node WebCrypto + IDB-Dummy) ab. Das beweist **keinen Browser-IDB-Clone** und keine Kamera-/Layout-/PWA-Funktion auf echten Geräten.

## Grenzen und offene Änderungen

- Kein Chromium auf R2: **keine Screenshots und keine Browser-/Geräteabnahme**. S1/S6 müssen 390×800 und 1100×700 sowie Tastatur/Screenreader, Safe Area, Zurück-Geste, installierte PWA und iOS/Android prüfen. Ziele CSS ≥44 px; physische Bedienbarkeit ungeprüft.
- **Historischer Verlauf unvollständig:** v1 bietet bisher keinen lesenden Verlauf-Endpunkt. Nur seit Verbindung empfangene vollständige `message_end`-Nachrichten erscheinen. Keine erfundene „vollständig“-Anzeige. Änderungsbitte S3: read-only Snapshot mit Cursor, damit Snapshot+SSE ohne Lücken/Dubletten zusammengeführt werden können; S1 entscheidet I1-Abnahme. Ruhende Sessions werden mobil nicht aufgeweckt.
- M-VO Datei-/PDF-/Bildvorschau, Senden, Rückfragen-Antwort, Stopp, Push/ntfy und Bestätigung eines weiteren Rechners (`b`) **nicht implementiert in I1**. Kein täuschend aktiver Platzhalter. `b` wird geparst, aber mit Erklärung nicht ausgeführt: Der Vertrag liefert noch keine vom Hub authentisierte Aktionsvorschau vor Freigabe (Änderungsbitte S1/S3).
- Kopplungseinladung nicht automatisch wiederholen, auch nicht bei unklarem Versand. Abbruch stoppt lokal das Polling; ein serverseitiger Abbruch für die anfragende Handyseite fehlt (S3 anfragen). Prüfwort I1 optional, nicht vorhanden.
- Automatisches Tokenerneuern im laufenden 12-h-Tab noch nicht implementiert; 401 zeigt abgemeldet, „Jetzt neu verbinden“ macht Challenge. Keychain/Keystore/Tauri: ausschließlich Plan in `TAURI.md`.

## Herkunft der Darstellung

Farben/ruhige Flächen und Session-/Verlaufsbegriffe aus pi-lite `ui/index.html`/`ui/app.js` reduziert; keine lokale `invoke`-Funktion kopiert. `icons/cocopi.svg` unverändert aus `copi/app/assets/cocopi.svg`, PNGs daraus mit vorhandenem librsvg/Cairo gerendert. Figtree Regular aus vorhandenen Appassets; SIL-OFL unter `fonts/OFL.txt`. Kein Orca-Code übernommen; QR-/Pulsabläufe nach eigenem A1/A3-Vertrag umgesetzt.
