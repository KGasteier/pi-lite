# I1-S3-fix · Sicherheitsnachbesserungen

Stand: 28.10.2026, Abschluss ca. 20:25 CET. Beginn laut Laufstatus 20:04:53 CET; deutlich innerhalb 100 Minuten.

## Ergebnis und Übergabestände

Pflichtpunkte H1–H3, Fehlercode-Erhalt/N5, M3, M5 und N2/N3 umgesetzt; zusätzlich M1 und die angeforderten M4-Härtungen. **Keine Pilot-/Deploymentfreigabe:** offene Reviewpunkte und Plattform-/Livegrenzen bleiben unten ausdrücklich stehen.

| Repository / eigener Worktree | Branch | Ausgang | Ergebnis |
|---|---|---|---|
| `/home/medienhaus/Work/pi-lite-wt/s3` | `v3/s3` | `64e7154` | **`0f2716a`** |
| `/home/medienhaus/Work/pi-hub-wt/s3` | `v3/s3` | `365cadc` | **`e2eabfa`** |

Beide Worktrees nach den Commits sauber. Keine fremden Änderungen übernommen; keine Abhängigkeiten, Installationen, Netzwerk-/Systemkonfiguration oder produktiven Hubprozesse verändert. Kein Push, Merge, Reset oder Stash. Cargo ausschließlich offline/locked. Hub-Commits mit vorgegebener Identität. Keine Live-Modellprompts.

## Finding → Commit → Nachweis

| Finding | Umsetzung / Commit | Test / Ergebnis |
|---|---|---|
| **H1** Bestätigung per `geraeteart` umgehbar | Hub `84728e4`: jede e-Einlösung bei mindestens einem nicht widerrufenen Gerät bleibt zunächst pending, unabhängig von beantragter Geräteart. Mit Handy b-Bestätigung, andernfalls Zustimmung des vorhandenen Desktops. | `invite_as_phone_with_existing_phone_needs_confirmation`, `invite_with_existing_desktop_needs_approval`: kein neues Gerät vor Bestätigung; Hub-v1 grün. |
| **H2** b-Code in SSE/Replay | Hub `84728e4`: SSE erhält nur explizit ausgewählte Anfrage-ID/Name. `bestaetigung` einschließlich Code/URL verbleibt ausschließlich in der Einlöse-HTTP-Antwort. | `confirm_code_never_in_sse_or_replay`: Live-Stream und Replay ohne Code/URL/Bestätigungsobjekt. |
| **H3** verspätete Instanzereignisse / freie Kapazität vor Exit | Hub `84728e4`: Ereignisse nur bei `s.inst === inst`; `ruht`/`fehler` mit lebender Instanz zählt weiter offen; Wiederöffnen und Senden während Stopp gesperrt. Erst Exit entfernt Instanz. | `late_exit_after_rest_cannot_free_slot`, vorhandene Parallel-/Kapazitätstests: verzögerter Exit, Wiederöffnen, alte Ereignisse, dritte Session. |
| **Fehlercodes + N5** | pi-lite `0e77243`, `0f2716a`: API bleibt `Result<Value, String>`; Protokollfehler beginnen mit `[code] `. Bekannte Codes ausschließlich mit lokalen Texten; unbekannte gültige Codes mit dokumentiertem Servertext-Fallback. | `error_codes_use_local_text_and_unknown_fallback`; echter lokaler HTTP-Rundlauf prüft `[verbraucht]` und `[nicht_angemeldet]`. |
| **M3** kontextlose Signatur | Hub `091ce47`; pi-lite `0e77243`: exakter LF-Text `cocopi-v1-anmelden\n<hub_origin>\n<slot>\n<geraet_id>\n<challenge_id>\n<nonce>`, kein Schluss-LF. Ursprung aus bewusster Client-Konfiguration / `HUB_URL`, nicht aus Challenge. Challenge enthält Slot; Client vergleicht mit `hallo`. Schema, Protokoll, Beispiele aktualisiert. | `signature_for_hub_a_rejected_by_hub_b`, Rust `login_signature_context_and_canonical_origin`, `signature_matches_protocol_example`; deterministischer Ed25519-Vektor in Node und Rust identisch. |
| **M5** hängender Start | Hub `091ce47`: `HUB_START_TIMEOUT_MS`, Standard 60000; positive Ganzzahl. Timer bis erfolgreichem `get_state`, danach `fehler` und `stop(true)`/SIGKILL. Erzwungenes Ruhen erlaubt. | `start_timeout_and_forced_rest_release_slot`: nicht antwortender Fake wird beendet; Kapazität erst nach Exit frei; zusätzlich erzwungenes Ruhen geprüft. |
| **N2** SSE ohne Lesebudget | pi-lite `0e77243`: SSE `timeout_recv_body(45 s)`. | `http_agents_disable_proxy_and_bound_sse_reads` prüft Agent-Konfiguration; keine reale 45-s-Halboffen-Verbindung getestet. ureq-Gesamtbudget, nicht gleitender Idle-Timer, siehe Grenzen. |
| **N3** Umgebungsproxy | pi-lite `0e77243`: `.proxy(None)` auf beiden Agenten. | Derselbe Konfigurationstest prüft Proxy `None` für normale Requests und SSE. Kein echter Proxyserver betrieben. |
| **M1** lesendes `hallo`, veraltete Sperre | Hub `e2eabfa`: `inspect` statt Schreibtransaktion auf Lesepfaden. Exklusive Sperre enthält PID/Zeitstempel, drei kurze Wiederholungen bei belegter Sperre; tote PID → `register_veraltet`, laufende/beschädigte Sperre → `register_belegt`. Keine automatische Löschung fremder Sperre. | `hallo_does_not_write_registry` prüft Nichterstellung sowie stabile Inode/mtime; `stale_lock_recovered_or_clear_error` prüft tote und lebende PID sowie Lesen trotz Sperre. |
| **M4** State-Dateien | Hub `e2eabfa`: privater Eigentümer/Modus, `lstat`, Symlinkablehnung, `O_NOFOLLOW` beim Lesen und Prüfung des offenen Deskriptors. Geräte-/Sessionregister mit exklusivem Zufalls-Tempnamen, Sync/Rename/Verzeichnis-Sync. Sessionpfade beim Wiederöffnen unter `PI_SESSIONS_DIR`; Alt-Registry wird im Slotbetrieb nicht zusätzlich persistiert. | `state_dir_world_writable_refused`, `registry_symlink_refused`, `state_dir_symlink_and_public_registry_refused`, `session_registry_symlink_and_outside_path_refused`. Fremde UID nicht praktisch erzeugt; Prüfung im Code vorhanden. |

## Ausgeführte Prüfungen

- `cargo test --offline --locked --manifest-path copi/Cargo.toml -p pi-backend -- --nocapture`: **96 Unit-Tests bestanden**, `tests/hub.rs` **1 bestanden**, `tests/ssh.rs` **3 als bestanden gemeldet, 2 Live-Tests ignoriert**, Doc-Tests 0. **Einschränkung:** einer der drei SSH-Tests meldet selbst `node/socat fehlt – übersprungen`; kein echter SSH-Tunnel-Nachweis. Der v1-Node-Hub-Clienttest lief wirklich und wurde nicht übersprungen.
- Für den abschließenden Cargo-Lauf beide Verweise explizit auf den eigenen Hub gesetzt: `COCOPI_TEST_HUB_DIR=/home/medienhaus/Work/pi-hub-wt/s3` **und** `PI_HUB_DIR=/home/medienhaus/Work/pi-hub-wt/s3` (Alt-Integration verwendet letzteren). Target `/home/medienhaus/Work/pi-lite-wt/target-s3`, Jobs 3.
- `cargo clippy --offline --locked --manifest-path copi/Cargo.toml -p pi-backend -- -D warnings`: **EXIT 0**.
- Hub `node test/v1.mjs`: **43 Prüfungen bestanden**.
- Hub `node test/smoke.mjs`: **alle Tests bestanden** (Mock/Fake, keine Modelle).
- `node --check` für `server.mjs`, beide v1-Module und `test/v1.mjs`: erfolgreich.
- Python `jsonschema.Draft202012Validator`: Schema gültig und **23 JSON-Beispiele gültig**; keine Pakete installiert.
- `git diff --check` beider Worktrees: erfolgreich. Nur `hub_v1.rs` gezielt formatiert, keine Workspace-Formatierungsänderung.

Logs (flüchtig): `/tmp/I1-S3-fix-cargo-final.log`, `/tmp/I1-S3-fix-clippy-final.log`, `/tmp/I1-S3-fix-hub.log`, `/tmp/I1-S3-fix-smoke.log`, `/tmp/I1-S3-fix-schema.log`. Der im Cargo-Log sichtbare simulierte Panic gehört zum erfolgreichen Catch-/Restoration-Test, nicht zu einem fehlgeschlagenen Test.

## Schnittstelle / Anfragen an andere Rollen

### S2 Desktop

1. Fehleradapter kann die vereinbarten Präfixe weiterhin unverändert auswerten: `[abgelaufen]`, `[verbraucht]`, `[unbekannt]`, `[zu_viele]`, `[zu_viele_offen]`, `[nicht_angemeldet]` usw. Kein öffentlicher Rückgabetyp geändert. Neu: `register_belegt`, `register_veraltet` (503); lokal erklären, keine automatische gewaltsame Entsperrung.
2. Login macht vor Challenge einen zusätzlichen `hallo`-Request und verlangt den Slot in der Challenge. Gegen alte Server ohne Kontextsignatur ist dieser Stand absichtlich nicht kompatibel; Hub und Client gemeinsam integrieren.
3. SSE läuft bei ureq nach insgesamt 45 s aus, auch mit Pulsen. Nach Timeout mit letzter Ereignis-ID neu verbinden, Replay deduplizieren; bei `neu_abgleichen` Snapshot ersetzen. **Keine Prompts automatisch wiederholen.** Reconnect/Fehleranzeige in der tatsächlichen Desktop-Worker-Schleife abnehmen.
4. Vorhandene Desktopzustimmung auch für weitere e-Einladungen ohne Handy anbieten. Bei vorhandenem Handy b-Code ausschließlich aus der konkreten HTTP-Antwort des einlösenden Desktops anzeigen, niemals aus SSE ableiten.
5. Review N4 (lokaler Fallback-Eintrag) und `mode::init()` vor Backendaufbau bleiben S2-Auftrag; nicht in S3-Dateien geändert. Lokale Speichernamen möglichst aus (Ursprung, Slot) ableiten; diese API bleibt zunächst kompatibel.

### S5 / Mobil / Protokollabnahme

Exakte neue Signaturbytes übernehmen, Challenge-Slot validieren; deterministischen Vektor unter `shared/protocol/v1/examples/anmelden_{challenge,req}.json` nutzen. Keine Nonce-only-Signaturen mehr. Das Handy muss b-Code bewusst über den Einlöseweg erhalten (QR/Benutzereingabe), nicht aus SSE. Native und mobile Clients gegen denselben neuen Hubstand testen.

### K / Betrieb / erneuter Review

- Commits nur gezielt integrieren; kein Merge durch S3 erfolgt. Pflichtpunkte mit unabhängigem Reviewer erneut prüfen.
- Bei äußerem TLS/Reverseproxy **`HUB_URL` explizit auf den äußeren Ursprung setzen**; Default passt nur zum tatsächlichen lokalen Listener. Kein Vertrauen in unkonfigurierte Forwarded-Header ergänzt.
- Vorhandene State-Verzeichnisse/-Dateien müssen dem Dienstkonto gehören und privat sein; Fix chmodt fremde/öffentliche Bestände nicht still passend. Alte leere Crash-Locks liefern klaren Sperrfehler. Veraltete Sperren nur nach Kontrolle und gestoppten Schreibern manuell entfernen; PID-Wiederverwendung kann einen alten Lock wie einen belegten aussehen lassen.

## Nachgewiesene und offene Grenzen

**Nachgewiesen:** lokale Linux-Tests mit Node-Mock/Fake einschließlich realem HTTP/SSE, tatsächlichem Fake-Kindprozess, verzögerten Exit-Ereignissen, Code-Geheimhaltung in Live-/Replay-Streams, Schema/Signaturvektor und Server-A/B-Abweisung. Keine Aussage über produktive Inbetriebnahme.

**Weiter offen / nicht überbehaupten:**

- Kein echtes Modell, kein produktiver Hubneustart, kein WAN/Tailscale-/TLS-/Reverseproxy-/Handytest, keine Mac-/Windows-Builds oder Keychain/Windows-ACL-Abnahme.
- 45-s-SSE ist ein **Gesamtbodybudget** von ureq 3.4.2, kein durch Pulse verlängerter Idle-Timeout. Konfiguration und kurzer SSE-Rundlauf geprüft, tatsächlicher 45-s-Ablauf und UI-Reconnect hier nicht live getestet.
- M4 prüft State-Endverzeichnis und Registerdateien; keine vollständige `openat`-/dirfd-basierte Absicherung sämtlicher Vorfahren und kein Schutz gegen einen bereits kompromittierten gleichen Unixbenutzer oder einen austauschbaren übergeordneten Pfad. Produktive State-Wurzel weiterhin unter vertrauenswürdigem privaten Home halten. Gezielter Fremd-UID-Test nicht ausgeführt.
- Starttimeout/erzwungenes Ruhen sendet SIGKILL an den verwalteten Prozess, **keine bewiesene Prozessbaum-/cgroup-Terminierung**. Tatsächlicher asynchroner Exit einer produktiven pi-Instanz, geerbte Tool-Kinder und Crash-Waisen bleiben separat abzunehmen.
- M1 bietet sichere Fehlermeldung statt automatischer Stale-Lock-Reparatur. Unlesbare/alte leere Sperren können blockieren, aber nicht mehr anonym als generischer interner Fehler.
- Nicht Teil dieses Fixstands: M2 Proxy-/Absenderdrosselung, N1 SSE-/Tokenlimits je Gerät, N4/N6 Desktop/Mode-Themen, N7 CLI-Widerruf und Registerbereinigung sowie verwaiste Prozesse. Unbekannte Fehlercodes dürfen gemäß Auftrag noch Servertext zeigen; nur bekannte Codes sind lokalisiert.
- Kontextsignatur schützt bei Schlüsselwiederverwendung die Hub-Ursprungsbindung; automatische getrennte Schlüsselablage je Hub/Slot und persistente UI-Slotbindung sind noch nicht eingeführt. Slot wird im aktuellen Client aus `hallo` genommen und gegen Challenge geprüft.
- Persistenz bleibt Metadaten/Sessionpfad, kein RAM-Resume; SSE-Ring begrenzt und flüchtig, keine lückenlose Historien- oder Exactly-once-Garantie.

Finale Übergabe: **FIX fertig 0f2716a e2eabfa**.
