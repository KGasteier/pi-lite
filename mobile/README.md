# cocopi mobil · I2

Reduzierte pi-lite-Weboberfläche für den eingecheckten Rechner: Sessions ansehen, kurzen Auftrag senden, Rückfragen beantworten, abbrechen und Ergebnisse ansehen/teilen. Kein lokales pi, kein eigener Handy-Check-in, kein Upload, keine Telemetrie oder neue Abhängigkeit. Gemeinsamer Tauri-freier Transport: `../ui/transport.js`.

## Andocken und Zustände

Am eingecheckten Rechner „Handy koppeln“ öffnen und dessen **k-Code** scannen/eintippen. Das Handy übernimmt keine Adresse aus QR-Daten und wartet auf die Zustimmung am Rechner. Kamera nur nach Klick, ohne Audio; bei Navigation/Verbergen stoppen die Tracks. Ohne BarcodeDetector/Kamera bleibt Eintippen verfügbar.

Die Kopplung gilt nur bis zum Check-out. 401 (auch ohne offenen SSE-Stream) oder eigener Widerruf zeigt:

> Dein Rechner hat ausgecheckt. Dieses Handy ist abgemeldet.

„Erneut koppeln“ führt zurück. Verlauf, Eingaben, Session-/Ergebnisliste, Vorschau, Blob-URLs, Token und Kontobindung werden verworfen. Eine IDB-Transaktion lässt nur `privateKey`, `publicKey`, `pubkey` zurück; keine Gerätenamen/Slot-/Geräte-ID. Nach bestätigtem manuellem Abmelden wird auch der Schlüssel gelöscht. Ohne aktive Registrierung lässt sich unter „Einstellungen“ der Geräteschlüssel bewusst neu erzeugen. Speicherfehler werden sichtbar gemeldet, nicht als erfolgreiche Löschung ausgegeben. Ein Netzverlust allein ist **kein** Check-out-Nachweis: letzter Stand bleibt als veraltet markiert. Ohne Netz kann das Handy den Check-out nicht erkennen; bei der nächsten 401 wird geleert.

## Sessionaktionen

- Kurzer Auftrag: maximal 4000 Zeichen, eigener `client_id` (UUID), POST `/v1/sessions/:id/senden`. Keine optimistische Erfolgsmeldung, keine automatische Wiederholung. Knopf und Eingabe bis zur Antwort gesperrt. Netz-/5xx-Fehler sperrt weiteren Versand: „Versand unklar. Nicht erneut senden. Prüfe den Auftrag am Rechner.“ Sessionwechsel hebt diese Sperre nicht auf. Bestätigte 4xx-Ablehnung entsperrt.
- Rückfragen: `confirm`, `select`, `input`, `editor`; Antwort mit vorhandener Frage-ID über `/antwort{frage_id,wert}`. Keine automatische Zustimmung oder vorgewähltes „Ja“. Unbekannte Frageform am Rechner beantworten.
- Abbrechen: `/stopp`; „Abbruch angefragt“ ist nicht „fertig“. Nur Hubzustand zählt. Abbruch bleibt auch bei unklarer Prompt-Versandlage möglich.
- `session_ereignis` mit `von.rolle=seminarleitung` wird als „Name · Seminarleitung“ farbig markiert. S3s `cocopi_absender` ebenso wie attribuierte `message_end` werden unterstützt. Die Rolle muss upstream geprüft sein; diese Anzeige ist keine eigene Rollenprüfung und erlaubt keine Übernahme.

## Ergebnisse – read-only

POST `/v1/sync/vorschau`: ausschließlich `persoenlich` mit sicherem `pfad` unter `Ergebnisse/`. Einzeldateien über den vorgeschlagenen additiven Lesepfad `GET /v1/sync/datei?pfad=Ergebnisse/…` mit Authorization-Header. Keine Archive/Uploads. Maximal 25 MiB pro Vorschau, Größe auch während des Lesens begrenzt.

Rasterbilder als lokale Blob-URL; PDF im Browser-Viewer plus „PDF im Browser öffnen“; Text/Markdown/HTML/SVG **nur als Text**, niemals aktives HTML. MIME-Prüfung für Rasterbilder/PDF, PDF-Magicsignatur. „Teilen“ nur bei `navigator.share` + `canShare({files})`, durch ausdrücklichen Klick. Textdateien werden als `.txt` geteilt. Teilen verlässt bewusst cocopi; Kopien in anderen Apps oder separat geöffneten Browser-PDF-Tabs kann cocopi nicht zurückrufen. Blob-URLs im cocopi-Tab werden beim Wechsel/Check-out widerrufen.

**Aktuelle Integrationsgrenze:** S7s Sync-Handler ließ beim letzten Abgleich nur Desktopgeräte zu, S3 hat noch kein eingebundenes Syncobjekt/Einzeldatei-Leseroute. Die Vorschau ist gegen den In-Memory-Vertrag geprüft, nicht als fertiger Hub-Dateifluss abgenommen. Änderungsbitten an S3/S7 stehen im Auftragsbericht.

## Authentisierung / Ablage

Ed25519-Challenge nach `shared/protocol/v1/PROTOKOLL.md`: exakte UTF-8-Signatur `cocopi-v1-anmelden\n<hub_origin>\n<slot>\n<geraet_id>\n<challenge_id>\n<nonce>`, kein Schluss-LF. Ursprung aus konfigurierter Adresse, Slot aus bewusst gespeicherter Registrierung; Wechsel wird vor Signatur abgewiesen. CryptoKey `extractable:false` per strukturiertem Clone in IndexedDB. Fehlendes WebCrypto/IDB sperrt mit Erklärung; kein exportierbarer/unsicherer Ersatz.

Token nur im RAM/Authorization-Header, nie URL/Cookie/Log. SSE per fetch, `Last-Event-ID`, Dubletten-/Lückenerkennung, Pulsgrenze 45 s. Keine Garantie vollständiger historischer Chats oder serverweiten Exactly-once-Versands. Die serverseitige Deduplizierung von `client_id` ist bei S3 angefragt. 401 führt konservativ immer zur Abmeldung und Neukopplung (auch Tokenablauf).

Keine Chats/Ergebnisse in IDB, localStorage oder Cache. Service Worker speichert nur feste öffentliche Shell-Allowlist, kein Laufzeit-/APIcache. Pfade in Manifest/SW/Transport sind relativ zum Slot; `/slot/account03/m/` bleibt innerhalb `/slot/account03/`. CSP, no-referrer, keine Fremd-CDNs. Nicht-exportierbar schützt nicht vor bösartigem Code desselben Ursprungs: minimale Auslieferung/XSS-Schutz bleiben nötig.

## Auslieferung – Änderungsbitte S3/S4

Auf demselben HTTPS-Ursprung hinter TLS/Tailscale bereitstellen:

- `/slot/accountNN/m/` → `mobile/index.html`;
- `/slot/accountNN/m/<App-Datei>` → explizite Allowlist aus `sw.js` plus `sw.js` selbst; niemals Tests/README/Repo-Dateien;
- `/slot/accountNN/ui/transport.js` → `ui/transport.js`, MIME `text/javascript`;
- `/slot/accountNN/c` → bewusste Übergabe nach `m/`, Fragment erhalten, nichts automatisch einlösen; Querycodes nicht loggen.

Der Proxy entfernt `/slot/accountNN` für Hub-API-Aufrufe. Loopback-Hub, keine CORS-/Legacy-Tokenfreigabe. Statischer Server allein liefert nur die App-Hülle. Keine Nutzerhubs/-sessions als Test verwenden.

## Nachweise

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/share/mise/shims:$PATH
node --test mobile/test/*.mjs
for f in ui/transport.js mobile/*.js mobile/test/*.mjs; do node --check "$f" || exit 1; done
git diff --check
```

34 Tests grün, 0 übersprungen: A1/QR/Signatur/SSE/IDB-Dummy; Prompt-Doppelclick, unklarer Versand, read-only Ergebnisfilter; DOM-Vertragsdummy für Prompt/Rückfrage/Stopp, Seminarleitungsmarke, Textvorschau und Abmeldung. Integration importiert S3 `lib/hub-v1.mjs` (Default `/home/medienhaus/Work/pi-hub-wt/s3`, überschreibbar `COCOPI_V1_TEST_MODULE`), nur zufällige Loopbackports, temporäre Ablage/Mockadapter. Signierte Belegung → k-Kopplung → Prompt/Rückfrage Nein/Stopp → signierte Freigabe → 401/Logout ist gegen S3 `229f68e` geprüft. Kein echtes Modell.

**Ungeprüft:** Browser-IDB-Clone, PDF-Viewer/Web Share, Kamera, PWA-Installation, Safe Area/Tastatur/Screenreader, echte iOS-/Androidgeräte. Kein Chromium im Arbeitsimage; keine Screenshots/visuelle Abnahme. DOM-Dummy ersetzt keine Browserprüfung (390×800/1100×700). Tippflächen CSS mindestens 44/48 px, Fokus und aria-live vorhanden; reale Bedienbarkeit offen. ntfy, weitere Rechnerbestätigung (`b`), vollständiger Altverlauf und Tauri-Mobile-Hülle weiter offen (`TAURI.md`).

## Herkunft

Darstellung/Begriffe aus pi-lite reduziert; kein lokales `invoke` kopiert. Icon aus `copi/app/assets/cocopi.svg`, Figtree Regular aus vorhandenen Assets (OFL unter `fonts/OFL.txt`). Kein Orca-Code übernommen. Nur eigene Pfade dieses Rollenauftrags geändert.
