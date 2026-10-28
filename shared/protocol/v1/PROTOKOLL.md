# cocopi-Protokoll v1 · HTTP und SSE

Normativer I1-Vertrag (FREEZE-I1, `cocopi-slots-v1`). Alle Nachrichten enthalten `protokoll: 1` und `typ`. Optionale unbekannte Felder werden ignoriert. UTF-8-JSON; Zeitpunkte = Unix-Millisekunden. Anzeigenamen bestimmen niemals Rechte. Jeder Hub bedient genau `HUB_SLOT` (`account01` … `account10`).

## Transport und Authentisierung

Basis ist die konfigurierte `hub_url`, im Test ausschließlich `http://127.0.0.1:<port>`. Im Betrieb TLS/Tailscale vorschalten, keine öffentliche HTTP-Freigabe. Zugangstokens ausschließlich `Authorization: Bearer <token>`, niemals Query, Fragment oder Log. Keine Cookies, keine CORS-Freigabe; Browser verwenden `fetch` mit Header, auch für SSE. Sämtliche POST-Bodies enthalten `protokoll:1`. Fehler: HTTP-Status plus `{protokoll:1,typ:"fehler",code,was,tun}`.

| Methode | Pfad | Auth | Body / Antwort |
|---|---|---|---|
| POST | `/v1/hallo` | keine | `hallo{app,protokoll}` → `willkommen{min_client:1,slot,person,hub_version,vertrag}` |
| POST | `/v1/einloesen` | keine (e/k); Gerät (b) | `{art:e|k|b,code,slot?,pubkey?,geraetename?,geraeteart?}` → `einladung_eingeloest{geraet_id,slot,person}` oder `koppeln_wartet{anfrage_id}`; b bestätigt die vom Hub gespeicherte Aktion |
| POST | `/v1/anmelden/challenge` | keine | `{geraet_id}` → `challenge{challenge_id,nonce,slot,gueltig_bis}` |
| POST | `/v1/anmelden` | Ed25519-Signatur | `{challenge_id,signatur}` → `angemeldet{zugangstoken,gueltig_bis,geraet_id,slot,person}` |
| GET | `/v1/sessions` | Gerät | `sessions_liste{sessions:[session_zustand…]}` |
| POST | `/v1/sessions/oeffnen` | Desktop | `{session_id?,workspace?,titel?}` → `session_zustand` (201 neu, 200 erneut offen) |
| POST | `/v1/sessions/:id/senden` | Gerät | `{text}` → `session_zustand` (202); keine automatische Wiederholung |
| POST | `/v1/sessions/:id/antwort` | Gerät | `{frage_id,wert}` → `session_zustand` (202); nur vorhandene Rückfrage, keine automatische Zustimmung |
| POST | `/v1/sessions/:id/stopp` | Gerät | → `session_zustand` |
| POST | `/v1/sessions/:id/ruhen` | Desktop | → `session_zustand`; beendet auch einen hängenden Start zwangsweise |
| POST | `/v1/sessions/:id/ansehen` | Gerät | fertig → bereit |
| GET | `/v1/ereignisse` | Gerät | SSE (siehe unten) |
| POST | `/v1/koppeln/start` | Desktop | → `koppeln_start{code,url,gueltig_bis}`; ersetzt unbenutzten Code dieses Desktops |
| POST | `/v1/koppeln/bestaetigen` | erzeugender Desktop | `{anfrage_id,erlauben:boolean}` → `koppeln_bestaetigt{geraet_id?,erlauben}` |
| GET | `/v1/koppeln/:anfrage_id` | keine, nur unerratbare Anfrage-ID | `koppeln_wartet` / `koppeln_bestaetigt`; kein Token |
| GET | `/v1/geraete` | Desktop | `geraete_liste{geraete:[…]}` (keine Schlüssel) |
| POST | `/v1/geraete/:id/widerrufen` | Desktop oder eigenes Handy | → `widerruf{geraet_id}`; schließt alle Geräte-SSE sofort |

`pubkey` = 32 rohe Ed25519-Bytes, Base64url ohne Padding. `signatur` = 64 Signaturbytes Base64url. Signiert werden exakt die UTF-8-Bytes (LF, kein abschließender Zeilenumbruch):

```text
cocopi-v1-anmelden\n<hub_origin>\n<slot>\n<geraet_id>\n<challenge_id>\n<nonce>
```

`hub_origin` stammt aus der bewusst konfigurierten Hub-Adresse, niemals aus der Challenge: Schema + kleingeschriebener Host + Port, Standardport 443/80 entfällt, kein abschließender Slash. Server: `new URL(HUB_URL).origin`; hinter TLS muss `HUB_URL` den äußeren Ursprung enthalten. Client liest den Slot aus `hallo` und verlangt denselben Slot in `challenge`. Die Nonce ist ein zufälliger 32-Byte-Wert als Base64url. Challenge 60 s gültig, einmal verwendbar, serverseitig an Gerät/Slot gebunden. Bearer 12 h gültig, im Hub nur gehasht und an Gerät gebunden. Challenge-Endpunkt gibt keine Schlüssel zurück. Beispiele `anmelden_challenge.json` / `anmelden_req.json` gehören zum deterministischen Signaturvektor (Seed nur Testdaten): Seed `00` × 32, Ursprung `https://r2.example`, Gerät `test-geraet`, Slot `account01`. Schlüssel je (Hub, Slot) getrennt ablegen; bestehende UI-Speichernamen bleiben im I1-Client API-kompatibel, Kontextbindung greift auch bei Schlüsselwiederverwendung.

## Einladung, Koppeln, Bestätigen

CLI im Hub-Slot: `node server.mjs admin einladung --slot account01 --name Mira`. `HUB_STATE` für v1 ist ein Verzeichnis; Alt-API-Dateien bleiben unterstützt (v1 daneben in `v1/`). Register 0600, Verzeichnis 0700; Eigentümer muss die laufende UID sein. Symlinks und Gruppen-/Weltrechte werden abgewiesen. Dateien werden mit exklusivem Zufallsnamen geschrieben, synchronisiert und atomar ersetzt (einschließlich Verzeichnis-Sync). Sessionpfade beim Wiederöffnen bleiben unter `PI_SESSIONS_DIR`. Admin und laufender Hub koordinieren Änderungen per exklusiver Sperrdatei mit PID/Zeitstempel; drei kurze Wiederholungen bei belegter Sperre. Tote PID → klarer Fehler `register_veraltet`, keine automatische Löschung (kein Rennen mit einem anderen Aufräumer). Leere/beschädigte Sperre → `register_belegt`. Lesepfade wie `hallo` laden Fremdänderungen ohne Schreiben oder Sperre. Keine zweite Hubinstanz je Slot.

Erzeuger schreiben `<hub_url>/c#1.<art>.<code>`. Parser akzeptieren außerdem `?c=1.<art>.<code>` und Rohcode mit Art aus Kontext. 16 Crockford-Base32-Zeichen (80 Bit), in Vierergruppen. Codes nur als SHA256 des kanonischen Codes gespeichert. e: 24 h, k: 5 min, b: 2 min. Atomare Einlösung vor Registrierung; 5 fehlgeschlagene Versuche/Minute je Absender (nicht ungeprüft `X-Forwarded-For` vertrauen), nach 10 Fehlversuchen auf einen vorhandenen Code gesperrt. Ein Slot-Parameter kann nur den konfigurierten Slot bestätigen, nicht wechseln.

Koppeln registriert erst nach Zustimmung des erzeugenden Desktops. Dessen Widerruf/Ablauf macht offene Anfragen ungültig. Abgelehnte Anfragen speichern kein Gerät. Neues Handy authentisiert sich anschließend mit Challenge, nicht mit einem Token aus einer frei lesbaren Poll-Antwort. Jede e-Einlösung bei mindestens einem nicht widerrufenen Gerät braucht Bestätigung, unabhängig von der clientseitigen Geräteart. Mit vorhandenem Handy: b-Code; ohne Handy: Zustimmung des bestehenden Desktops über `koppeln/bestaetigen`. Bis dahin bleibt die Geräteliste unverändert und Anmeldung ist unmöglich. `bestaetigung{code,url,…}` steht ausschließlich in der HTTP-Antwort an das einlösende Gerät, nie in SSE oder Replay. I1-Implementierungsgrenzen stehen in SICHERHEIT.md.

## Zustände und Grenzen

`session_zustand{session_id,zustand,seit,titel,workspace,frage?}` ist die alleinige Zustandsquelle. Zustände: `ruht`, `arbeitet`, `wartet`, `fertig`, `bereit`, `fehler`. Offen zählen arbeitet/wartet/fertig/bereit, aktiv zählen arbeitet/wartet (wartende Rückfragen geben den Arbeitsplatz nicht frei). `HUB_MAX_OPEN_SESSIONS=2`, `HUB_MAX_ACTIVE_SESSIONS=1`; Prüfung und Reservierung atomar vor Spawn/Prompt. `ruht` und `fehler` zählen erst dann nicht offen, wenn ihr Prozess tatsächlich beendet ist. Bis dahin ist Wiederöffnen mit 409 `arbeitet_noch` gesperrt. Ereignisse alter Instanzen werden verworfen. Ruhen beendet den Prozess zwangsweise, nicht die Sessiondatei. `HUB_START_TIMEOUT_MS` (Standard 60000) begrenzt den Start bis zur erfolgreichen `get_state`-Antwort: danach `fehler` und erzwungener Prozessstopp. Wiederöffnen startet einen neuen Prozess, kein RAM-Resume. Dritte offene → 409 `zu_viele_offen`, zweite aktive → 409 `zu_viele_aktiv`. Keine Queue in I1 (konkreter S3-Auftrag; Abweichung ACCEPTANCE T3.5 an K gemeldet). Kein Löschen bei Ablehnung.

## SSE und Wiederverbindung

`GET /v1/ereignisse`: `Content-Type: text/event-stream`, `Cache-Control: no-store`; jede JSON-Nachricht in `data:`, Ereignisart in `event:`, laufende ID in `id:`. Ereignisse `sessions_liste`, `session_zustand`, `session_ereignis{session_id,pi_event}`, `koppeln_wartet`, `koppeln_bestaetigt`, `widerruf`, `puls`. `puls{t}` alle 15 s, nach 45 s ohne Empfang Anzeige veraltet („Stand hh:mm“), nie lokal ausweichen. Widerruf/Tokenablauf schließt die Verbindung auch ohne neuen Request.

Replay mit `Last-Event-ID: <hub-epoch>:<seq>`. Begrenzter RAM-Ring; unbekannte Epoche/zu alte ID → `fehler{code:"neu_abgleichen"}` und frische `sessions_liste`. Clients ersetzen dann ihre Liste und deduplizieren IDs; keine Garantie auf lückenlose alte Chat-Historie oder Exactly-once-Prompts. Keine automatische Promptwiederholung. Tokenablauf → erneut Challenge, dann SSE verbinden. Rust verwendet keinen Umgebungs-Proxy. Sein SSE-Body ist auf 45 s begrenzt (ureq 3.4: Gesamtbudget, kein gleitender Idle-Timer); danach bewusst neu verbinden und letzte ID zum Replay nutzen. Ein erfolgreicher Puls hebt dieses Gesamtbudget nicht auf.

## Fehlertexte (verbindlich)

Rust-API bleibt additiv kompatibel bei `Result<Value, String>`: Protokollfehler haben exakt die Form `[code] was tun`. S2 kann das Präfix ohne zweiten HTTP-Client auswerten. Ein typisierter Rückgabefehler würde bestehende Aufrufer brechen; deshalb hier kein API-Wechsel. Alle unten bekannten Codes werden auf fest eingebaute deutsche Clienttexte abgebildet, fremde Servertexte werden dafür ignoriert. Nur bei unbekannten, syntaktisch gültigen Codes dienen `was`/`tun` als Rückfall. Ungültiges Codepräfix fällt auf `intern` zurück.

| Code | HTTP | was | tun |
|---|---|---|---|
| protokoll_zu_alt | 426 | cocopi muss aktualisiert werden. R2 spricht eine neuere Sprache. | Lade die neue cocopi-Version bei deiner Seminarleitung herunter. |
| hub_zu_alt | 426 | R2 ist noch nicht auf dem neuesten Stand. | Sag der Seminarleitung Bescheid. Deine Sessions sind sicher. |
| abgelaufen | 410 | Dieser Code ist abgelaufen. | Bitte um einen neuen Code. |
| verbraucht | 409 | Dieser Code wurde schon benutzt. | Erzeuge einen neuen Code. Wenn du das nicht warst, sag deiner Seminarleitung Bescheid. |
| unbekannt | 404 | Dieser Code oder Eintrag wurde nicht gefunden. | Prüfe die Zeichen oder öffne den Link direkt. |
| zu_viele | 429 | Zu viele Versuche. | Warte eine Minute und versuche es dann noch einmal. |
| gesperrt | 403 | Dieser Code wurde gesperrt. | Bitte um einen neuen Code. |
| fremder_slot | 403 | Dieses Gerät gehört zu einem anderen Konto. | Nutze das Gerät für dieses Konto. |
| nicht_angemeldet | 401 | Dieses Gerät ist nicht mehr angemeldet. | Melde dich mit einer neuen Einladung oder durch Koppeln wieder an. |
| bestaetigung_noetig | 409 | Bitte am bestehenden Gerät bestätigen. | Öffne Einstellungen › Geräte am angemeldeten Rechner. |
| abgelehnt | 403 | Der Rechner hat abgelehnt. Es wurde nichts gespeichert. | Erzeuge am Rechner einen neuen Code. |
| zu_viele_offen | 409 | Du hast schon 2 offene Sessions. | Lass eine ruhen, dann kannst du diese öffnen. Es geht nichts verloren. |
| zu_viele_aktiv | 409 | Eine Session arbeitet schon. | Warte, bis sie fertig ist, oder brich sie ab. |
| arbeitet_noch | 409 | Diese Session arbeitet oder beendet sich noch. | Warte kurz oder lass sie ruhen. |
| pfad_nicht_erlaubt | 403 | Dieser Ordner ist nicht freigegeben. | Wähle einen Projektordner deines Kontos. |
| neu_abgleichen | 409 | Die Verbindung war zu lange unterbrochen. | Die Sessionliste wird neu geladen. Prüfe den letzten Auftrag, bevor du erneut sendest. |
| ungueltig | 400 | Die Anfrage konnte nicht gelesen werden. | Prüfe die Eingabe und versuche es erneut. |
| register_belegt | 503 | Die Geräteliste ist gerade gesperrt. | Warte kurz. Bleibt die Meldung, frage die Seminarleitung. |
| register_veraltet | 503 | Die Geräteliste hat eine Sperre eines beendeten Prozesses. | Bitte die Seminarleitung, die alte Sperrdatei zu prüfen und zu entfernen. |
| intern | 500 | R2 konnte die Anfrage nicht abschließen. | Versuche es später erneut oder frage die Seminarleitung. |

Keine Telemetrie. TLS-Vorschaltung, Hostpinning, globale CPU-Queue, sichere Transfers und echte Kontenprovisionierung sind nicht Teil dieses I1-Codes.
