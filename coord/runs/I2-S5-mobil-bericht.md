# I2-S5-mobil · Abschlussbericht

## Stand / Commits
Branch `v3/s5`, Worktree sauber. Kein Push/Merge, keine neuen Abhängigkeiten, keine Dienste/System-/Kontodaten verändert.

- `6a7487c` Andocken, Sessionaktionen, Ergebnisvorschau und No-Retention.
- `3d8561f` Poolfreigabe/DOM-Abläufe prüfen, relative Slot-Shell, Doku.
- `459c260` Späte Antworten nach Abmeldung sperren, Schlüsselwechsel, weitere Regressionen.

## Dateien
`ui/transport.js`; `mobile/{app,device,workflow,state unverändert,sw}.js` (state.js nicht geändert), `mobile/{index.html,style.css,manifest.webmanifest,README.md}`; `mobile/test/{app,device,mobile,pool-s3,slot-shell,workflow}.mjs`.

## Ergebnis (nachgewiesen)
- Handy nutzt k-Kopplung am Slotpfad, kein eigenständiger Check-in. Eigener Widerruf/401 (auch ohne SSE) leert sofort RAM/DOM/Blob-URLs und startet IDB-Löschung aller Registrierung/Metadaten; nur CryptoKeys/pubkey bleiben. Exakter Text „Dein Rechner hat ausgecheckt. Dieses Handy ist abgemeldet.“ und „Erneut koppeln“. Schlüssel kann ohne Registrierung in Einstellungen neu erzeugt werden; bestätigtes manuelles Abmelden löscht ihn.
- Kurze Aufträge, Rückfragen confirm/select/input/editor und Stopp per A1. Pro Prompt UUID `client_id`, Knopf bis Bestätigung gesperrt; Funkloch/5xx sperrt weitere Prompts statt Wiederholung. Abbruch weiterhin möglich. Keine automatische Rückfragezustimmung. Späte Antworten dürfen nach Check-out nichts wiederherstellen.
- Ergebnisliste ausschließlich `persoenlich` unter `Ergebnisse/`, nur Lesepfad. Rasterbild-/PDF-/Textanzeige implementiert; PDF im Browser-Viewer, HTML/SVG ausschließlich Text, Web Share nur nach API-Verfügbarkeitsprüfung/Klick. Vorschau begrenzt auf 25 MiB. Dateilesepfad derzeit nur gegen In-Memory-Hub geprüft (Blocker unten).
- Seminarleitung sichtbar mit Name/Abzeichen/Farbmarke, auch S3-Event `cocopi_absender`.
- Tippflächen mindestens 44/48 CSS-px, Fokuszustände und aria-live/aria-busy. Relative Manifest-/SW-/Transportpfade bleiben unter `/slot/accountNN/`.

## Nachweise
- `node --test mobile/test/*.mjs`: **34/34 grün, 0 skipped**. Vollständiges lokales Log `/tmp/I2-S5-mobil-tests.log` (nicht dauerhaft).
- `mobile/test/pool-s3.mjs`: gegen `/home/medienhaus/Work/pi-hub-wt/s3` **`229f68e`** echte HTTP-/Signaturintegration: Empfangsignatur/Belegung → Desktopanmeldung → k-Kopplung/Zustimmung → Handyanmeldung → Prompt → Rückfrage Nein → Stopp → signierte Freigabe → 401/Logout, erneute alte Anmeldung abgewiesen. Ausschließlich Tempverzeichnis/Mockadapter/eigener Loopbackport.
- `mobile/test/app.mjs`: DOM-Vertragsdummy prüft Knopfsperre/Doppelclick, explizite Rückfrage/Abbruch, Seminarleitungsmarke, read-only Textvorschau, 401/Registrierungslöschung/Erneut koppeln sowie späte Promptantwort nach Check-out.
- `mobile/test/device.mjs`: IDB-Dummy/WebCrypto prüft nicht exportierbaren Schlüssel und ausschließlich `privateKey/publicKey/pubkey` nach Leeren.
- `mobile/test/workflow.mjs`: Client-ID, unklarer Versand, Slotpfad, Ergebnisfilter/A1/Einzeldatei gegen In-Memory-Hub, 401 ohne SSE.
- `mobile/test/slot-shell.mjs`: Allowlist unter Slotpräfix, einschließlich gemeinsamem Transport.
- `for f in ui/transport.js mobile/*.js mobile/test/*.mjs; do node --check "$f" || exit 1; done` und `git diff --check`: grün.

## Ungeprüft / Grenzen
- **Ergebnisfluss nicht Ende-zu-Ende am S3/S7-Hub abgenommen**: Lesepfad/Handyberechtigung fehlen noch. Kein Erfolg behauptet; UI zeigt Serverfehler.
- Kein Chromium im Arbeitsimage: keine Screenshots, kein Browser-/PDF-/Web-Share-/IDB-Clone-Nachweis. Keine echten iOS-/Android-, PWA-, Kamera-, Tastatur-/Screenreader-/Safe-Area-Tests. DOM-Dummy ist kein Layout-/Gerätenachweis. Keine Rust-Builds nötig/ausgeführt.
- Bei Funkloch ist Check-out nicht erkennbar; Daten bleiben als veraltet im RAM bis bestätigtem Widerruf/401/Seitenende. Keine Inhalte persistent. Separat geöffnete PDF-Tabs bzw. bewusst geteilte Kopien in anderen Apps kann cocopi nicht zurückrufen; Blob-URL des cocopi-Tabs wird widerrufen.
- Keine serverweite Exactly-once-Garantie, keine vollständige historische Chatliste; ntfy/Tauri-Mobile/Rechnerbestätigung b weiter offen.
- 401 wird konservativ immer als Abmeldung behandelt (auch Tokenablauf); dann neu koppeln.

## Änderungsbitten an andere Rollen (nicht selbst geändert)
1. **S3/S7 – P0 Ergebnisintegration:** S7 `064ae2a` lässt Sync bisher nur für Desktop zu; S3-Kontext besitzt noch kein `sync`. `createSync`/Handler verbinden; Handy darf nur Vorschau und Ergebnis-Einzeldatei lesen, niemals Upload/Archiv. Bitte additiven authentisierten `GET /v1/sync/datei?pfad=Ergebnisse/…` liefern (binär, korrektes MIME/no-store, nur sichere reguläre Dateien, keine Symlinks/Pfadausbrüche). Mobil implementiert diesen In-Memory-Vertrag, keine fremde Datei geändert.
2. **S3 – Doppelversand:** `client_id` additiv in v1 dokumentieren/serverseitig je Gerät+Session deduplizieren; gleicher ID/anderer Inhalt ablehnen. Client ist bereits geschützt, Hub berücksichtigt Feld bisher nicht.
3. **S3/S4 – Auslieferung:** App-Allowlist + sw.js unter `/slot/accountNN/m/`, gemeinsamer Transport unter `/slot/accountNN/ui/transport.js`, bewusste `/c`-Übergabe. Keine Tests/README/Repo-Dateien ausliefern. Proxy entfernt Slotpräfix nur vor API-Hub, Codequery nicht loggen. Details `mobile/README.md`.
4. **S3/S8 – Rollenherkunft:** S3 nimmt `von.rolle` derzeit aus Client-JSON an. A9-Zustimmung/verifizierte Rollenherkunft upstream absichern; Mobilmarkierung verleiht keine Rechte und verifiziert keine Rolle.
5. **K/S1/S6 – Abnahme:** Screenshots 390×800 und 1100×700 sowie echte Mobil-/PDF-/Share-/Barrierefreiheitstests nach Integration. `docs/HANDOFF.md` und Hub STATUS/TODO nur durch jeweiligen Besitzer um diesen Mobilstand ergänzen.

## Offene Fragen
- Lesepfad/Handy-Syncberechtigung wie oben bestätigen; kein vorhandener I1-Lesepfad im geprüften Vertrag gefunden.
- Dürfen separat im Browser geöffnete PDFs im Pilot eingesetzt werden (bewusste externe Kopie, nicht automatisch rückrufbar)? Verhalten ist in der Mobil-Doku offengelegt.
