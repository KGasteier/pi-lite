# cocopi v3 · Akzeptanzkriterien I1 (S1)

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
| T1.11 | Hybrid ohne lokales pi installiert **nichts** und zeigt den Text aus FLOWS F5 3F. | `hybrid_without_pi_explains_not_installs` (Fallen-Skripte `npm`, `curl`, `brew` werden nicht aufgerufen) |

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
| T2.13 | Die QR-URL trägt den Code im Fragment (`#`), nicht im Pfad oder in der Query. | `qr_url_code_in_fragment` |

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

## A4 · Kontomodell

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
