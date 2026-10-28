# cocopi v3 · Akzeptanzkriterien I1 + I2 (S1, Fassung 2)

Jedes Kriterium ist **prüfbar** und hat einen Nachweis: einen Testnamen, einen Befehl oder einen Screenshot (§7.1).

**Testnamen sind Vorschläge.** Die zuständige Rolle darf umbenennen, muss den neuen Namen aber im Bericht nennen.

**Orte und Zuständigkeiten:**
- Rust-Tests liegen in `copi/…` und werden mit `cargo test --offline` ausgeführt (S2).
- Hub- und Mock-Tests sind Node-Tests (S3); vorgeschlagener Ort ist `pi-hub/test/` bzw. der Mock-Hub von S3.
- Mobil-Tests liegen unter `scripts/test-*.cjs` (S5).
- Screenshots entstehen mit `copi --shot docs/v3/shots/<name>.png --size 1280x800` und `COPI_SCENE` (S2).

Allgemein für I1:
- **G1:** `cargo test --offline`, `cargo clippy --offline -- -D warnings` und `cargo fmt --check` sind grün. Nachweis: Befehlsausgabe im Bericht.
- **G2:** Es gibt keine neue Webview- oder Electron-Abhängigkeit im Desktop (E2). Nachweis: `cargo tree --offline -p copi | grep -Ei 'wry|webview|tauri'` liefert nichts.

---

## T1 · Remote-only / Hybrid (E4, KONZEPT A2)

| # | Kriterium | Nachweis (Testname / Befehl) |
|---|---|---|
| T1.1 | Ohne `mode.json` gilt `Mode::RemoteOnly`. Eine kaputte `mode.json` ergibt ebenfalls `RemoteOnly`. | `mode_defaults_to_remote_only`, `mode_corrupt_file_is_remote_only` |
| T1.2 | `pi_command` gibt bei `RemoteOnly` `Err(SpawnBlocked)` zurück, für **jeden** `SpawnPurpose`. Es wird kein Prozess erzeugt. | `remote_only_blocks_every_spawn_purpose` |
| T1.3 | Außerhalb von `winshim.rs` steht in `copi/` (ohne Tests) kein `Command::new("pi")` und kein `Command::new(` mit `pi`-Pfad. | `only_winshim_spawns_pi` (liest die Quelltexte und sucht per Muster) + Befehl `grep -rn 'Command::new("pi")' copi --include=*.rs` → nur `winshim.rs` |
| T1.4 | Desktop startet ohne `pi` im `PATH`, zeigt D-ST bzw. die Sessionliste und erzeugt keinen `pi`-Prozess. | Test `start_without_pi_in_path_spawns_nothing`. Er setzt `PATH` auf ein leeres Verzeichnis mit einem **Fallen-Skript `pi`**, das bei Aufruf eine Markerdatei schreibt. Nach dem Start, nach Backend-Wechsel, nach „offline“ und nach dem Ausblenden der Seitenleiste darf die Markerdatei nicht existieren. |
| T1.5 | `backends.json` fehlt oder ist unlesbar, dazu `RemoteOnly`: Die Backend-Liste enthält **kein** `kind:"local"`, und es wird **kein** lokales Backend ersatzweise gesetzt (heute `net.rs:588`). | `remote_only_backends_never_contain_local`, `unreadable_backends_no_local_fallback` |
| T1.6 | Die Startkarte ruft im Modus `RemoteOnly` weder `pi_version` noch `latest_version` auf. | ist durch T1.4 abgedeckt (Fallen-Skript) + `start_card_remote_only_has_no_pi_update` |
| T1.7 | Wechsel nach Hybrid nur über den Bestätigungsdialog. „Abbrechen“ lässt `RemoteOnly` bestehen. | `hybrid_requires_confirmation`, `hybrid_cancel_keeps_remote_only` |
| T1.8 | Ein Wechsel in beide Richtungen beendet und startet keine Session: Die Zahl der Instanzen und ihre Generationen bleiben gleich. | `mode_switch_keeps_running_sessions` |
| T1.9 | Die Einstellung bleibt nach einem Neustart erhalten. Die Datei hat die Rechte 0600. | `mode_persists_across_restart` |
| T1.10 | D-SZ zeigt den Modus **immer**. | Screenshots `shots/t1-remote-only.png` und `shots/t1-hybrid.png`, dazu Test `status_line_always_shows_mode` (Text in der Szene) |
| T1.11 | Hybrid ohne lokales pi installiert **nichts** und zeigt den Text aus FLOWS F10 3F (bis I1: F5 3F). | `hybrid_without_pi_explains_not_installs` (Fallen-Skripte `npm`, `curl`, `brew` werden nicht aufgerufen) |

## T2 · QR-Login gegen Mock-Hub (E5, KONZEPT A3)

| # | Kriterium | Nachweis |
|---|---|---|
| T2.1 | Eine gültige Einladung (`e`) registriert das Gerät für **genau** den Slot aus der Einladung. Die Antwort enthält `slot`, `person` und `geraet_id`. | `invite_registers_device_for_slot` (Mock-Hub) |
| T2.2 | Eine abgelaufene Einladung wird mit `code:"abgelaufen"` abgewiesen. Der Desktop zeigt den Text aus A3. | `invite_expired_rejected` + Desktop `login_shows_expired_text` |
| T2.3 | Eine verbrauchte Einladung wird mit `code:"verbraucht"` abgewiesen. Bei zwei **gleichzeitigen** Einlösungen gelingt genau eine. | `invite_single_use`, `invite_concurrent_redeem_once` |
| T2.4 | Der Hub speichert keine Klartext-Codes, nur Hashes. | `hub_stores_only_code_hash` (Datei durchsuchen, der Code darf nicht vorkommen) |
| T2.5 | Der private Schlüssel verlässt das Gerät nicht: Kein Request-Body an den Mock enthält ihn. | `private_key_never_sent` (Mock zeichnet alle Bodies auf) |
| T2.6 | Ein Gerät aus Slot A erhält mit seinem Token keine Daten aus Slot B (403), auch nicht durch einen manipulierten `slot`-Parameter. | `device_cannot_access_other_slot` |
| T2.7 | Umbenennen der Person ändert keine Rechte: Nach `person`-Änderung hat `geraet_id` weiter dieselben Rechte und keine zusätzlichen. | `rename_person_keeps_rights` |
| T2.8 | Koppeln (`k`) ist erst gültig nach **[Erlauben]** am Desktop. Ablehnen speichert nichts. | `pair_requires_desktop_approval`, `pair_rejected_stores_nothing` |
| T2.9 | Ein neuer Koppel-Code macht den vorigen, unbenutzten Code ungültig. | `new_pair_code_invalidates_unused` |
| T2.10 | Widerruf: Das Token ist danach ungültig (401), und eine offene Ereignisverbindung des Geräts wird innerhalb von 2 s geschlossen. | `revoke_invalidates_token`, `revoke_closes_open_stream` |
| T2.11 | Fünf Fehlversuche pro Minute: Der sechste wird mit `code:"zu_viele"` abgewiesen. | `redeem_rate_limited` |
| T2.12 | D-SZ zeigt nach dem Login „Mira · account03 @ R2“. | Screenshot `shots/t2-angemeldet.png`, dazu Test `status_line_shows_person_slot_host` |
| T2.13 | Der Code der QR-URL taucht in keinem Hub-Log und in keiner Zugriffs-Logzeile auf. Ort: Fragment `#`; Query `?c=` nur, wenn S5 die Android-Grenze aus KONZEPT A3 bestätigt. | `qr_code_never_logged` |

## T3 · Sessiongrenzen (KONZEPT A1)

| # | Kriterium | Nachweis |
|---|---|---|
| T3.1 | Bei 2 offenen Sessions wird die dritte `session.oeffnen` mit `zu_viele_offen` abgewiesen. **Keine** Session-Datei wird verändert oder gelöscht (Prüfsumme davor = danach). | `third_session_rejected_nothing_deleted` |
| T3.2 | Die Grenze gilt **über Geräte**: Desktop öffnet 2, Handy versucht die 3. → abgewiesen. | `limit_counts_across_devices` |
| T3.3 | Parallel: 5 gleichzeitige `session.oeffnen` bei 0 offenen → genau 2 Erfolge. | `parallel_open_respects_limit` |
| T3.4 | Ein Reconnect umgeht die Grenze nicht: Trennen, Neu-Verbinden und Öffnen ergibt weiterhin 2 offene. | `reconnect_does_not_reset_count` |
| T3.5 | Höchstens 1 Session „arbeitet“. Ein zweiter Auftrag geht in den Zustand „wartet auf Start“ und startet nach dem Ende des ersten. | `second_busy_is_queued` |
| T3.6 | „ruht“ zählt nicht als offen. Nach „Ruhen lassen“ kann eine neue Session geöffnet werden. | `resting_session_frees_slot` |
| T3.7 | Die Anzeige unterscheidet ruht, arbeitet, wartet auf dich, fertig, bereit und Fehler. Desktop und Mobil zeigen für dieselbe Session denselben Zustand, die Quelle ist `session.zustand` vom Hub. | Test `state_labels_match_protocol` (Desktop + `scripts/test-session-state.cjs` Mobil), Screenshot `shots/t3-zustaende.png` |
| T3.8 | Fehlt der Puls länger als 45 s, wird jeder Zustand grau mit „Stand hh:mm“ gezeigt. Es wird **kein** Zustand als aktuell behauptet. | `stale_after_missed_heartbeat` |
| T3.9 | Subagenten zählen nicht als eigene offene Session, umgehen die Grenze aber auch nicht. Sie laufen innerhalb der arbeitenden Session. | `subagent_counts_with_parent` |

---

## A1 · Protokollvertrag

| # | Kriterium | Nachweis |
|---|---|---|
| A1.1 | `shared/protocol/v1` existiert mit Nachrichten, Zuständen, Fehlercodes, `puls` und `protokoll`. Zu jedem Fehlercode gibt es `was` und `tun` auf Deutsch. | Datei + `protocol_error_codes_have_texts` |
| A1.2 | Ein Client mit einer zu alten Protokollversion bekommt den festen Text, ein Hub mit zu alter Version ebenso. | `protocol_mismatch_client_old`, `protocol_mismatch_hub_old` |
| A1.3 | Ein unbekanntes optionales Feld wird ignoriert und ist kein Fehler. | `protocol_ignores_unknown_optional_field` |
| A1.4 | Freigabe durch S1 ist im Bericht vermerkt. | Bericht S1 I1-Abschluss |

## A2 · Moduslogik

| # | Kriterium | Nachweis |
|---|---|---|
| A2.1 | Es gibt genau **eine** Prüfstelle: `pi_backend::winshim::pi_command`. | T1.3 `only_winshim_spawns_pi` |
| A2.2 | Das Zustandsdiagramm in KONZEPT.md entspricht dem Code: zwei Zustände, Standard `RemoteOnly`, Hybrid nur mit Bestätigung. | T1.1, T1.7 + Review S1 |

## A3 · QR-Ablauf

| # | Kriterium | Nachweis |
|---|---|---|
| A3.1 | Alle drei Arten (e, k, b) laufen über **denselben** Einlöse-Endpunkt im Hub, mit `art` als Feld. | `redeem_single_endpoint_all_kinds` |
| A3.2 | Gültigkeiten: e = 24 h, k = 5 min, b = 2 min. Die Grenzen werden mit einer manipulierten Uhr getestet. | `code_ttl_per_kind` |
| A3.3 | Die Bildschirmtexte stimmen mit KONZEPT A3 überein. | Tests vergleichen gegen eine gemeinsame Textdatei (`shared/protocol/v1/texte.de.json`, Vorschlag) + Screenshots `shots/a3-*.png` |
| A3.4 | Die Beschreibung einer Bestätigung (b) kommt vom Hub, nicht aus dem QR-Inhalt. | `confirm_description_from_hub` |

## A4 · Kontomodell (ersetzt durch E7; A4.1/A4.2 gelten je Arbeitsplatz weiter, Pool: T13)

| # | Kriterium | Nachweis |
|---|---|---|
| A4.1 | Das Provisionierungsskript (Dry-Run) gibt für `account01` und `account02` aus: Unix-Konto, Slice-Grenzen, Quote, Hub-Port, Linger. Es verändert dabei **nichts**. | Befehl von S4 `… --dry-run account01 account02` + Prüfskript |
| A4.2 | Das Geräte-Register eines Hubs enthält nur Geräte seines Slots. | T2.6 |
| A4.3 | Freigabe durch S1 ist im Bericht vermerkt. | Bericht |

## A5 · Funktionsinventar

| # | Kriterium | Nachweis |
|---|---|---|
| A5.1 | Jede Zeile in INVENTAR.md hat genau einen v3-Ort oder „weg“. | Review S1 + Befehl `awk` (Spalte v3-Ort nicht leer) |
| A5.2 | Die 14 als Dublette markierten Wege sind im Desktop-Code entfernt. Der Nachweis je Zeile ist ein Testname oder ein `grep`, der nichts mehr findet, z. B. `grep -n '"/ Befehle"' copi/app/src/view.rs` → leer. | `inventory_duplicates_removed` (prüft Szene: kein Knopf „/ Befehle“, „+ Instanz“, „▦ Raster“, „A−“/„A+“) |
| A5.3 | Die Hauptansichten passen zu den ASCII-Skizzen in FLOWS.md. | Screenshots `shots/a5-start.png`, `shots/a5-arbeiten.png`, `shots/a5-einstellungen.png` + Review S1 gegen Regeln 1–8 |
| A5.4 | Mobil (I1) zeigt Sessions mit Zuständen **nur lesend**. Es gibt keinen Knopf für Neu oder Löschen. | `scripts/test-mobile-readonly.cjs` + Screenshot `shots/m-liste.png` |

---

# I2 · T13–T16 (Fassung 2, FREEZE-I2)

Spalte **Nachweis**: Testname, Befehl oder Screenshot. Die Markierung **[da]** heißt: Der Test steht mit diesem Namen schon auf dem genannten Branch (gesehen am 28.10., **nicht** von S1 ausgeführt). Ohne Markierung ist es ein Vorschlag. Spalte **Prüft**: Rolle, die den Test schreibt bzw. ausführt; **S6** führt die Angriffs- und Gate-Fälle unabhängig aus (`bash test/v3-gate.sh` mit `test/i2/*.test.mjs`). SKIP zählt nie als bestanden.

Abkürzungen der Orte:
- **Hub-S7** = pi-hub `empfang/test/empfang.mjs` bzw. `test/sync.mjs`, `node --test`
- **S6-i2** = pi-lite `test/i2/*.test.mjs`, `node --test test/i2/`
- **Rust** = `cargo test --offline` im genannten Crate

Allgemein für I2: G1 und G2 aus I1 gelten weiter, dazu:
- **G3:** `bash test/v3-gate.sh "$PWD" <hub-worktrees>` endet mit Exit 0, und keine T13–T16-Zeile steht auf SKIP. Prüft: S6.
- **G4:** Alle wörtlichen Texte aus FLOWS F5–F9 und KONZEPT B11 sind im Code genau so vorhanden. Nachweis: `grep -F` je Text in `copi/app/src` (Liste aus FLOWS), Abweichungen im Bericht der jeweiligen Rolle. Prüft: S1 (Review).

## T13 · Check-in mit Pool und Warteschlange (E7, E8, KONZEPT B2/B3, FLOWS F5)

| # | Kriterium | Nachweis | Prüft |
|---|---|---|---|
| T13.1 | 20 gleichzeitige Check-ins bei 10 freien Arbeitsplätzen: genau 10 erhalten `eingecheckt` mit **10 verschiedenen** Slots, 10 erhalten `warteschlange` mit den Plätzen 1–10 ohne Lücke und ohne Doppel. Zwei Check-ins bekommen nie dasselbe Konto. | Hub-S7 [da] „20 parallele Check-ins: 10 verschiedene Slots, 10 FIFO-Wartende, Nachrücken und Neustart“; S6-i2 [da] „Pool-Kern: 20 konkurrierende Reservierungen …“ und „Empfang HTTP: 20 parallele OIDC-Check-ins …“ | S7, S6 |
| T13.2 | FIFO: Wird ein Arbeitsplatz frei, bekommt ihn Platz 1. Alle anderen rücken um eins vor. Der Client wechselt **ohne Klick** von der Warteschlange in die Sync-Auswahl. | Hub-S7 (wie T13.1, Teil „Nachrücken“) + Rust S2 [da] `browser_callback_is_not_an_identity_and_queue_advances_without_click` | S7, S2 |
| T13.3 | Die Seminarleitung hat keinen Vorrang: Ihr Check-in bei vollem Pool landet hinten. Einem belegten Arbeitsplatz wird nie eine Belegung entzogen, außer durch Check-out. | Hub-S7 [da] „FIFO-Verfall 90 Sekunden, Seminarleitung ohne Vorrang“ | S7 |
| T13.4 | Ein Warteplatz verfällt nach 90 s ohne Abfrage; bei 89 s gilt er noch. Getestet mit manipulierter Uhr. | Hub-S7 (wie T13.3); S6-i2 [da] „Pool-Kern: unbeobachtete Warteschlange verfällt nach 90 Sekunden“ | S7, S6 |
| T13.5 | Pool-Zustand übersteht einen Neustart des Empfangs: dieselben Belegungen und dieselbe Reihenfolge. | Hub-S7 (wie T13.1, Teil „Neustart“); S6-i2 [da] „Pool-Kern: belegtes Konto nicht ohne Leeren freigeben; Zustand über Neustart erhalten“ | S7, S6 |
| T13.6 | Rolle nur aus geprüftem Token: Gruppen `cocopi-seminarleitung` → seminarleitung, `cocopi-teilnehmende` → teilnehmende, sonst `keine_berechtigung`. Ein Feld `rolle` im Request-Body wird ignoriert. Abgewiesen werden falsche `iss`, falsche `aud`, abgelaufenes `exp`, falsche `nonce`, manipulierte Signatur und `alg:none`. | Hub-S7 [da] „OIDC RS256/ES256, Gruppenrolle, fremde/abgelaufene/manipulierte Nachweise“ (Fake-OIDC `empfang/test/fake-oidc.mjs`); S6-i2 [da] „OIDC: lokaler JWKS-Issuer prüft RS256 und ES256; Rollen nur aus signierten Gruppen“; Vorschlag zusätzlich `oidc_body_role_ignored`, `oidc_alg_none_rejected` | S7, S6 |
| T13.7 | PKCE S256 und Loopback: Der Desktop öffnet den System-Browser mit `code_challenge_method=S256`, lauscht nur auf `127.0.0.1`, verwirft einen Rückruf mit falschem `state` und bricht nach Zeitablauf ab. Der Rückruf allein gilt nicht als Identität, erst die Antwort des Empfangs. | Rust S3 [da] `local_discovery_browser_callback_token_exchange`, `redirect_wrong_state_consumes_attempt_and_timeout`; S2 [da] `browser_callback_is_not_an_identity_and_queue_advances_without_click` | S3, S2 |
| T13.8 | Gast: Ein Einladungscode gilt 5 min und genau einmal. Bei zwei gleichzeitigen Einlösungen gelingt genau eine. Die Rolle ist immer `gast`. Nur die Seminarleitung kann Einladungen erzeugen. Der Empfang speichert nur den Hash des Codes. | Vorschlag Hub-S7 `einladung_einmalig_5min_gast`, `einladung_nur_seminarleitung`, `einladung_nur_hash` (Datei durchsuchen) | S7, S6 |
| T13.9 | Geräteausweis: Ed25519, `exp − iat = 12 h`, bindet `pid`, Person, Rolle, `net_pubkey` und `geraet_pubkey`. Wird ein Feld verändert, ist die Signatur ungültig. Ein abgelaufener Ausweis wird abgewiesen. | Rust S3 [da] `canonical_signature_tamper_and_unsafe_numbers`, `empfang_envelope_matches_node_vector`, `checkin_acceptance_binds_pin_device_role_path_and_expiry`; S8 [da] `signature_time_binding_and_tampering` | S3, S8 |
| T13.10 | Abbruch in jedem Schritt vor der Zuweisung (Browser, Warteschlange) gibt den Warteplatz auf und zeigt „Check-in abgebrochen.“. Nach der Zuweisung führt „Check-in beenden …“ in die Check-out-Vorschau. Eine späte Antwort nach dem Abbruch öffnet den Check-in nicht wieder. | Rust S2 [da] `cancel_each_step_preserves_remote_data_until_confirmed_checkout`, `cancelled_checkin_cannot_revive_from_late_reply`, `i2_escape_cancels_browser_queue_upload_checkout_and_preview` | S2 |
| T13.11 | Wortlaut der Warteschlange genau: „Alle 10 Arbeitsplätze sind belegt. Du bist Nr. 2 in der Warteschlange.“ | Screenshot `shots/t13-warteschlange.png` (`COPI_SCENE=v3-warteschlange` [da] auf `v3/s2`) + `grep -F` (G4) | S2 |
| T13.12 | Logs des Empfangs enthalten keinen Anzeigenamen, keine `sub`, kein Token und keinen Code, nur Zeit, Ereignis und HMAC-pid. Tagesdateien älter als 7 Tage werden gelöscht. | Vorschlag Hub-S7 `logs_ohne_namen_tokens`, `logs_rotation_7_tage` (Testlauf mit Person „Mira Testname“, danach `grep -r` im Logverzeichnis → leer) | S7, S6 |

## T14 · Sync-Auswahl beim Check-in (E10, A7, KONZEPT B4/B6, FLOWS F6)

| # | Kriterium | Nachweis | Prüft |
|---|---|---|---|
| T14.1 | Die Auswahl zeigt je Projekt bzw. Bereich die Größe. Vorausgewählt ist **nur** das zuletzt benutzte Projekt (`.sync/zustand.json`). Fehlt der Zustand, ist nichts vorausgewählt. | Rust S2 [da] `no_upload_without_confirmation_and_only_last_project_selected` + Vorschlag `no_state_preselects_nothing` + Screenshot `shots/t14-auswahl.png` (`COPI_SCENE=v3-sync-auswahl` [da]) | S2 |
| T14.2 | Ohne Klick auf „Auswahl senden und starten“ geht kein Byte nach R2. Das gilt auch für Esc, Fenster schließen und „Ohne Dateien starten“. | Rust S2 [da] `no_upload_without_confirmation_and_only_last_project_selected`, `i2_ui_selection_requires_click_and_preview_cancel_does_not_checkout` | S2, S6 |
| T14.3 | Klassen: Alle Fälle aus `shared/sync/faelle.json` ergeben in Rust (`pi-sync`) und Node (`lib/sync-v1.mjs`) dieselbe Klasse. Dazu gehören `Ergebnisse/target/a.exe` → persönlich, `Projekte/App/node_modules/a.js` → flüchtig und `Geheim/notiz.md` → flüchtig. | Rust `pi-sync` [da] `gemeinsame_klassenfaelle`; Hub-S7 [da] „gemeinsame Klassifizierungsfälle und Prüfsummen“ | S7 |
| T14.4 | Pfadausbrüche werden abgewiesen, ohne dass etwas zurückbleibt: `..`, absolute Pfade, `C:\`, Backslash, Symlink, Hardlink, Gerät und FIFO im Archiv, ein bestehender Ziel-Symlink im Slot. Ein vorangestellter gültiger Eintrag bleibt nicht liegen. | Hub-S7 [da] „Pfadangriffe, Headerfehler, Links/Geräte und Größen werden abgewiesen“, „Symlinks im Slot werden weder gelesen noch archiviert“, „fehlerhafter Upload bleibt atomar ohne Ziel“; S6-i2 [da] `sync.test.mjs`; Rust `pi-sync` [da] `pfadangriffe_hash_und_links`, `symlink_ziel_bleibt_unberuehrt` | S7, S6 |
| T14.5 | Riesendateien: Eine einzelne Datei über 200 MiB ist nie vorausgewählt und wird ohne ausdrückliche Freigabe abgewiesen. Über 2 GiB entpackte Nutzdaten je Belegung werden abgewiesen. Ein 3-GiB-Größenheader ohne Nutzlast erzeugt keine große Speicherbelegung. | S6-i2 [da] „Testarchive: 3-GiB-Größe steht nur im Header …“; Hub-S7 (wie T14.4, Teil „Größen“); Vorschlag S2 `large_file_never_preselected` | S7, S6, S2 |
| T14.6 | Ein Upload ist atomar: Bei Abbruch oder Fehler gibt es auf R2 kein halbes Projekt. Ein zweiter Upload mit demselben Namen wird abgewiesen und überschreibt nichts. | Hub-S7 [da] „fehlerhafter Upload bleibt atomar ohne Ziel“ + Vorschlag `upload_same_name_rejected` | S7 |
| T14.7 | Nur ein Desktop-Gerät der eigenen Belegung darf hochladen. Ohne Geräteschlüssel oder Token gibt es weder Upload noch Archiv. Kein Token in der URL. | S6-i2 [da] „Sync: ohne Geräteschlüssel/Token weder Upload noch Archivabruf“ | S6 |

## T15 · Check-out mit Vorschau, Archiv und Leeren (E9, A7/A8, KONZEPT B5, FLOWS F7)

| # | Kriterium | Nachweis | Prüft |
|---|---|---|---|
| T15.1 | Die Vorschau zeigt zwei Spalten, „Kommt zurück auf deinen Rechner“ und „Wird gelöscht“, mit Größen. Unveränderte hochgeladene Dateien stehen nicht unter „kommt zurück“. Ohne aktuelle Vorschau ist kein Check-out möglich. | Rust S2 [da] `complete_upload_preview_checkout_and_conflicts`, `checkout_requires_current_preview_in_ui_and_adapter`; Hub-S7 [da] „Upload, Manifest, Vorschau, nur geänderte Rückläufer und große Auswahl“; Screenshot `shots/t15-vorschau.png` (`COPI_SCENE=v3-checkout-vorschau` [da]) | S2, S7 |
| T15.2 | Rückspielen ohne Überschreiben: Eine lokal geänderte Datei bleibt bytegleich (sha256 davor = danach). Die R2-Fassung liegt als `Name (R2 TT.MM. HH-MM).ext` daneben, bei Wiederholung mit ` 2`. Ein Eintrag in `.sync/konflikte/` entsteht. Identische Dateien werden nicht doppelt abgelegt. | Rust `pi-sync` [da] `auswahl_stream_import_ohne_ueberschreiben`; S2 [da] `complete_upload_preview_checkout_and_conflicts` | S7, S2 |
| T15.3 | Reihenfolge: Löschen beginnt erst nach `erhalten{sha256}` mit dem richtigen Hash. Bei falschem Hash, Abbruch oder Verbindungsabbruch vor dem Erhalt bleiben Belegung und alle Dateien auf R2. | S6-i2 [da] „Empfang E2E: Check-in → Projekt → Vorschau → Archiv → Check-out → Slot-Fake leer“; Rust S2 [da] `cancel_each_step_preserves_remote_data_until_confirmed_checkout`; Vorschlag Hub-S7 `erhalten_falscher_hash_loescht_nichts` | S7, S6, S2 |
| T15.4 | **Nach dem Check-out ist das Konto nachweislich leer** (Prüfskript): `cocopi-slotctl pruefen accountNN` meldet leer. Home und `/mnt/data/accountNN` enthalten nur das Skelett, die Quotenbelegung ist ≈ 0, und unter der UID läuft kein Prozess. Erst danach gilt der Slot im Pool als `frei`. | S4 [da] `test_clear_proof_and_repeat_are_idempotent`, `test_quota_residue_prevents_empty_proof`, `test_live_processes_prevent_any_deletion` (`python3 deploy/r2/test_scripts.py`); auf R2 nach Freigabe D-R1: Befehl `sudo cocopi-slotctl pruefen account03` → Ausgabe im Bericht | S4, S6 |
| T15.5 | Schlägt das Leeren oder `pruefen` fehl, bleibt der Arbeitsplatz gesperrt. Der Client zeigt „Der Arbeitsplatz ist noch nicht nachweislich leer.“ und **nie** „frei“. | Hub-S7 [da] „fehlgeschlagenes Leeren hält Slot gesperrt, Empfang-Challenge einmalig“, „Leeren-Fake bekommt richtigen Slot; nur bestätigtes leer gilt“; S6-i2 [da] „Empfang: fehlgeschlagenes Leeren lässt Konto gesperrt …“ | S7, S6 |
| T15.6 | Das Leeren folgt keinem Symlink nach außen (Symlink nach `/etc` im Home: `/etc` bleibt unverändert). Ein paralleles zweites Leeren wird blockiert. | S4 [da] `test_inner_symlink_to_etc_unlinked_not_followed`, `test_slot_root_and_parent_symlinks_rejected`, `test_concurrent_clear_is_blocked` | S4 |
| T15.7 | Zwangs-Check-out (8 h ohne Gerät, 23:30, Seminarleitung): Zuerst wird das verschlüsselte Abholfach gespeichert, dann geleert. Nur der richtige Geräteschlüssel derselben `pid` öffnet es. Ein falscher Schlüssel, eine fremde `pid` oder ein manipuliertes Chiffrat scheitern. Nach 72 h + 1 s ist es gelöscht. Nach `erhalten{sha256}` wird es sofort gelöscht. | Hub-S7 [da] „Abholfach verschlüsselt, gebunden, ablaufend; Zwangs-Checkout löscht erst nach Sicherung“; S6-i2 [da] „Abholfach-Kryptografie …“, „Empfang Abholfach: anderer pid/falscher Schlüssel blockiert …“ | S7, S6 |
| T15.8 | Das Abholfach wird beim nächsten Check-in **vor** der Sync-Auswahl angeboten. „Später abholen“ lässt es liegen. | Rust S2 [da] `pickup_is_offered_before_selection_and_can_be_collected` + Screenshot `shots/t15-abholfach.png` (`COPI_SCENE=v3-abholfach` [da]) | S2 |
| T15.9 | Nach dem Check-out kann keine späte Session- oder Puls-Nachricht den Arbeitsbereich wieder öffnen. Gekoppelte Handys sind abgemeldet (401). | Rust S2 [da] `i2_checkout_invalidates_late_session_and_heartbeat_events`; Vorschlag S5 `scripts/test-checkout-revokes-mobile.cjs` | S2, S5 |
| T15.10 | Ein Check-out bricht laufende pi-Prozesse ab. Die Freigabe antwortet erst, wenn alle Prozesse beendet sind. | Vorschlag Hub-S3 `freigabe_wartet_auf_prozessende` | S3 |

## T16 · Seminar: Chat, DMs, Mitsteuern (A9, KONZEPT B7, FLOWS F8/F9)

| # | Kriterium | Nachweis | Prüft |
|---|---|---|---|
| T16.1 | Ohne gültigen Geräteausweis gibt es keinen Peer im Seminar. Ein Ausweis eines fremden Empfangsschlüssels wird abgewiesen. Name und Rolle stammen nur aus dem Ausweis. | Rust S8 [da] `no_certificate_no_control`, `participant_cannot_request_control_or_spoof_role` | S8 |
| T16.2 | Nur die Rolle Seminarleitung kann „Mitsteuern anfragen“. Bei Teilnehmenden und Gästen fehlt der Knopf, und eine Anfrage wird im Automaten abgewiesen. | Rust S8 [da] `participant_cannot_request_control_or_spoof_role` | S8 |
| T16.3 | **Ohne Zustimmung keine Eingabe:** Bei „Ablehnen“, „Nur ansehen“, Esc oder unbeantworteter Anfrage erreicht kein Prompt der Seminarleitung den Hub (Mock-Hub zählt 0 `senden`). Erst nach „Erlauben 30 min“ geht ein Prompt durch. | S6 [da] `test/i2/seminar-consent.rs` `no_consent_never_reaches_hub_then_explicit_consent_can_send`, `view_only_rejection_checkout_and_wrong_session_never_send`; S6-i2 [da] „Mitsteuern: ohne Zustimmung, nur Ansicht, Ablehnung und Check-out erreichen keinen Hub“; S8 [da] `rejection_persists_nothing` | S6, S8 |
| T16.4 | Rahmen und Leiste sind in jeder Ansicht immer sichtbar, solange die Freigabe aktiv ist: Chat, Dateien, Seminar, Einstellungen offen, Fenster schmal (900 px). Die Leiste zeigt die Restzeit in Minuten und „Beenden“. | Screenshots `shots/t16-mitsteuern-chat.png`, `shots/t16-mitsteuern-dateien.png`, `shots/t16-mitsteuern-schmal.png` (`--size 900x700`; Szene `v3-mitsteuern` fehlt noch, heute nur `v3-seminar-platzhalter` auf `v3/s2`) + Vorschlag S8 `control_frame_visible_in_every_view` | S8, S1 (Review) |
| T16.5 | Prompts der Seminarleitung erscheinen im Verlauf als „Aylin · Seminarleitung“ mit Farbmarke und tragen `von:{person,rolle}`. Eine Rückfrage beantwortet weiter Mira, die Seminarleitung kann sie nicht beantworten. | Rust S8 [da] `leader_prompt_visibly_marked_and_no_requester_attribution` | S8 |
| T16.6 | Ende bei Check-out (beide Seiten), bei Ablauf (30 min bzw. 120 min, manipulierte Uhr), bei Verbindungsverlust > 60 s (bei 59 s noch aktiv), bei „Beenden“ und bei „Zurückgeben“. Danach erreicht kein Prompt mehr den Hub. | Rust S8 [da] `expiry_stops_control`, `view_only_stop_disconnect_checkout_and_session_binding` | S8, S6 |
| T16.7 | Gespiegelt wird nur die freigegebene Session. Andere Sessions und Geheimnisse wie Tokens und Schlüssel bleiben unsichtbar. | Rust S8 [da] `mirror_only_selected_session_and_redacts_secrets` | S8 |
| T16.8 | Seminarchat und DMs: Eine Nachricht kommt beim richtigen Peer an. Eine DM sehen nur die zwei Beteiligten. Ein nicht gesendeter Text bleibt im Feld mit „Nachricht nicht gesendet.“. Chat und DMs werden auf R2 nicht gespeichert. | Vorschlag S8 `dm_only_two_peers`, `unsent_message_stays_in_field`; `grep -r` im Hub-State nach dem Testtext → leer | S8, S6 |
