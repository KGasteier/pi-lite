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

## I2 · Empfang, Pool und Belegung (additiv, FREEZE-I2 A6)

I1 bleibt unverändert verwendbar. Im Poolbetrieb (`HUB_EMPFANG_PUBKEY` gesetzt) sind Geräte nur während einer gültigen Belegung nutzbar. Personen wählen keinen Unix-Slot selbst. Der Empfang läuft lokal `127.0.0.1:8800`; außen werden `/v1/empfang/*` dorthin und `/slot/accountNN/*` zum jeweiligen Hub weitergeleitet. Letzterer Präfix wird am Proxy entfernt. Der Rust-Hub-Client akzeptiert diese zehn expliziten Slot-Basispfade, keine beliebigen Pfade.

### Empfangs-Endpunkte

| Methode | Pfad | Auth | Body / Antwort |
|---|---|---|---|
| POST | `/v1/empfang/hallo` | keine | `{protokoll:1}` → `empfang_willkommen{slots_gesamt,slots_frei,warteschlange,oidc:{issuer,client_id,scopes},empfang_pubkey,vertrag:"cocopi-pool-v1"}` |
| POST | `/v1/empfang/checkin` | OIDC oder Einladung | `{nachweis:{art:"oidc",id_token,nonce}\|{art:"einladung",code},geraet:{pubkey,name,art:"desktop"},net_pubkey?,archiv_pubkey}` → **201** `eingecheckt{checkin_id,slot,hub_pfad,person,rolle,belegung,geraeteausweis,gueltig_bis,geraet_id?}` oder **202** `warteschlange{warte_id,platz,geschaetzt_min?}` |
| GET | `/v1/empfang/warten/:warte_id` | unerratbare Warte-ID | **202** `warteschlange` oder **200** `eingecheckt`; FIFO, verfällt 90 s nach letzter Abfrage |
| POST | `/v1/empfang/challenge` | keine | `{checkin_id}` → `challenge{challenge_id,nonce,gueltig_bis}` (60 s, einmalig) |
| POST | `/v1/empfang/checkout` | Gerät (siehe unten) | `{checkin_id}` → `checkout_bereit{checkout_id,archiv_id?,sha256?,bytes?}`; Archiv muss zuerst lokal geprüft und rückgespielt werden |
| POST | `/v1/empfang/checkout/:checkout_id/erhalten` | Gerät | `{sha256}` → `ausgecheckt`; danach Leeren und Freigabe, niemals bei fehlender Archivbestätigung |
| GET | `/v1/empfang/status` | Gerät, Rolle seminarleitung | `empfang_status{belegung:[{slot,person,seit,zustand}]}`; keine Sessioninhalte |

`archiv_pubkey` = roher X25519-Schlüssel (32 Bytes, Base64url), nötig für den verschlüsselten Rückläufer bei Zwangs-Check-out. Kein privater Schlüssel verlässt das Gerät. `net_pubkey` bindet den P2P-Knoten; ohne solchen Schlüssel bleibt Seminar gesperrt. `pool_voll` ist **kein Fehler**: der HTTP-202-Erfolgszustand heißt `warteschlange`, nicht `fehler`; die App zeigt den Platz.

Geschützte Empfangsanfragen verwenden je eine frische Challenge. Die Anfrage führt `X-Cocopi-Checkin`, `X-Cocopi-Challenge`, `X-Cocopi-Signatur`. Signiert wird (LF, kein abschließender LF):

```text
cocopi-v1-empfang\n<empfang_origin>\n<checkin_id>\n<challenge_id>\n<nonce>
```

Ursprung wie bei I1 kanonisiert. Empfang prüft die Signatur gegen das Gerät der Belegung und verbraucht die Challenge einmalig; weder Bearer noch OIDC-Token werden für Slot-Hub-Login wiederverwendet. I1-Hub-Challenge bleibt unverändert. Der Rust-Client exportiert `pi_backend::hub_v1::empfang::{Empfang,verify_signed,canonical_json}`. `uebernehmen` prüft Belegung/Ausweis gegen den bewusst bestätigten Empfangsschlüssel, bindet das lokale Gerät und richtet danach die Challenge-Anfragen ein. S8 muss zusätzlich Zweck, Gültigkeit und P2P-Schlüsselbindung des Ausweises prüfen; `verify_signed` allein vergibt keine Rechte.

### OIDC / authentik

Öffentlicher Client: Authorization Code mit PKCE **S256**, System-Browser nach Klick, Rückweg `http://127.0.0.1:<zufälliger Port>/oidc/callback`. Zufällige 32-Byte-Werte für Verifier (43 Base64url-Zeichen), `state`, `nonce`; gültige Verifierlänge 43–128 (RFC 7636). Discovery liefert Autorisierungs- und Token-Endpunkt, `issuer` muss exakt zur konfigurierten Anbieteradresse passen; HTTPS darf nie auf HTTP herabgestuft werden. Kein Proxy, keine Redirect-Verfolgung, 1 MiB Discovery-/Tokenantwort, Zeitlimits. Loopback nimmt genau einen HTTP-Rückruf an, prüft Host, Pfad und `state`, antwortet ohne Skript/Remoteinhalte: „Du kannst zu cocopi zurückkehren.“ Falscher State beendet den Versuch. Nonce wird mit dem `id_token` an den Empfang weitergereicht. Dieser prüft Signatur/JWKS, iss/aud/exp/iat/nonce und groups, nicht die Desktop-UI.

Rust-API `hub_v1::oidc::{Discovery,Login,pkce_challenge}`: Browseröffnen ausschließlich per `open_browser`-Callback. **Integrationsgrenze:** SHA-256 ist als Callback einer geprüften Bibliothek injiziert, da keine zusätzliche Abhängigkeit ohne Freigabe eingebaut werden darf; kein eigener Kryptocode. Der RFC-Test prüft Challengekodierung gegen vorgegebene Hashbytes, nicht eine im Backend implementierte SHA-256-Funktion. Produktive App muss einen echten SHA-256-Callback liefern.

### Signierte Nutzdaten und Hub-Belegung

Übertragungsformat des Empfangs: `{inhalt:<Nutzdaten>,signatur:"<Base64url>"}`. Signiert wird nur `inhalt`, rekursiv sortierte Objektschlüssel (UTF-16-Reihenfolge wie JavaScript), keine Leerzeichen, UTF-8, keine abschließende neue Zeile. Zahlen müssen sichere Ganzzahlen im Bereich ±(2^53−1) sein; keine Fließkommazahlen. Arrays behalten ihre Reihenfolge. Keine unsignierten Hüllfelder. Additiv akzeptieren S3-Client und Hub auch das frühe flache Format `{...Nutzdaten,signatur}`; darin wird alles außer `signatur` signiert. Erzeuger verwenden die Hülle. Zeiten sind Unix-Millisekunden, **Ausnahme** Ausweis `iat`/`exp`: Unix-Sekunden.

- Belegung: `{v:1,typ:"belegung",slot,checkin_id,person,rolle,geraete:[{geraet_id,pubkey,name,art}],gueltig_bis}`.
- Freigabe: `{v:1,typ:"freigabe",slot,checkin_id}`.
- Geräteausweis: `{v:1,typ:"geraeteausweis",pid,person,rolle,net_pubkey,geraet_pubkey,iat,exp}`; höchstens 12 h, Rollen `seminarleitung|teilnehmende|gast` nur aus geprüftem Ausweis.

| Methode | Pfad | Zugriff | Body / Antwort |
|---|---|---|---|
| POST | `/v1/belegung` | nur Empfang, Ed25519 + echter Socket 127.0.0.1 | `{protokoll:1,belegung:<signiert>}` → 200 `belegung_ok{slot,checkin_id}` |
| POST | `/v1/freigabe` | wie oben | `{protokoll:1,freigabe:<signiert>}` → 200 `freigegeben{slot,checkin_id}`, **erst nach Prozessende** |

Hub konfiguriert `HUB_SLOT` und den **außerhalb des Netzwerks bestätigten** Pin `HUB_EMPFANG_PUBKEY` (raw Ed25519/Base64url). Keine automatische Pinübernahme aus eingehendem JSON. Fehlt der Pin, liefern interne Endpunkte 501. Falsche Signatur, fremder Slot, abgelaufene/aktive andere Belegung → 403 `belegung_ungueltig`. Genau die signierten Geräte werden als Quelle `belegung` registriert; alte Geräte/Codes/Anfragen werden ersetzt. Die `checkin_id` wird persistent nur gehasht als Replaymarker gespeichert und bleibt nach Freigabe gesperrt (409 `verbraucht`). Keine personenbezogenen Inhalte im Replaymarker. Slot-Leeren durch S4 löscht auch dieses Register; daher gilt der Replay-Schutz innerhalb der Lebensdauer des Hub-State, nicht über ein vollständiges Home-Leeren hinweg. Empfang darf alte Belegungen nach Leeren nicht erneut senden; ein stärkerer installationsübergreifender Replayvertrag benötigt einen außerhalb des flüchtigen Homes liegenden Marker (Änderungsbitte an K/S4).

Freigabe widerruft **alle** Geräte einschließlich gekoppelter Handys, beendet SSE, sperrt neue Anfragen, sendet SIGTERM an alle vom v1-Hub gehaltenen pi-Instanzen und nach **10 s** SIGKILL. Erfolg erst nach bestätigtem Exit aller Kinder; fehlt nach weiteren 5 s der Nachweis, Fehler statt 200, Belegung bleibt gesperrt und erneuter Freigabeversuch ist möglich. Sessionregister/Replay-Inhalte werden erst nach Prozessende geleert. Das ersetzt **nicht** S4s UID-weites Beenden und nachgewiesenes Dateileeren; diese Schutzschicht beendet auch unabhängige Toolprozesse und entfernt alle Daten.

**Proxy-Pflicht für S4:** `/v1/belegung` und `/v1/freigabe` dürfen niemals weitergeleitet werden, auch nicht unter `/slot/accountNN/` oder durch einen allgemeinen `/v1/*`-Fallback. Rückgabe z. B. 404/403. Allein die Loopback-Prüfung reicht hinter einem lokalen Proxy nicht, weil dessen Socket ebenfalls 127.0.0.1 ist. Keine Auswertung von `X-Forwarded-For` als Autorisierung. Hub zusätzlich weiterhin ausschließlich an 127.0.0.1 binden.

### Mitsteuern (A9, nur Kennzeichnung)

`senden{text,von?:{person,rolle}}` ist additiv. Hub prüft Format, erzeugt zur Anzeige ein `session_ereignis{session_id,von,pi_event:{type:"cocopi_absender",text}}` und reicht den unveränderten Prompt an pi durch. **Keine** Rollen-/Zugriffsrechte aus `von` ableiten; es ist unbeglaubigte Darstellungsinformation. Zustimmung und gültiger Leitungs-Ausweis werden vom teilnehmenden Client vor dem Weiterreichen geprüft (S8). Ein Anzeigename im Hub-Chat ist kein Identitätsnachweis.

## I2 · Sync-Endpunkte (A7, Besitzer S7)

Normative Pfad-/Archiv-/Konfliktregeln: [shared/sync/SYNC.md](../../sync/SYNC.md). Alle Routen sind geräteauthentisiert (S7: Desktop), **außer** den unten beschriebenen signierten Dienst-Routen. `handleSync(req,res,ctx)` wird **vor** dem allgemeinen JSON-Leser aufgerufen, damit Uploads gestreamt bleiben. Kontext enthält `devices`, authentisiertes `device`, `slot`, `url`, `roots`, `inRoots`, `read`, `send`, `msg`, `failure`, und den lazy erzeugten `sync`-Dienst (`createSync`). Fehlt `lib/sync-v1.mjs` oder private Zustandablage, 501 `nicht_verfuegbar`. Importfehler eines vorhandenen Moduls werden nicht als fehlende Funktion verschluckt.

| Methode | Pfad | Body / Antwort |
|---|---|---|
| POST | `/v1/sync/hoch` | tar.gz-Stream; `X-Cocopi-Bereich: Projekte|Ergebnisse|Skills|Verlaeufe`, `X-Cocopi-Name`; → 201 `sync_hoch_ok{bereich,name,dateien,bytes,manifest_sha256}` |
| POST | `/v1/sync/vorschau` | `{protokoll:1}` → `sync_vorschau{persoenlich:[{pfad,bytes,status}],fluechtig:{bytes,dateien,beispiele},gross:[…]}` |
| POST | `/v1/sync/archiv` | `{protokoll:1,auswahl:{zusaetzlich:[pfad…],ausschliessen:[pfad…]}}` → 201 `archiv_bereit{archiv_id,sha256,bytes}` (noch kein Download) |
| GET | `/v1/sync/archiv/:archiv_id` | tar.gz-Stream, Header `X-Cocopi-Sha256`; Client prüft Hash und entpackt ohne Überschreiben, erst danach Empfang `erhalten` |

Wurzeln: `<HUB_ROOTS[0]>/Projekte`, `<HUB_ROOTS[0]>/Ergebnisse`, `~/.pi/agent/skills`, `~/.pi/agent/sessions`; in Tests ausschließlich Temp-HOME. Maximal 2 GiB je Check-in, einzelne Dateien >200 MB nur ausdrücklich, niemals Symlinks/Devices/FIFOs/Pfadausbrüche. Keine automatische Ausführung. Proxy muss Upload-/Timeoutgrenzen entsprechend setzen.

### Empfang → Sync: eng begrenztes Dienstarchiv (S7-Erweiterung)

`POST /v1/sync/dienst/archiv` und `POST /v1/sync/dienst/archiv/:archiv_id` benötigen **keinen Geräte-Bearer**, aber echten Loopback-Socket **und** Empfangssignatur. Der Hub delegiert vor Geräteauth an S7 mit `{devices,slot,url,sync,empfangPubkey,now}`. Body `{protokoll:1,auftrag:<flach signiertes JSON>}`, Inhalt `{v:1,typ:"sync_dienst",aktion:"archiv"|"holen",slot,checkin_id,iat,nonce,archiv_id?}`. `iat` Sekunden ±60 s, `nonce` zufällige 32 Byte Base64url, persistent einmalig. Aktiver Slot und Check-in müssen passen; `holen` bindet exakt die Archiv-ID. Antworten: erst 201 `archiv_bereit`, dann gzip-Stream mit SHA256-Header. Keine Modell-/Shell-/beliebigen Dateirechte. Diese Sonderroute verwendet S7s flachen Signaturvertrag, **nicht** die `{inhalt,signatur}`-Hülle der Belegung. Auch `/v1/sync/dienst/*` darf der Proxy niemals weiterleiten. Wiederholung eines konsumierten Auftrags ist kein Erfolg; Empfang muss für einen neuen Versuch frisch signieren.

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
| pool_voll | 202 | Alle Arbeitsplätze sind belegt. | Du wartest in der Warteschlange; dein Platz wird angezeigt. **Kein Fehlerpaket**, Typ `warteschlange`. |
| keine_berechtigung | 403 | Du darfst diesen Bereich nicht öffnen. | Frage deine Seminarleitung. |
| ausweis_ungueltig | 401 | Dein Geräteausweis ist ungültig. | Melde dich erneut am Empfang an. |
| belegung_ungueltig | 403 | Diese Belegung konnte nicht bestätigt werden. | Bitte die Seminarleitung, den Empfang zu prüfen. |
| nicht_verfuegbar | 501 | Dieser Bereich ist noch nicht verfügbar. | Versuche es später erneut. |
| archiv_fehlt | 404 | Dieses Archiv wurde nicht gefunden. | Prüfe den Check-out am Rechner. |
| intern | 500 | R2 konnte die Anfrage nicht abschließen. | Versuche es später erneut oder frage die Seminarleitung. |

Keine Telemetrie. TLS-Vorschaltung, Hostpinning, globale CPU-Queue, sichere Transfers und echte Kontenprovisionierung sind nicht Teil dieses I1-Codes.
