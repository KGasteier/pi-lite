# I1-S3-review · Sicherheitsreview S3 (Opus)

**Stand:** 2026-10-28.
**Geprüft:**
- pi-lite `v3/s3` @ `64e7154`, Diff gegen `cocopi-v3`;
- pi-hub `v3/s3` @ `365cadc`, Diff gegen `master`.

Ich habe keinen Code geändert. Die PoC-Skripte liegen nur in `/tmp/i1s3rev/` (`poc.mjs`, `poc2.mjs`, `poc3.mjs`). Sie laufen auf localhost in Temp-Verzeichnissen und rufen kein Modell auf.

## Ergebnis

**Freigabe für Merge in `cocopi-v3`: ja mit Auflagen.**
- Auflage: H1–H3 sind vor dem Merge zu beheben, jeweils mit Regressionstest.
- Ohne diese Korrekturen lautet das Ergebnis **nein**.

Begründung:
- H1 umgeht die Handybestätigung aus A3 vollständig.
- H3 bricht die Sessiongrenze T3.
- Beide sind per PoC belegt. Die Korrekturen sind klein.

## Ausgeführte Nachweise

| Befehl | Ergebnis | Log |
|---|---|---|
| `node test/v1.mjs` (Hub) | 31/31 ok, EXIT 0 | `/tmp/I1-S3-review-hub.log` |
| `node test/smoke.mjs` (Hub) | ok, EXIT 0 | `/tmp/I1-S3-review-smoke.log` |
| `cargo test -p pi-backend` | Lib 92 ok; `tests/hub.rs` 1 ok; `ssh.rs` 3 ok, 2 ignored (live); `client_against_started_node_hub_mock` ist wirklich gelaufen, nicht übersprungen | `/tmp/I1-S3-review-cargo.log` |
| `cargo clippy -p pi-backend -- -D warnings` | EXIT 0 | `/tmp/I1-S3-review-clippy.log` |
| PoC 1–11 (eigene Skripte) | Ergebnisse siehe Befunde | – |

## Befunde

### Hoch

**H1 · Zweiter Zugang ohne Handybestätigung: die Einladung wird als „handy“ eingelöst.**
- **Stelle:** `lib/devices-v1.mjs:132-139`
- **Angriff:** Wer einen geleakten oder zweiten e-Link hat, sendet `geraeteart:"handy"`. Das Gerät wird sofort registriert, obwohl im Slot schon ein Handy gekoppelt ist (PoC1: `einladung_eingeloest`, Login ok).
  - Danach hat dieses Gerät vollen Zugriff: Sessions lesen, senden, antworten.
  - Es kann als „Handy“ außerdem selbst b-Codes für weitere Desktops bestätigen.
- **Vorschlag:**
  - Hat der Slot schon irgendein nicht widerrufenes Gerät, braucht jede e-Einlösung eine Bestätigung, egal welche Geräteart angegeben ist.
  - Bei vorhandenem Handy läuft sie über b, sonst über den Desktop (`approve`).
  - Die Geräteart stammt vom Client und darf nie entscheiden, ob bestätigt werden muss.
- **Test:** `invite_as_phone_with_existing_phone_needs_confirmation`. Erwartet: `koppeln_wartet`, die Geräteliste bleibt unverändert, ein Login ist nicht möglich.

**H2 · Der b-Code wird per SSE an alle Geräte des Slots gesendet und liegt im Replay-Puffer.**
- **Stelle:** `lib/hub-v1.mjs:88` zusammen mit `lib/devices-v1.mjs:143-145`
- **Angriff:** `emit(result)` sendet das gesamte Einlöseergebnis, einschließlich `bestaetigung.code` und `url`, an jeden SSE-Client (PoC10: das Handy erhält den Code im Stream). Dazu kommt der RAM-Ring mit 1000 Ereignissen.
  - Das Scannen wird damit überflüssig.
  - Zusammen mit H1 bestätigt das Angreifergerät den eigenen zweiten Desktop.
- **Vorschlag:**
  - Nur `koppeln_wartet{anfrage_id, geraetename}` senden, nie `code` oder `url`.
  - Den b-Code ausschließlich in der HTTP-Antwort an den einlösenden Desktop geben.
- **Test:** `confirm_code_never_in_sse_or_replay`. Er liest den Stream und `Last-Event-ID`-Replay und prüft, dass der Code fehlt.

**H3 · T3 ist umgehbar: ein verspätetes `hub_exit` des alten Prozesses nach `ruhen` und Wiederöffnen.**
- **Stelle:** `lib/hub-v1.mjs:61-69`, `:127`
- **Angriff:** `ruhen` → sofort `oeffnen` (dieselbe ID) → das SIGTERM-Exit des alten Prozesses kommt später an. Der Wrapper der **alten** Instanz setzt dann `change(s,'fehler')` auf die **neue** Belegung.
  - Danach zählt die Session nicht mehr als offen, obwohl ihr neuer Prozess läuft.
  - Man kann eine dritte Session öffnen (PoC2: 3 lebende Prozesse, Grenze 2).
  - Wiederöffnen aus `fehler` startet einen weiteren Prozess auf **dieselbe** Sessiondatei.
  - Bei echtem pi ist das realistisch, weil `kill` asynchron ist.
- **Vorschlag:**
  - Ereignisse nur anwenden, wenn `s.inst === inst` gilt.
  - Bei `hub_exit` der aktuellen Instanz `s.inst = null` setzen.
  - `ruhen` zählt als offen, bis der alte Prozess wirklich beendet ist (`exit`), oder Wiederöffnen wird bis dahin abgewiesen.
- **Test:** `late_exit_after_rest_cannot_free_slot`. Der Fake-`stop` sendet `hub_exit` verzögert. Danach muss das dritte Öffnen 409 liefern, und es dürfen höchstens 2 Prozesse leben.

### Mittel

**M1 · Jeder Request schreibt das Register, und eine Altsperre legt den v1-Dienst lahm.**
- **Stelle:** `lib/devices-v1.mjs:56-77`. Aufrufer:
  - `person()` aus `hallo` ohne Authentisierung, `hub-v1.mjs:84`;
  - `authenticate()` bei jedem Request, `:198`;
  - `challenge()`, `:173`.
- **Angriff, zwei Varianten:**
  - Unauthentisiertes `POST /v1/hallo` erzeugt pro Aufruf Sperre + Schreiben + fsync + Rename (PoC3). Das verstärkt Schreiblast und DoS.
  - Bleibt nach einem Absturz oder `kill -9` eine `geraete.json.lock` liegen, antworten `hallo`, Anmeldung und Sessions dauerhaft mit 500 (PoC4).
  - Ein paralleler Admin-CLI-Aufruf lässt laufende Requests mit `intern` scheitern.
- **Vorschlag:**
  - Lesepfade ohne Schreiben und Sperre, mit einem In-Memory-Stand, der bei mtime-Änderung neu gelesen wird.
  - Sperrdatei mit PID/Zeitstempel und Erkennung veralteter Sperren.
  - Bei Konflikt kurz erneut versuchen.
- **Test:** `hallo_does_not_write_registry`, `stale_lock_recovered_or_clear_error`.

**M2 · Drosselung nach `req.socket.remoteAddress`.**
- **Stelle:** `lib/hub-v1.mjs:87`, `lib/devices-v1.mjs:106`
- **Angriff:**
  - Hinter dem geplanten TLS-Reverseproxy oder Tailscale-Serve teilen sich alle Absender `127.0.0.1`. Fünf Fehlversuche pro Minute sperren dann alle Einlösungen des Slots.
  - Mit IPv6-Rotation lassen sich 10 000 Absender füllen. Dann gilt `zu_viele` global für jeden neuen Absender.
- **Vorschlag:**
  - Explizites `HUB_TRUSTED_PROXY` mit genau einem vertrauenswürdigen Header.
  - IPv6 nach /64 zusammenfassen.
  - Bei vollem Absenderspeicher die ältesten Einträge verdrängen statt alle zu sperren.
- **Test:** Proxy-Konfiguration mit zwei Absendern über einen Proxy: der eine darf den anderen nicht sperren.

**M3 · Signatur ohne Kontextbindung.**
- **Stelle:** Client `copi/crates/pi-backend/src/hub_v1.rs:305`; Hub `lib/devices-v1.mjs:186`
- **Angriff:** Der Client signiert jeden Nonce-String, den der Server liefert. Wird derselbe lokale Schlüssel für zwei Hubs oder Slots genutzt (der Name kommt von der UI), kann ein böswilliger Hub A eine Challenge von Hub B weiterreichen und sich dort als das Gerät anmelden.
- **Bewertung der übrigen Challenge-Eigenschaften:** Replay ist durch einmalige Verwendung und 60 s verhindert (getestet). Die Nonce mit 32 Byte reicht.
- **Vorschlag:**
  - Signiert wird `"cocopi-v1-anmelden\n"+hub_origin+"\n"+slot+"\n"+geraet_id+"\n"+challenge_id+"\n"+nonce`.
  - Ein Schlüssel je (Hub, Slot); der Name wird vom Client aus diesen Werten abgeleitet.
  - Das ist eine Protokolländerung und gehört deshalb **vor I2**.
- **Test:** Eine Signatur für Hub A wird von Hub B abgewiesen.

**M4 · Rechte und Symlinks von `HUB_STATE` werden nicht geprüft.**
- **Stelle:** `lib/devices-v1.mjs:52`, `:61`; `lib/hub-v1.mjs:35-36`
- **Angriff:**
  - Ein vorhandenes Verzeichnis mit 0777 wird akzeptiert (PoC8). `mode` gilt nur beim Anlegen.
  - Ein Symlink `geraete.json` wird gelesen. PoC9: ein untergeschobenes Gerät `evil` wird angenommen.
  - Der feste Name `sessions-v1.json.new` folgt beim Schreiben einem Symlink.
  - Auf R2 mit zehn Unix-Konten ist ein anderer lokaler Nutzer ein reales Szenario.
- **Vorschlag:**
  - Beim Start Eigentümer == uid und `mode & 077 == 0` prüfen, sonst abbrechen.
  - Dateien per `lstat` prüfen und Symlinks ablehnen.
  - Temp-Dateien mit `wx` und Zufallsnamen anlegen.
  - `s.file` beim Wiederöffnen auf `SESSIONS_DIR` begrenzen (`hub-v1.mjs:59`).
- **Test:** `state_dir_world_writable_refused`, `registry_symlink_refused`.

**M5 · Ein hängender Prozessstart blockiert den aktiven Platz ohne Ausweg.**
- **Stelle:** `lib/hub-v1.mjs:56-64`, `:126`
- **Angriff/Fehlerfall:** Antwortet pi nie auf `get_state`, bleibt die Session dauerhaft `arbeitet`. `ruhen` wird verweigert (`arbeitet_noch`), und `stopp` hilft beim Start nicht. Der Slot ist bis zum Hub-Neustart blockiert.
- **Vorschlag:**
  - Start-Timeout, danach `fehler` und Prozess beenden.
  - `ruhen` mit erzwungenem Stopp erlauben.
- **Test:** Ein Fake, der nie antwortet, muss nach dem Timeout den Platz freigeben.

### Niedrig

- **N1 · SSE ohne Verbindungslimit** (`hub-v1.mjs:103-112`). PoC5: 200 parallele Streams eines Geräts wurden angenommen. Vorschlag: höchstens etwa 4 je Gerät und ein globales Limit. Das Token-Limit von 1000 lässt sich durch ein eigenes Gerät auffüllen (Selbst-DoS für 12 h).
- **N2 · Kein Lese-Timeout im Rust-SSE** (`hub_v1.rs:224-225`, `timeout_recv_body(None)`). Bei einer halboffenen TCP-Verbindung blockiert `next_event` für immer. Vorschlag: Lese-Timeout von etwa 45 s, passend zu `puls` alle 15 s.
- **N3 · Proxy aus Umgebungsvariablen** (`hub_v1.rs:219-228`). Der ureq-Agent übernimmt standardmäßig `HTTP(S)_PROXY`/`ALL_PROXY` (`ureq-3.4.2/src/config.rs:948`). Bei `http://127.0.0.1` könnte der Bearer-Token im Klartext an einen Proxy gehen. Das Loopback-Bypass-Verhalten habe ich **nicht verifiziert**. Vorschlag: `.proxy(None)` für Loopback oder grundsätzlich.
- **N4 · Lokaler Backend-Eintrag trotz Nur Remote:** In `app/src/net.rs:588` wird bei kaputter `backends.json` `BackendCfg::local()` am Filter von `backends::parse` vorbei eingefügt. Ein Spawn bleibt durch `pi_command` gesperrt, aber das widerspricht SICHERHEIT.md („kein lokaler Eintrag“). Vorschlag: `backends::parse("[]")`.
- **N5 · Fremdtext vom Hub:** `packet_error` (`hub_v1.rs:385-391`) zeigt `was`/`tun` des Servers ungefiltert an. Damit ist Phishing-Text eines bösartigen Hubs möglich. Vorschlag: Codes auf lokale Texte abbilden.
- **N6 · `COPI_MODE_FILE` wirkt auch im Release** (`mode.rs:34`). Das ist keine echte Grenze, weil wer die Umgebung kontrolliert ohnehin `PATH` kontrolliert. Trotzdem nur unter `cfg(test)`/debug zulassen.
- **N7 · Weitere Kleinpunkte:**
  - Die Admin-CLI kann nicht widerrufen (KONZEPT: `--widerrufen`).
  - Codes und Anfragen werden nie bereinigt.
  - Nach `rename` fehlt ein fsync auf das Verzeichnis.
  - Nach einem Hub-Absturz verwaiste pi-Kindprozesse werden nicht erfasst.
  - Schlüsseldatei:
    - Sie ist unverschlüsselt.
    - Unter Windows fehlt jede ACL-Prüfung.
    - `set_permissions` folgt einem Symlink im Wurzelverzeichnis.
    - Alles davon ist dokumentiert.

## Antworten auf die Prüffragen

**1 · E4: keine Umgehung gefunden.**
- `HYBRID` steht standardmäßig auf false. Eine kaputte oder fehlende Datei ergibt RemoteOnly. `set` schaltet erst nach erfolgreichem Persistieren um.
- Alle produktiven pi-Starts laufen über `winshim::pi_command`: `rpc.rs:45`, `cli.rs:53` und `:187`, `preview.rs:221`. Das habe ich manuell über alle `Command::new` in `copi/` geprüft.
- Die übrigen Spawns sind keine lokalen Agenten:
  - `ssh` startet entfernt;
  - außerdem `curl`, Editor, `pdftotext`, `kill` und `open`.
- Der Test `only_winshim_spawns_pi` ist nur eine Literal-Heuristik.
- `mode::init()` wird in `app/` **nirgends aufgerufen**. Das ist für I1 sicher, weil der Modus damit immer RemoteOnly bleibt, aber die Einbindung ist noch offen (S2).

**2 · A3:**
- **Einmaligkeit:** atomar, auch zwischen Prozessen (Sperre + Rename) und über einen Neustart hinweg (PoC6: `verbraucht`).
- **Ablauf der Codes:** getestet.
- **Drosselung:** siehe M2.
- **Leaks:**
  - Keine Codes oder Tokens in Logs; getestet für Code, Alt-Token und Gerätetoken.
  - Das Register enthält nur Hashes.
  - Leak über SSE: siehe H2.
  - Die CLI gibt die URL auf stdout aus; das ist dokumentiert.

**3 · Gerätebindung:**
- Challenge: Replay und Ablauf sind getestet. Die Bindung an Hub/Zweck fehlt (M3).
- Token-Ablauf: 12 h, getestet.
- Widerruf: sofort wirksam, auch für offene SSE. Das ist getestet; jeder Request prüft außerdem das Register.
- Fremder Slot: Parameter und Store-Slot sind getestet.
- Zweiter Desktop: Die Bestätigung ist umgehbar (H1, H2).

**4 · T3:**
- Paralleles Öffnen und Senden ist atomar, weil Node die Prüfungen synchron ausführt. Das ist getestet.
- Nach einem Neustart sind alle Sessions `ruht`, und es wird kein Prozess gestartet (PoC7).
- Ein `exited`-Race lässt die Grenze kippen (H3). Ein hängender Start blockiert den Slot (M5).

**5 · Hub allgemein:**
- `/api` ist im Slotbetrieb gesperrt (Test: `/api/ping` liefert 403).
- Statische Dateien: Ein Pfadausbruch ist durch `normalize` + `join` nicht möglich.
- `workspace` wird per `realpath` + `root+sep` geprüft.
- Body-Limit von 64 KiB greift (PoC11: 400).
- SSE: siehe N1. Rechte und Symlinks: siehe M4.

**6 · Rust-Client:**
- Nur rustls + webpki-roots; http nur zu numerischem Loopback.
- Keine Redirects; URLs mit Zugangsdaten werden abgelehnt.
- Antworten sind auf 2 MiB begrenzt, ebenso SSE-Zeilen und -Pakete.
- Im Code habe ich keine Panic-Pfade gefunden: Slices liegen auf ASCII-Grenzen, `Value`-Indizes sind panikfrei. Es gibt dafür **keinen Test mit bösartigem Server**.
- Schlüsseldatei:
  - Unix: 0600, Symlinks und Gruppen-/Weltrechte werden abgelehnt (getestet).
  - macOS: läuft über denselben Unix-Pfad, Keychain fehlt.
  - Windows: ungeprüft, kein Build.

**7 · Was nur behauptet und nicht durch Tests belegt ist:**
- Windows-/macOS-Schlüssel und Windows-Spawn;
- echtes TLS bzw. Reverseproxy;
- Verhalten hinter einem Proxy (Drosselung, Origin);
- Robustheit des Clients gegen bösartige Antworten bzw. SSE (kein Test);
- echtes pi mit asynchronem Exit (H3);
- `mode::init` in der App;
- Dateisperre nach einem Absturz.

Belegt sind die Testnamen aus SICHERHEIT.md „Nachweise“. Alle laufen grün.

## Vor I2 beheben

1. H1, H2, H3: Auflage für den Merge, jeweils mit Regressionstest.
2. M3: Kontextbindung der Signatur. Das ist eine Protokolländerung und muss deshalb vor dem nächsten Freeze erfolgen.
3. M1: Lesepfade ohne Schreiben, Wiederherstellung bei veralteter Sperre.
4. M4: Prüfung von Rechten, Eigentümer und Symlinks bei `HUB_STATE`; `s.file` auf `SESSIONS_DIR` begrenzen.
5. M5: Start-Timeout.
6. N4: lokaler Eintrag bei kaputter `backends.json`.
7. S2 bindet `mode::init()` ein, mit einem Test, dass es vor dem ersten Backend-Aufbau aufgerufen wird.

## Vor Pilot beheben

1. M2: Proxy- und Absender-Konfiguration. Echtes TLS bzw. Tailscale real testen, einschließlich `Origin`/`HUB_URL`.
2. N1: Limits für SSE und Tokens je Gerät.
3. N2: Lese-Timeout im Rust-SSE.
4. N3: Proxy aus der Umgebung abschalten bzw. verifizieren.
5. N5: lokale Fehlertexte statt Servertext.
6. N6: `COPI_MODE_FILE` nur in Debug-Builds.
7. N7: Widerruf über die Admin-CLI, verwaiste Kindprozesse, Bereinigung der Register.
8. Keychain (macOS) bzw. Windows-ACL für den Geräteschlüssel samt Build und Test unter Windows.
9. Client-Test gegen einen bösartigen Mock: übergroße Zeile, ungültiges UTF-8, falsches `protokoll`, Redirect.
