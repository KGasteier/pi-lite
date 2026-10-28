# cocopi v3 I1-S2 · Desktop-Oberfläche

## Zuständigkeit und Invarianten

Die Seminaroberfläche liegt in `src/desktop.rs`; `src/desktop/hub.rs` adaptiert
S3s `pi_backend::hub_v1`, ohne eigenen HTTP-, SSH- oder Kryptografiepfad.
`copi` ohne Feature behält die vorhandene Oberfläche. Im interaktiven cocopi ist
die alte Oberfläche nicht erreichbar; ihre Modelle bleiben für copi und dessen
Regressionstests erhalten. Keine zweite Backendliste, kein Raster, kein lokales
CLI-/Hilfsmodell, keine Bookmark-Farbpalette und kein Herkunftsfilter im Seminar-UI.

`mode::init()` läuft vor jedem Backend-Aufbau beim normalen Start und bei
`--sync-states`, nicht in Demo-/Screenshot-Läufen. Die Startreihenfolge ist über
einen injizierten Initialisierer ohne echte Konfigurationszugriffe getestet.
Der Desktop startet beim Öffnen der App **keine** Session – auch nicht in Hybrid.
Die zentrale Startsperre liegt bei S3. `net.rs` verwendet bei unlesbarer Liste
keinen lokalen Ersatz mehr.

Moduswechsel erfolgen ausschließlich in Einstellungen › Modus. Der Statuszeilen-
Klick führt dorthin, nicht zu einem zweiten Modusschalter. Hybrid erfordert den
verbindlichen Dialog; Abbrechen ist initial fokussiert, Esc schließt ihn.
Das einzige zusätzliche Tastenkürzel ist Cmd+, bzw. Ctrl+, für Einstellungen.

## Verbindung und Profile

Eine Einladungs-URL wird nicht heimlich zum vertrauenswürdigen Hub. Vor dem ersten
Kontakt bestätigt die Person den angezeigten Ursprung. Rohcodes verwenden eine
vorher bestätigte Adresse (Einstellungen › Erweitert › Verbindung).
`desktop-v3.json` speichert nur bestätigte Adresse und Gerätenamen, keine Tokens,
Einladungen oder Verläufe. Private Schlüssel und Anmeldung gehören S3s DeviceKey.

Blockierende API-Aufrufe laufen in einem Worker. SSE-Ereignisse sind keine
Befehlsbestätigungen; nach 45 Sekunden ohne Ereignis ist die Verbindung veraltet.
Prompts werden nie automatisch wiederholt und bleiben bei einem Sendefehler im
Textfeld. „Verbunden“ ist eine Hub-Bestätigung, keine lokale Aktivitätsschätzung.

Das Pairing verwendet die echte qrcodegen-Matrix als egui-Textur mit vier weißen
Randmodulen und Nearest-Filter. Der headless Rasterizer respektiert diesen Filter.
Die Szene enthält nur einen erfundenen Einmalcode für `example.invalid`, niemals
ein Zugangstoken. Countdown und Codeablauf sind getrennt von Gerätezustimmung.
Fehlt das Prüfwort im Hub-Ereignis, wird dies ausdrücklich erklärt. Laut Freeze
ist es in I1 optional; die Person kann nach Prüfung des Gerätenamens zustimmen.
In I2 ist das übereinstimmende Prüfwort Pflicht.

Der QR-Schritt schließt das Einstellungsfenster. Abbrechen ist unabhängig von
Codeablauf und QR-Texturerzeugung erreichbar; beide Ablaufzweige sind per
simuliertem Klick getestet. Sprecher und Nachrichtentext sind getrennte Daten:
eine dezente Sprecherzeile und Innenabstand vermeiden verklebte Chattexte.
Sessiontitel, Zustand und Aktionsmenü stehen in einer Zeile; die ausgewählte
Session erhält einen durchgehenden neutralen Hintergrund.

## Tests und Szenen

```sh
export CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s2
export CARGO_BUILD_JOBS=3
cargo test --offline --locked --manifest-path copi/Cargo.toml -p copi --features cocopi --bin cocopi
cargo test --offline --locked --manifest-path copi/Cargo.toml -p copi --bin copi
```

Die alten Tests erzeugen bewusst die vorhandenen copi-Demos. Neue `v3-*`-Szenen
verwenden ausschließlich den neuen Desktop und seine Fixtures. Tests initialisieren
weder eine echte Konfiguration noch eine echte Verbindung. `DesktopApi` lässt sich
mit einem reinen In-Memory-Adapter ersetzen.

Szenen: `v3-start`, `v3-start-fehler`, `v3-main-remote`, `v3-modus`,
`v3-hybrid-dialog`, `v3-geraete`, `v3-koppeln-qr`, `v3-grenze`.

```sh
COPI_SCENE=v3-start /home/medienhaus/Work/pi-lite-wt/target-s2/debug/cocopi --shot /tmp/v3-start.png --size 1400x880
```

Auf dem isolierten Node-Mock wird zusätzlich der echte App-Adapter geprüft:
Einladung, Ed25519-Login mit temporärem DeviceKey, Geräteliste, Öffnen/Ruhen,
Paircode und Widerruf. Kein Modell und keine echte Konfiguration beteiligt.

Der integrierte S3-Client erhält bekannte HTTP-Fehler als `[code]`-Präfix; der
Adapter bildet sie auf lokale A3-Texte bzw. den Sessionlimit-Dialog ab. Ein echter
Loopback-HTTP-Test prüft abgelaufen/verbraucht/unbekannt/zu viele Versuche mit
absichtlich unpassenden Servertexten. Der Node-Adaptertest öffnet zwei Sessions
und prüft beim dritten HTTP-Versuch den tatsächlich gerenderten Limitdialog.
`COCOPI_TEST_HUB_DIR` kann den isolierten Mockpfad explizit setzen.

## Ehrliche I1-Grenzen

Dateitransfer, Vorlesen, Ressourcenmessungen und Modellwahl sind in dieser neuen
Oberfläche nicht angebunden. Die drei Dateiaktionen sind als noch nicht verfügbar
markiert. Die Modellzeile zeigt bis zur Anbindung der Freigabeliste „Modell: Standard“;
es wird kein konkreter Modellname behauptet. Verbrauch bleibt unbekannt. Keine erfundenen Messwerte.

Die neue Oberfläche erhält Sitzungstatus vom Hub; lokale Hybrid-Sessions aus der
vorherigen Oberfläche werden in I1 noch nicht aufgelistet. Umschalten beendet sie
nicht. Fragen aus `extension_ui_request` werden ehrlich als noch nicht beantwortbar
erklärt und niemals automatisch freigegeben. Für vollständigen Hybrid-/Rückfrage-
Umfang ist eine eigene Folgeaufgabe erforderlich.

Keine macOS-/Windows-Liveabnahme, keine R2-Verbindung, kein Kameratest und kein
Modellaufruf sind durch headless Tests oder Screenshots nachgewiesen.

S3 begrenzt den SSE-Body auf insgesamt 45 Sekunden (kein gleitender Idle-Timer).
Der Desktop zeigt das Ende als Verbindungsunterbrechung; automatischer Reconnect
mit Last-Event-ID/Deduplizierung ist hier noch nicht implementiert. Es werden
niemals Prompts automatisch wiederholt. Der lokale Schlüsselspeichername ist
noch nicht je Ursprung/Slot getrennt; die neue Signatur bindet den Ursprung und
prüft den Challenge-Slot gegen `hallo`. Beides bleibt vor Pilotbetrieb zu prüfen.
