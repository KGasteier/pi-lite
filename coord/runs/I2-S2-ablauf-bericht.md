# I2-S2-ablauf · Abschlussbericht

Stand: 28.10.2026 · Branch `v3/s2` · Basis `e8f000e` · Abschluss `40d873c`.

## Ergebnis und Grenzen

Die native Desktop-Kartenfolge **Check-in → Auswahl → Upload → Arbeiten → Vorschau → Rückkehr → Check-out** ist implementiert und mit einem reinen In-Memory-Adapter geprüft. Die echte Empfang-/Sync-Anbindung ist **offen**. Der produktive `HubAdapter` weist neue Ablaufaufrufe ausdrücklich ab; er meldet weder eine erfolgreiche Übertragung noch eine Löschung oder Freigabe. Es wurde kein eigener Empfang-HTTP-Client gebaut.

FREEZE-I2, Entwicklungsprompt und `docs/v3/FLOWS.md` wurden als Vorgaben verwendet. Keine festen persönlichen Slotkonten, keine zweite Identität aus dem Browser-Callback, kein automatischer Upload, keine lokale optimistische Freigabe eines belegten Arbeitsplatzes.

## Geändert — nur eigene S2-Quellen

- `copi/app/src/desktop.rs`: additive `DesktopApi`-Requests/-Replies und Kartensteuerung, Arbeitsstatus mit Person/Poolkonto/R2/Check-in-Zeit, eine Check-out-Stelle oben rechts im Arbeitsmodus, Chat/Dateien/Seminar, Cleanup nach bestätigtem Check-out; Generationen gegen verspätete Requests und Hub-Ereignisse.
- `copi/app/src/desktop/checkin.rs` (neu): typisierte Ablaufzustände und Adaptervertrag, Abbruch-/Preview-Regeln, reiner `MemoryApi`, sieben Modelltests.
- `copi/app/src/desktop/sync_ui.rs` (neu): Karten, Polling, Checkboxen/Größen/Cacheausschluss, Fortschritt/Abbruch, zweispaltige Vorschau, Abschluss/Konflikthinweis/Ordneröffner, Abholfach; statische Demo-Szenen ohne echte Aktionen.
- `copi/app/src/desktop/hub.rs`: neue Ablaufrequests fail-closed, bisheriger Hubvertrag erhalten.

Keine fremden Quellen, pi-sync, Shared-Protokolle, Hubserver oder S8-Dateien geändert. Keine Installation, kein Push/Merge, kein echter Modellaufruf, keine System-/Dienständerung, kein Eingriff in Nutzerfenster oder echte Konfiguration.

## Sichtbarer Ablauf

- Startkarte „Einchecken mit UdK-Konto“, Gast-Einladung nachgeordnet.
- Systembrowser-Wartekarte mit Öffnen/Abbruch; geprüfter Adaptercallback statt UI-Identitätsbehauptung.
- Warteschlange mit Position und automatischem Polling (2 s), ohne Weiterklick.
- Auswahl aus `~/cocopi/`: Projekte, Skills, Verläufe, Ergebnisse mit Größen; nur zuletzt benutztes Projekt in der Fixture vorgewählt. Builds/Cache explizit ausgenommen. Upload nur nach „Auswahl senden und starten“; „Ohne Dateien starten“ löst keinen Upload aus.
- Upload-/Downloadfortschritt und bestätigungspflichtiger Abbruch. Bei zugewiesenem Slot bleibt dieser belegt; Abbruch der Auswahl führt zur Check-out-Vorschau, nicht zu stiller Freigabe.
- Arbeiten mit sichtbarem Check-in-Status und genau einem „Auschecken …“-Knopf oben rechts.
- Zweispaltige Vorschau: persönliche Rückläufer links, flüchtige Daten rechts mit aufklappbaren Beispielen; Zurück zur Arbeit möglich. Check-out erfordert aktuelle Vorschau-ID.
- Abschluss erst nach `CheckedOut` des Adapters; lokale Konflikte werden als danebenliegende R2-Fassung beschrieben. Abholfachangebot vor der neuen Auswahl.

## Tests / Nachweise

Alle Cargo-Aufrufe offline, locked und debug; Target `/home/medienhaus/Work/pi-lite-wt/target-s2`, `CARGO_BUILD_JOBS=3`.

1. Modelltests: **7 bestanden**. Decken Auswahlbestätigung, kein Upload ohne Auswahl/Klick, Previewpflicht in UI-Modell und MemoryApi, Browsercallback ohne selbst behauptete Identität, automatisch nachrückende Queue, Abbrüche, verspätete Antworten, Pickup und Konflikthinweis ab. Log `/tmp/I2-S2-flow-direct.log`.
2. Desktop-Tests direkt: **21 bestanden**, einschließlich sechs neuer I2-Tests: alle elf Karten rendern mit erwarteten Texten, genau eine Arbeitsmodus-Check-out-Stelle, Auswahl/Preview-Klicks, Escape in fünf Ablaufzuständen, alte Flowantworten, alte Session-/Heartbeat-Ereignisse nach Check-out und asynchroner MemoryApi-Durchlauf. Log `/tmp/I2-S2-desktop-tests.log`.
3. Finaler breiter cocopi-Testlauf im isolierten HOME/XDG/Agent-Verzeichnis: **137 bestanden, 0 fehlgeschlagen, 2 ignoriert, 1 ausgefiltert**. Befehl:
   `cargo test --offline --locked --manifest-path copi/Cargo.toml --features cocopi --bin cocopi -- --skip adapter_against_isolated_node_mock`
   Die zwei Live-Hub-Tests bleiben ignoriert; der Node-HTTP-Integrationstest wurde ausdrücklich ausgeschlossen. Log `/tmp/I2-S2-final-tests.log`. Testprofil `/tmp/I2-S2-tests-home.oyyYRn`.
4. Finaler Debugbuild erfolgreich: `cargo build --offline --locked --manifest-path copi/Cargo.toml --features cocopi --bin cocopi`; Log `/tmp/I2-S2-final-build.log`.
5. Clippy offline erfolgreich (Warnungen, kein `-D warnings`): Log `/tmp/I2-S2-clippy.log`. Noch vorhandene Warnungen betreffen u.a. für echte Adapter vorgesehene Felder/Callback und ältere Quellen; nicht als warnungsfrei behauptet.
6. `rustfmt` nur für eigene Quellen, `git diff --check` grün. Arbeitsbaum nach eigenen Commits sauber.

## Screenshots

Elf Headless-PNGs, 1400×880, ausschließlich erfundene Demo-Daten, unter:
`/home/medienhaus/Work/pi-lite/coord/shots/I2/S2-<Szene>.png`

- `v3-start`
- `v3-checkin`
- `v3-warteschlange`
- `v3-sync-auswahl`
- `v3-arbeiten-eingecheckt`
- `v3-checkout-vorschau`
- `v3-checkout-fertig`
- `v3-seminar-platzhalter`
- `v3-sync-fortschritt`
- `v3-checkout-fortschritt`
- `v3-abholfach`

Finale Bilder erneut mit Debugbinary erzeugt. Auswahl und Check-out-Vorschau visuell geprüft; zusätzlich Arbeits- und Abschlusskarte angesehen. Keine Behauptung eines vollständigen Fenster-/Browser-/Mac-Livetests. Isoliertes Screenshotprofil `/tmp/I2-S2-final-shots-home.Z1lwx1`. Reproduktion: `COPI_SCENE=v3-sync-auswahl <debug/cocopi> --shot <Datei> --size 1400x880`, mit isoliertem HOME/XDG/Agent-Verzeichnis.

## Übergabe / offene Integration

### S3 — Empfang und Authentifizierung

`Checkin`, `AuthCallback`, `Wait`, `CancelFlow` müssen an den echten Empfang/Loopback-Adapter gebunden werden. UI verarbeitet keine Token und leitet keine Person aus `code`/`state` ab. `AuthCallback` hat redigiertes Debug. Öffnung der Browser-URL setzt voraus, dass der Adapter sie gegen die konfigurierte OIDC-Instanz geprüft hat. PKCE, nonce/state, Loopback, JWKS, Geräteausweis, Challenge, Pool-Routing und sichere Speicherung sind hier **nicht implementiert oder live verifiziert**.

Wichtig: Abbruch muss auch eine zeitgleich erfolgte Belegung auflösen bzw. deren sicheren Folgezustand liefern. Keine blind wiederholten mutierenden Requests ohne Idempotenzvertrag. Neue Hub-Ergebnisse sind an eine Verbindungsgeneration gebunden, damit alte SSE-/Workerantworten nach Check-out keine Sessions oder Verbindung reaktivieren.

### S7 / S4 — Sync und Freigabe

`SyncPlan`, `Upload`, `CheckoutPreview`, `Checkout`, `Collect` und Transfer-`Wait` sind additive Desktop-Verträge. Plan-/Previewdaten sind bisher Fixtures, kein echtes Dateiinventar. `MemoryApi` macht keinerlei I/O.

Der echte Adapter muss A7-Grenzen durchsetzen (Bereiche, Klassen, Symlink-/Pfadschutz, >200-MB-Entscheidung, 2-GiB-Grenze, SHA256, Archivablage, konfliktfreies Rückspielen). **Großdatei-Sonderauswahl ist in dieser S2-Fixture noch nicht abgebildet.** `CheckedOut` darf erst nach erfolgreichem Rückspielen, Empfangsquittung und bestätigtem Slot-Leeren/Freigeben kommen. No-Retention, vollständiges Slot-Wiping und verschlüsseltes 72-h-Abholfach sind ausdrücklich **kein durch S2 nachgewiesener Sicherheitszustand**. Abgebrochener Checkout hält die Belegung, bis der Adapter den tatsächlichen Zustand bestätigt.

### S8 — Seminar-Andockpunkt

S2 stellt `fn seminar_tab(&mut self, ui: &mut Ui)` und Tab „Seminar“ bereit. Der Methodenrumpf ist derzeit ein ehrlicher Platzhalter, keine vorgetäuschte P2P-Funktion. Vorläufige Schnittstellenbitte war `Seminar: Default` mit `ui(&mut self, &mut egui::Ui)`; K integriert anhand der tatsächlichen S8-Schnittstelle Modul/Feld/Initialisierung/Delegation. Keine S8-Quellen ungefragt übernommen. Checkout/Disconnect müssen in der späteren Integration Mitsteuerfreigaben beenden (A9).

### Offene Abnahme

- Echte Empfang-/Sync-Anbindung und Ende-zu-Ende-Test mit geprüftem Abbruch, Konflikten und Slot-Leerungsnachweis.
- Echter Systembrowser-/Loopbacktest; Linux-Fensterinteraktion sowie macOS/Windows separat.
- Produktiver Ordneröffner, reale Dateiauswahl/-größen und Großdateifreigabe.
- S8-Modul und reale Seminar-/Ausweis-/Mitsteuerintegration.
- Separater HTTP-Mock-/Hubtest nach Integration; kein echter Modelltest durchgeführt.

## Eigene Commits (nicht gepusht / nicht gemergt)

1. `ae98526` — Kartenfolge und testbaren Empfang-Sync-Vertrag bauen.
2. `1e71e6b` — Karten und asynchrone Abbrüche prüfen; alte Hubantworten nach Check-out entwerten.
3. `40d873c` — Warteschlangentext an FREEZE-I2 angleichen.

Koordinationsbericht und Bilder liegen im gemeinsamen `coord/`, nicht im S2-Quellcommit. Heartbeats wurden während der Arbeit in `coord/heartbeat/s2` fortgeschrieben.
