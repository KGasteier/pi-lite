# I1-S2-desktop · Implementierungsbericht

**Stand:** 28.10.2026, 19:20 · Arbeitsbaum `/home/medienhaus/Work/pi-lite-wt/s2`, Branch `v3/s2`, HEAD `9c8857c`.

**Bewertung:** Native reduzierte Oberfläche und echte Hub-Anbindung geliefert und mit Mock geprüft. **Keine vollständige I1-Abnahme**: Die unten genannten Funktions-/Schnittstellenlücken bleiben offen. Keine R2-, Modell-, Kamera- oder interaktive Desktopabnahme behauptet.

## Commits

Eigene grüne Teilstände:

- `876e933` — cocopi v3 I1-S2-desktop: reduzierte Oberfläche, Modus und QR-Dialoge
- `9508d87` — cocopi v3 I1-S2-desktop: Dialogbedienung und scharfe QR-Screenshots prüfen
- `384e921` — cocopi v3 I1-S2-desktop: Lockfile für gemergten Hubclient aktualisieren
- `9c8857c` — cocopi v3 I1-S2-desktop: Hubadapter und SSE mit isoliertem Mock prüfen

S3 integriert durch `git merge --no-edit v3/s3`, zuletzt bis `64e7154`.
Mergecommits `efaac16`, `72c1d6f`, `cfcd277`; A2 war bereits mit `2001d5f` integriert.
S3s Backend-/Protokolldateien wurden ausschließlich gemergt, nicht von S2 editiert.
Keine fremden Änderungen gestagt, kein Push, Reset, Stash oder Deployment.

## Eigene Dateien

- `copi/app/src/desktop.rs`: neuer cocopi-Desktop, Zustände/Dialogs/Worker, Headless-Interaktionstests.
- `copi/app/src/desktop/hub.rs`: Adapter auf S3 `HubV1`/`DeviceKey`; kein eigener HTTP- oder Kryptografiepfad; Unit- und Node-Mocktest.
- `copi/app/src/{main,app,view}.rs`: Feature-gesteuertes Routing; cocopi startet nicht automatisch einen lokalen Agenten; copi behält seine Oberfläche.
- `copi/app/src/net.rs`: unlesbare Verbindungsliste erzeugt keinen lokalen Ersatz.
- `copi/app/src/headless.rs`: Nearest-Filter bei QR-Texturen berücksichtigen.
- `copi/app/Cargo.toml`, `copi/Cargo.lock`: freigegebenes qrcodegen 1.8.0 optional; S3-Client-Lockfile ergänzt.
- `copi/app/DESKTOP_V3.md`: Architektur, Ausführung, Invarianten und offene Grenzen.
- `THIRD_PARTY.md`: vollständiger Orca-MIT-Hinweis mit Commit und qrcodegen-MIT-Lizenz.

## Geliefert

- Rust/egui, keine Electron-/Webview-Hülle. Separate Seminaroberfläche ohne die alten Raster-, Backend-, Helper-/CLI- und Bookmark-Farb-Bedienelemente. Legacy-Code bleibt für copi erhalten, ist im normalen neuen cocopi-Desktop nicht erreichbar.
- Permanente klickbare Statuszeile für Modus, Person, Slot und Verbindung; keine zweite Modusschaltstelle. Nur neues Kürzel Cmd+, bzw. Ctrl+, für Einstellungen.
- Sechs Einstellungsbereiche Konto, Geräte, Modus, Darstellung, Vorlesen, Erweitert.
- Persistierter Modus über S3 `mode::set`; Hybrid mit wortgetreuem Hinweis und initial fokussiertem Abbrechen, Escape/Enter geprüft. Remote-only-Rückkehr ohne Bestätigung. Moduswechsel selbst startet/beendet keine Session.
- Einladungsstart, Anmeldebestätigung, exakte A3-Fehlertexte im Zustandsmodell. Einladungshost vor erstem Kontakt sichtbar bestätigen; kein stilles Vertrauen in eine Linkadresse.
- Gerätegebundene Anmeldung über S3 Ed25519/DeviceKey, Geräteauflistung und bestätigter Widerruf. Profil enthält nur Host/Gerätenamen, keine Tokens/Einladungen/Verläufe.
- Reales QR-Bild aus qrcodegen, vier Modulreihen Quiet Zone, Nearest-Textur, Ablauf/Countdown. Bestätigungs-QR für weiteren Desktop und Polling auf Zustimmung. Gerätefreigabe anhand Hub-Anfrage; Prüfwort fehlt sichtbar und ist gemäß Freeze in I1 optional.
- Hub-Sessionliste, Zustände, Herkunft R2, Anheften in der laufenden App, Öffnen/Ruhen/Senden/Stoppen über S3. Nur arbeitet nutzt #e454c4; bereit ohne Zustandsmarkierung. Limitdialog bietet nur fertige/bereite Kandidaten, kein stilles Ruhenlassen arbeitender Sessions/Rückfragen.
- SSE-Status/Stream auf separatem Worker; 15/45-s-Vertrag mit Veraltungsanzeige. Kein automatisches Prompt-Retry. Sendebestätigung löscht keine inzwischen neu bearbeitete Eingabe. Markdown-/Code-Rendering im Hauptchat.
- Chat/Dateien-Wechsel. Dateiaktionen ausdrücklich nicht verfügbar (I2), keine stillen lokalen Lese-/Uploadoperationen. Modellpräfix `udk2/…`, Verbrauch ehrlich unbekannt.

## Ausgeführte Prüfungen

Umgebung: Offline, `CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s2`, `CARGO_BUILD_JOBS=3`, Debug, keine Release-/Installationsänderung.

```sh
cargo test --offline --locked --manifest-path copi/Cargo.toml -p copi --features cocopi --bin cocopi
# 120 passed; 0 failed; 2 ignored
cargo test --offline --locked --manifest-path copi/Cargo.toml -p copi --bin copi
# 100 passed; 0 failed; 2 ignored
cargo build --offline --locked --manifest-path copi/Cargo.toml -p copi --features cocopi --bin cocopi
# erfolgreich (bestehende/Dead-code-Warnungen, kein fehlerfreier Clippy-Nachweis)
git diff --check
# grün; Arbeitsbaum nach Commit sauber
```

Lockfileauflösung nach qrcodegen/S3-Merge einmal bewusst ohne `--locked`, aber ausschließlich `--offline`; finale Prüfungen mit `--locked`.

Neue Desktop-Prüfungen umfassen: Statusklick, Settings-Kürzel, Hybrid Escape/default Enter, exakte A3-Texte, Widerruf, alle acht Szenen, Akzent, QR-Matrix/Quiet Zone, Workerfehler, SSE-Heartbeat ohne falsche Befehlsbestätigung, Stream-Isolation und Erhalt späterer Eingabe.

**Echter App-Adapter gegen isolierten Node-Mock:** eigener zufälliger Loopback-Port; eigener temporärer State/Schlüssel/Sessionroot; `HUB_TOKEN=demo`, Harness mock, kein ntfy. Einladung, Ed25519-Login, Geräteliste, Session öffnen/ruhen, Paircode und Widerruf erfolgreich. Nur die selbst gestartete Child-PID wird über RAII beendet. Keine echte Konfiguration und kein Modell. Dieser Test ist an den vorhandenen S3-Hub-Worktree gebunden und scheitert ehrlich, falls dieser fehlt.

Logs: `/tmp/s2-final-cocopi.log`, `/tmp/s2-final-copi.log`, `/tmp/s2-final-build.log`, `/tmp/s2-direct.log` (gezielte Desktop-/Mocktests).

## Screenshots

Alle erzeugt mit aktueller Debugbinary, 1400×880, reine Fixture-Daten:

`/home/medienhaus/Work/pi-lite/coord/shots/I1/`

- `v3-start.png`
- `v3-start-fehler.png`
- `v3-main-remote.png`
- `v3-modus.png`
- `v3-hybrid-dialog.png`
- `v3-geraete.png`
- `v3-koppeln-qr.png`
- `v3-grenze.png`

Start, Hauptchat, Modus, Hybrid, QR und Grenze visuell betrachtet; alle acht zusätzlich durch egui-Szenentest geprüft. Das QR-Bild hat sichtbare klare Module; **kein externer QR-Decoder/Kameratest** verfügbar. Die Matrixprüfung ersetzt keinen echten Scan. Der Fake-Link `example.invalid` belegt keinen erreichbaren Hub.

## Offene Lücken / Änderungsbitten an K und S3

1. **S3-Client verwirft HTTP-Fehlercodes:** `hub_v1::packet_error()` liefert noch nur `was tun`. App kann deshalb reale Einladungsfehler/Sessionlimit nicht zuverlässig den exakten A3-Zuständen zuordnen. Kein fragiles Text-Raten implementiert. S3 soll `[code] was tun` oder typisierte Fehler liefern; Adapter versteht Codes bereits. Anfrage: `coord/runs/I1-S2-desktop-anfrage.md`. Bis dahin exakte Texte und Limitdialog nur für strukturierte SSE-/Fixture-Antworten vollständig erreichbar.
2. **Hybrid ist als Modus persistierbar, aber lokale Sessions sind nicht integriert:** vorhandene lokale Agenten werden durch Wechsel nicht beendet, neue Oberfläche listet/bedient sie jedoch nicht. Keine vollständige Hybrid-Funktion oder lokale Herkunftsabnahme. Der Modusbereich nennt diese Grenze ausdrücklich.
3. **Chat-Parität unvollständig:** kein kompletter Verlauf nach Neustart/Reconnect, keine vollständigen Toolkarten oder Antwort-Bedienung für `extension_ui_request`; Rückfrage zeigt einen ehrlichen Hinweis statt automatischer Zustimmung. Kein lückenloses Replay/Exactly-once-Nachweis.
4. **Modellwahl fehlt:** `udk2/…` ist Präfix/Platzhalter, kein konkreter Servermodellname. Freigabelisten-Picker und echte Verbrauchswerte fehlen. Neue Oberfläche bietet kein Helfermodell.
5. **Gerätenamenänderung am Hub fehlt:** Eingabe vor Anmeldung verfügbar; nach Anmeldung wird fehlende Umbenennungs-API ehrlich gemeldet. Keine Persistenz am Hub vorgetäuscht. Anheften ist nur laufzeitlokal.
6. **Dateien, Vorlesen, Ressourcen** in neuer Oberfläche nicht angebunden. Dateiaktionen explizit I2; kein Upload. Darstellung nur Grundoberfläche, keine vollständige Einstellungsmigration.
7. Zweiter-Desktop-QR/Polling und SSE sind implementiert, aber nicht als kompletter Zweigeräte-/Handyablauf getestet. Ablauf/Abbruch mit echter Kamera, Funkloch, Tokenablauf und Replay separat abnehmen. I1-Schlüsseldatei 0600 statt Keychain laut Freeze; Keychain I2/I3.
8. Headless Linux und lokaler Mock sind keine macOS-/Windows-/ARM-/R2-/Mobilabnahme. Kein interaktives Fenster, keine echten Nutzergeräte, kein R2-Betrieb und keine Modellkosten ausgelöst.

## Empfohlene nächste Schritte

1. S3-Fehlercodes nachliefern, echte Adapter-Negativtests für Ablauf/Verbrauch/Limit/Widerruf ergänzen.
2. Lokalen Hybridadapter, Rückfrageantworten, Verlauf und servergemeldete Modellfreigabeliste auf derselben neuen Oberfläche vervollständigen.
3. Erst danach getrennt genehmigte Plattform-/QR-/Zweigeräte-Liveabnahme; nicht aus den grünen Fixture-Tests auf Seminarbetriebsreife schließen.
