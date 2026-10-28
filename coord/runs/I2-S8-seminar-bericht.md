# I2-S8-seminar · Bericht

Stand: 28.10.2026, Branch `v3/s8`, Worktree `/home/medienhaus/Work/pi-lite-wt/s8`. **Bausteine und isolierte Nachweise fertig; produktive Desktop-/Hub-Verdrahtung ausdrücklich noch offen.** Keine fremden App-Dateien geändert, kein Merge/Push, keine Installation und keine neuen Dependencies.

## Commits und Dateien

- `6f5306b` — Ausweisprüfung und flüchtiger Seminarautomat.
- `1521dc9` — Seminaransicht, Zustimmung, fünf Szenen und Harness.
- `27fadf7` — global aufrufbares `consent_ui`, Repaint für Ablauf sowie zertifikatsgebundene Seminareröffnung.
- Neu: `copi/crates/pi-net/src/ausweis.rs`, `copi/crates/pi-net/src/seminar.rs`, `copi/app/src/desktop/seminar.rs`, `copi/app/examples/seminar_harness.rs`.
- Additiv: pi-net `lib.rs`, `wire.rs`, `node.rs`, `store.rs` (letzteres verweist neue Payloads an Ausweis-/Freigabeprüfung).
- `desktop.rs`, Backend-/Hub-Dateien, Lockfile und `docs/concepts/` unangetastet. Kurzzeitiger eigener rustfmt-Durchgriff auf `headless.rs` nach bytegenauem Vergleich mit rustfmt(HEAD) entfernt; Datei unverändert.

## Gebaut

**Ausweis A6:** Ed25519 aus vorhandener iroh-API; S7-Hülle `{inhalt:{…},signatur:base64url}`, rekursiv sortiertes JSON ohne Leerraum. Rollen `Seminarleitung | Teilnehmende | Gast`, Gültigkeit `iat <= jetzt < exp`, Laufzeit höchstens 12 h, echte TLS-Peer-ID muss exakt `net_pubkey` entsprechen. Empfang-/Geräteschlüssel raw 32 Byte Base64url; `net_pubkey` bestehende iroh-Hex-ID. Zeiten **Unix-Sekunden**. `Bestaetigt` weder öffentlich konstruierbar noch deserialisierbar. Empfangsschlüssel muss bereits bewusst bestätigt sein, kein Vertrauen in Peer-Schlüssel. Fehlender/ungültiger Ausweis bleibt „unbestätigt“; Chat bleibt erlaubt und markiert, Mitsteuern nicht.

**Austausch:** `Node::start_with_ausweis(identity, Option<Ausweis>)` sendet additiv `Payload::Geraeteausweis` auf jeder neuen Verbindung in beide Richtungen. Bisheriges `Node::start` bleibt unverändert ohne Ausweis nutzbar. `NodeEvent::Received.from` bleibt TLS-authentifiziert. Kein Versionswechsel, keine Umdefinition bestehender v1-Nachrichten. Alte Gegenstellen können unbekannte Varianten ablehnen; es wird keine Abwärtskompatibilität mit einem unveränderten Altclient für neue Funktionen behauptet.

**Seminar:** GUI-freier flüchtiger `SeminarState`, `SeminarNet`-Trait, `MemoryNet`-Mock. Wiederverwendung von `ChatStore` für Gruppe/DM und `RemoteBook` für Request/Grant/End/Input/Frame; keine Kopie des v2-Automaten. v2-Chatrollen sind intern unprivilegiert und **niemals** Quelle der v3-Rollen. Teilnehmendenliste: Ausweisname/-Badge, online, Slot, Nachricht; Leitung: Session-Auswahl, Ansehen, Mitsteuern anfragen. Gruppeneinladungen müssen von bestätigter Leitung stammen und angenommen werden; unbestätigte Peers können weiterhin DMs und Nachrichten in bestehenden Chats senden. Eigener Tab ohne zweite Funktionsstelle.

**A9:** Modal mit fokussiertem **Ablehnen**, Erlauben 30 min / Nur ansehen / Ablehnen. Nur explizit angefragte eigene Session freigebbar. Cyan `#4fb8d8`, Restzeit, Beenden / Zurückgeben. Leitung sendet `Steuern{req,text}`, Teilnehmer prüft Grant und aktuellen Ausweis erneut und erzeugt `HubPrompt{session,text,von}` für `SeminarNet::senden`. `von` ausschließlich aus geprüftem Ausweis, nicht aus Eingabetext. Sichtbare Marke „Aylin · Seminarleitung“. Rückfragen werden nicht fernbeantwortet. Ablauf, beidseitiges Ende, >60 s offline, ungültiger Ausweis und Check-out entfernen Grants. Ablehnung erzeugt keinen gespeicherten Verlauf. Check-out leert Ausweise, Kontakte, Chats und UI-Promptliste.

**Ansehen:** `SeminarState::mirror(req,session,frame,now)` prüft exakt freigegebene eigene Session, Ausweise und Ablauf, schwärzt mit vorhandenem `remote::redact`, begrenzt Paketgröße; UI rendert empfangene `RemoteFrame`-Items. Ohne Runtime-Spiegel ehrlich „Wartet auf freigegebenen Verlauf …“.

## Nachweise / reproduzierbare Befehle

Umgebung für alle Cargo-Aufrufe:

```sh
export PATH="$HOME/.cargo/bin:$HOME/.local/share/mise/shims:$PATH"
export CARGO_BUILD_JOBS=3
export CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s8
cargo test --offline --manifest-path copi/Cargo.toml -p pi-net --lib
cargo test --offline --manifest-path copi/Cargo.toml -p copi --features cocopi --example seminar_harness
cargo check --offline --manifest-path copi/Cargo.toml -p copi --features cocopi --bin cocopi
cargo run --offline --manifest-path copi/Cargo.toml -p copi --features cocopi --example seminar_harness -- /home/medienhaus/Work/pi-lite/coord/shots/I2
cargo clippy --offline --manifest-path copi/Cargo.toml -p copi --features cocopi --example seminar_harness -- -D warnings -A clippy::large-enum-variant -A clippy::collapsible-if
```

- **79 pi-net-Tests grün** (`/tmp/s8-net-final2.log`), darunter:
  - `no_certificate_no_control`
  - `participant_cannot_request_control_or_spoof_role`
  - `rejection_persists_nothing`
  - `expiry_stops_control`
  - `leader_prompt_visibly_marked_and_no_requester_attribution`
  - `view_only_stop_disconnect_checkout_and_session_binding`
  - `mirror_only_selected_session_and_redacts_secrets`
  - `chats_and_dms_reuse_v2_and_mark_unverified`
  - Ausweis-Manipulation, falscher Empfang, Zeit, Peerbindung, kanonisches JSON/Base64url.
  - `loopback_optional_ausweis_exchange_keeps_tls_sender` mit zwei flüchtigen Nodes.
- **3 Harness-Tests grün** (`/tmp/s8-ui-final2.log`): fünf Szenen rendern; echte Enter-Tasteneingabe lehnt fokussierte Anfrage ab, erzeugt niemals Active-Grant; mitgelieferter Renderer-Test ebenfalls grün.
- cocopi-Debug-Check grün, sechs bestehende Dead-Code-Warnungen; keine Release-Builds.
- Clippy für den isolierten Harness mit zwei benannten Ausnahmen grün (`/tmp/s8-clippy-final2.log`). **Ungefiltertes `-D warnings` nicht grün**: `large_enum_variant` an bestehenden NodeEvent/Command-Formen und `collapsible_if` in `store.rs:550`; keine pauschale Clippy-Grünbehauptung. Wiederverwendeter Renderer hat lokal am Modul `chunks_exact_to_as_chunks` erlaubt, statt fremde Datei zu ändern.
- rustfmt-Check der eigenen Rust-Dateien und `git diff --check` grün.

Fünf PNGs aus echter egui-UI, 1440×900, CPU-Renderer ohne App-/Config-Start, in `/home/medienhaus/Work/pi-lite/coord/shots/I2/`:

1. `v3-seminar-chat.png`
2. `v3-seminar-dm.png`
3. `v3-mitsteuern-anfrage.png`
4. `v3-mitsteuern-aktiv-student.png`
5. `v3-mitsteuern-aktiv-leitung.png`

Anfrage und aktive Ansichten visuell geprüft: Ablehnen-Fokus, markierte Prompts, Rahmen und Restzeit sichtbar. Ruhige feste Seitenleiste, ein Chatbereich, keine versteckten Kontextmenüs. Demo zeigt unbestätigten/offline Jonas ausdrücklich. Screenshotdaten sind synthetisch und mit getrenntem Demo-Schlüssel signiert.

## Konkrete Änderungsbitten / Integrationsgrenze

**S2/K (desktop.rs gehört S2):**

1. Feature-gegated `pub mod seminar;`, ein Feld `Option<seminar::Seminar>`, genau eine Methode `seminar_tab(&mut self, ui: &mut egui::Ui)` und genau einen Zugang „Seminar“ registrieren. Nicht zusätzlich v2-Seminar-UI daneben zeigen.
2. `Desktop::scene` an `seminar::scene(name)` delegieren; oben genannte fünf Namen. Aktuell nur über eigenen Harness erreichbar, **noch nicht** `COPI_SCENE` im Desktop.
3. `Seminar::tick()` **auch bei geschlossenem Tab** regelmäßig aufrufen. `Seminar::remote_bar(ui)` über dem Hauptchat immer sichtbar halten; sie zeichnet auch den Cyan-Rahmen ihres übergebenen UI-Rechtecks. Für gesamten Fensterrahmen auf äußerstem Desktop-UI aufrufen. `Seminar::consent_ui(ui)` ist separat öffentlich: global am äußersten UI auch bei geschlossenem Seminar-Tab aufrufen. `ui()` ruft sie ebenfalls auf; nicht im selben Frame doppelt rendern.
4. **`Desktop::send_as(von,text)`** bereitstellen (bei mehreren Sessions explizites Ziel erforderlich). Adapter implementiert `SeminarNet::senden(session,text,von)` → eigener Teilnehmer-Hub `senden{text,von:{person,rolle}}`. Keine fremden Tokens, kein direkter Leitung→Hub-Weg. Bestätigte Hub-Prompts im **Hauptchat** mit derselben Marke/Farbe anzeigen. S8 rendert dies vorerst isoliert.
5. Bei Check-out `Seminar::checkout()` auslösen; Roster/Slots und eigene Sessions aktuell halten, Presence über `state.status(peer,online,now_ms)`. Nur nach bewusstem Check-in aktivieren; Demo niemals als produktiven Trust-Anker verwenden.

**S3:** Produktionsadapter `SeminarNet` mit bestehendem Node verbinden. Ingress nur aus `NodeEvent::Received.from`, Zertifikat vor Control abwarten; Presence-Ausfall zuverlässig melden (nicht nur TLS-Idle als Check-out interpretieren). Eigenen Hub-Verlauf nur für ausdrücklich freigegebene Session an `state.mirror` geben. Additive `Steuern`-Variante liegt in pi-net/wire, v2 `RemoteInput` wird weiterhin geprüft akzeptiert. Hub-`senden.von` und Hauptchat-Zuordnung mit S2 testen. Empfang-Key-Pinning/Fingerabdruck in Konto kommt nicht aus diesem isolierten Modul.

**S7:** Hülle folgt der frühen Schnittstellenmeldung. Bitte Zeiten in Unix-Sekunden und `net_pubkey` als iroh-Hex bestätigen; ansonsten explizite A6-Abstimmung, keine stillen Einheitenheuristiken. Fake-OIDC-/echten Empfangsausweis als gemeinsame Integrationsfixture nach Merge prüfen.

**S6/K:** Nach Verdrahtung vollständiger Durchlauf mit zwei Desktop-Clients: Check-in → Ausweise → Gruppe/DM → Anfrage → Ablehnen/Ansicht/Control → tatsächlicher Hub-Chat mit `von` → Stop/Offline/Check-out. Aktuell **keine** produktive End-to-End-Freigabe.

## Nicht getestet / bekannte Grenzen

Keine echten authentik-/Empfang-/R2-/Hub-Verbindungen, keine Modelle, keine Live-Dateien, keine Installation oder fremden Prozesse. Keine macOS-/Windows-/Handy-Tests, kein Screenreader-/Laientest. Kern-Tests enthalten vorhandene lokale Transporttests (einschließlich alten LAN-Direktprototyp im selben Prozess); neuer Ausweistest nur Loopback. `MemoryNet` bestätigt nur lokale Übergabe, keine echte Zustellung; „wird gesendet“ bleibt deshalb in Demos teilweise stehen. Kein dauerhafter Grant-/DM-Speicher. Freigabeanfragen binden im v2-Vertrag den angefragten **Titel**; akzeptiert wird ausschließlich eine lokal vorhandene Session und ihr vollständiger `SessionRef`, Runtime muss eindeutige Session-IDs in `cwd`/Adapter halten. Begrenzung der Anfragen kommt aus RemoteBook/Transport. Produktionsschutz bei geschlossenem Tab und global sichtbarer Consent sind **Integrationspflicht**, nicht bereits fertig behauptet.
