# I1-S5-mobil · Abschlussbericht

Stand 28.10.2026, Branch `v3/s5`, Worktree `/home/medienhaus/Work/pi-lite-wt/s5`. Pflicht-FF auf `cocopi-v3` (`e0d9280`) vor Arbeit ausgeführt. Nur `mobile/**` und `ui/transport.js` geändert. Keine Pakete installiert, keine Echtdaten/Modellaufrufe, kein Push/Merge zurück, keine fremden Prozesse/Dienste verändert.

## Commits

- `852f4c6` — Header-Transport und Replay mit Mock prüfen.
- `a272feb` — Lesende Mobilansichten und v1-Kopplung absichern.
- `847e42b` — Fremden Gerätewiderruf im Integrationstest abgrenzen.

Worktree am Abschluss sauber. Kein Rust geändert/gebaut.

## Dateien / Ergebnis

- `ui/transport.js`: Tauri-freies ES-Modul `createTransport({baseUrl,getToken})`, geforderte sechs Methoden; zusätzlich optionale Testzeitparameter. fetch-SSE ausschließlich Bearer-Header, Epoche:Sequenz-Replay, Deduplizierung, sichtbare Lücke/Neuabgleich, 45-s-Pulsüberwachung, 401 → abgemeldet. QR-Parser Fragment/Query/Rohcode. Pfade und Base64url gegen S3 v1 abgeglichen.
- `mobile/{index.html,app.js,style.css,state.js}`: M-LI, M-SE read-only, M-EI eigenes Gerät/Abmelden, M-QR Eingabe und optional BarcodeDetector-Kamera. Ruhiges Layout, Konto/R2/Verbindung oben, Safe Area, Ziele ≥44 CSS-px, Texte nach A3, keine Sendefunktion/Platzhalter für I2. HTML/Medien aus Hubdaten werden nicht ausgeführt/extern geladen.
- `mobile/device.js`: WebCrypto Ed25519, privater CryptoKey `extractable:false` in IDB, öffentliche Schlüssel/Signaturen Base64url. Kein unsicherer Ersatz: fehlende API/IDB blockiert mit Erklärung. Token nur RAM. Browser-IDB selbst nicht geprüft.
- `mobile/{manifest.webmanifest,sw.js,icons/*,fonts/*}`: reine App-Hülle, feste Cache-Allowlist ohne API/Inhalte/Query/Auth-Requests, kein Laufzeitcache. SVG aus vorhandenem cocopi-Asset, 192/512-PNG mit vorhandenem librsvg/Cairo abgeleitet; PNG tatsächlich angesehen. Figtree + vorhandene OFL.
- `mobile/README.md`: Start-/Routingbitte an S3 und genaue Grenzen. `mobile/TAURI.md`: ausschließlich Plan für Hülle/Rechte/Keychain/Keystore/Verteilung; Fragment bleibt Standard, Query nur kompatibler Eingang, Plattformnachweis offen.
- `mobile/test/{transport,v1,mobile,device,integration-s3}.mjs`: 19 Tests, keine Abhängigkeiten.

## Nachweise

Ausgeführt mit Node `v22.22.1`:

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/share/mise/shims:$PATH
node --test mobile/test/*.mjs
find ui mobile -type f \( -name '*.js' -o -name '*.mjs' \) -print0 | xargs -0 -n1 node --check
git diff --check
```

**19/19 grün, 0 übersprungen.** Letzter TAP-Nachweis `/tmp/I1-S5-mobil-tests.tap` (flüchtig). Besonders:

- `SSE: Replay mit Last-Event-ID ohne Dubletten und ohne Lücken`;
- `SSE: Sequenzlücke wird sichtbar statt unvollständiger Erfolg`;
- `SSE: 45-s-Pulsgrenze (im Test verkürzt), Inhalte ersetzen keinen Puls`;
- `401 meldet alle Ereignisströme ab, kein endloser Neuversuch`;
- `v1: neue Hubepoche mit explizitem Neuabgleich ersetzt Cursor`;
- `Service Worker: statische Allowlist, keine API-/Laufzeit-Cachewrites` (Quellprüfung + VM-Ausführung mit Cache-Dummy);
- `Geräteschlüssel: Ed25519 nicht exportierbar, IDB-Clone, Base64url und lokale Abmeldung` (Node-WebCrypto + IDB-Dummy);
- `Integration S3-v1: Ed25519-Anmeldung, Kopplung, Sessions/SSE und eigener Widerruf`: importiert S3 `/home/medienhaus/Work/pi-hub-wt/s3/lib/hub-v1.mjs`, eigener zufälliger Loopbackport/Temp-Register/Mock-Adapter. Auch Widerruf **anderer** Geräte meldet dieses Handy nicht ab. S3-Hubstand bei Abschluss `60eb4a9`; uncommittete `server.mjs` nicht ausgeführt/verändert.

## Geprüft / ungeprüft

Geprüft: Transport-/Parser-/Zustandslogik, Headerauth, Replay/Funklochsimulation, Challenge/Kopplung/Widerruf gegen S3-Hubmodul, Shell-Cachegrenze, JS-Syntax aller `ui`-/`mobile`-JS-Dateien.

**Nicht nachgewiesen:** Browserdarstellung/Screenshots (kein Chromium), reale IDB-CryptoKey-Persistenz im Browser, Kamera, Service-Worker-Lifecycle/PWA-Installation, iOS/Android/Zurück-Geste/Safe Area/Screenreader, echte Netzunterbrechung/TLS/Hostpinning, native Hülle, Push. Keine Desktop-/Mobil-Gesamtabnahme behauptet.

**I1-Verlauf nur teilweise:** v1 hat keinen Verlauf-Snapshot-Endpunkt. M-SE zeigt empfangene vollständige `message_end`-Nachrichten seit Verbindung, nicht die vollständige alte/ruhende Session. Das steht sichtbar in der Ansicht. Deshalb I1-Ziel „Verlauf“ **nicht vollständig erfüllt**. Ruhende Session wird nicht schreibend aufgeweckt.

## Änderungsbitten / offene Fragen

1. **S3, für nutzbare PWA nötig:** gleiche-Origin-Auslieferung `/m/` (App-Allowlist + sw.js) und `/ui/transport.js`, MIME korrekt; `/c` statische bewusste Übergabe ohne automatisches Einlösen. Keine Code-/Tokenlogs, Queryzugriffe redigieren. Details README. Nichts dafür im fremden Hub geändert.
2. **S3/S1/K, I1-Abnahmeblocker Verlauf:** lesenden Verlauf-Snapshot mit Cursor anbieten/festlegen; Snapshot+SSE ohne Lücken/Dubletten zusammenführen. Kein existierender Endpunkt wurde erfunden. S1 entscheidet, ob die explizit partielle I1-Anzeige vorerst genügt.
3. **S1/S6:** Screenshots 390×800/1100×700, Laientest und echte Browser-/Geräteprüfung für die vier Ansichten nachholen; Quell-/Nodeprüfungen ersetzen das nicht.
4. **S1/S3:** `b` wird geparst, aber noch nicht ausgeführt. Vor Freigabe braucht es authentisierte Aktionsvorschau aus dem Hub (kein QR-Freitext), separate Zustimmung. Fragment-vs-Query auf echten Kamera-Intents noch offen; Empfehlung Fragment beibehalten, keine Formatänderung ohne K.
5. **S3:** Handyseitiges serverseitiges Abbrechen offener Kopplungsanfrage fehlt; Zurück stoppt nur lokales Polling. Prüfwort I1 optional, nicht implementiert. 12-h-Token wird nicht automatisch erneuert; nach 401/neuem Öffnen „Jetzt neu verbinden“ via Challenge.
6. **K/Doku-Besitzer:** Schnittstellenstand bei Integration in `docs/HANDOFF.md`, Hub-STATUS/TODO und ggf. Drittanbieterübersicht vermerken (S5 besitzt diese Dateien nicht). Figtree-OFL liegt bei.

M-VO, Antworten/Senden/Stopp, Push/ntfy und native App sind bewusst nicht I1-Produktfunktionen. Keine Freigabe/Installation auf R2 oder Geräten vorgenommen.
