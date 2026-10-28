# R2 · flüchtiger cocopi-Pool (I2)

**Nicht installiert. Kein Deployment ausführen.** Arbeitsstand nur lokal mit
Fake-Root/Fixtures getestet; bestehende R2-Konten, Dienste, Mounts und Netzregeln
wurden nicht geändert. Frühere I1-Dateien `BEFUND.md`, `DRYRUN.txt`, `TESTS.txt`
sind historische Befunde, keine Pool-Abnahme.

## Ziel und Dateien

Zehn gleichberechtigte Plätze `account01`–`account10`, UID/GID 20001–20010.
Keine persönliche Dauerzuordnung, auch account10 keine Reserve. Empfang bindet
nur `127.0.0.1:8800`; Hubs nur 8801–8810. Erst zwei synthetische Testplätze,
Zehnerfreigabe separat. Ein Platz umfasst **`/home/accountNN` und
`/mnt/data/accountNN`**. Der alte I1-Pfad `/mnt/data/cocopi/accountNN` wird nicht
übernommen. Sämtliche persönlichen Sitzungen, Geräte-/Hubzustände und Caches
müssen innerhalb der beiden flüchtigen Wurzeln liegen. `HUB_ROOTS` erlaubt nur
`/home/accountNN/arbeit` für Dateirouten, ist aber keine Sandbox für Shelltools.

| Datei | Aufgabe |
|---|---|
| `systemd/cocopi-hub@.service` | System-Hub, `User=%i`, eigene Slice, `/etc/cocopi/hub-%i.env` |
| `systemd/cocopi-empfang.service` | Systemkonto, privater State `/var/lib/cocopi-empfang` |
| `systemd/cocopi.slice`, `cocopi-account.slice.in` | globale und je-Platz-CPU/RAM/Taskgrenzen |
| `cocopi-slotctl` | fd-basierter Root-Löschhelfer mit Leer-Nachweis |
| `sudoers/cocopi-slotctl`, `slotctl-empfang` | exakt enumerierte Rootrechte nur für Empfang |
| `einrichten.sh` | acht nummerierte Schritte, Standard Dry-Run |
| `provision.sh`, `check.sh` | Poolkonten/Teilquoten/System-Units, technische Leseabnahme |
| `admin-check.sh`, `pool-read.py`, `proxy-check.py` | belegte/freie Plätze, Leerbelege, Quoten, Empfang und HTTPS-Sperren |
| `proxy/` | Caddy-TLS oder Tailscale-TLS vor lokalem Caddy |
| `AUTHENTIK.md` | Public Client, PKCE S256, Loopback beliebiger Port, Rollenclaim |
| `EINRICHTEN-MIT-PI.md` | kopierbarer interaktiver Prompt; ja vor jedem sudo |
| `DATENSCHUTZ.md` | einfache Information und Kontaktplatzhalter |

Die alte Root-Level-Unit `cocopi-hub@.service` ist absichtlich deaktiviert, nicht
mehr installieren. Keine neuen User-Hubs und kein Linger. I1-`teardown.sh` ist
für echte Ausführung gesperrt: dessen dauerhafte Nutzdatenarchive widersprechen
E9. Normaler Check-out entfernt Daten, nicht die feste Unix-Identität.

## Sichere lokale Vorschau

Als normaler Nutzer, ohne sudo und ohne Netzwerk:

```sh
python3 deploy/r2/test_scripts.py
bash deploy/r2/einrichten.sh --dry-run --slots account01,account02
bash deploy/r2/admin-check.sh --dry-run --slots account01,account02
```

Standard ohne Flag ist ebenfalls Dry-Run. Neuer deutscher Ausführungsschalter
`--ausfuehren`; `--apply` bleibt nur als kompatibler Alias der alten Skripte.
Echte Einrichtung erfordert zusätzlich `COCOPI_D_R1=freigegeben`,
`COCOPI_POOL_ABNAHME=bestaetigt`, einen unabhängig bestätigten öffentlichen
Empfangsschlüssel und einen bestätigten Hubvertrag `cocopi-pool-v1`. **Diese
Flags erst nach tatsächlichen Nachweisen setzen**, nicht um Prüfungen zu umgehen.
Skripte als Root nur aus unveränderlicher root:root-Kopie
`/usr/local/lib/cocopi-r2` ausführen, nie aus dem beschreibbaren Worktree.

Keine automatische Paketinstallation, Quotaaktivierung, Mount-/Firewalländerung,
kein Proxy-Start und keine Migration alter aktiver Userdienste. Werkzeuge,
rootkontrollierter ext4-Mount `/mnt/data`, aktive Userquoten auf `/` und
`/mnt/data`, gemeinsame Node-/pi-/Buildwerkzeuge sind Voraussetzungen.
`slots.conf` bleibt `HUB_CONTRACT=unconfirmed`, bis K die zusammenpassenden
S3/S7-Commits bestätigt. Öffentliche Umgebungswerte gehen über explizit
freigegebenes `sudo env`, geheime Werte nur in private Dateien/sudoedit.

## Löschhelfer-Vertrag

```text
cocopi-slotctl leeren account01                 # nur Plan, keine Änderung
cocopi-slotctl --ausfuehren leeren account01    # tatsächlich löschen
cocopi-slotctl status [account01]               # aktueller, lesender Zustand
cocopi-slotctl pruefen account01                # Live-Prüfung, ohne Schreiben
cocopi-slotctl --ausfuehren pruefen account01   # Prüfung + Erstbeleg vor Hubstart
```

Nur Root; Tests ausschließlich unprivilegiert mit temporärem `COCOPI_ROOT`.
Unter sudo wird `COCOPI_ROOT` abgewiesen. Keine freien Pfade/UIDs, Regex exakt
`^account(0[1-9]|10)$`, keine Shell. Verwaltungsverzeichnisse rootverwaltet;
Wurzeln/Eltern keine Links, innere Links werden nur selbst entfernt. Keine
Mounts im Löschbaum, auch keine gleich-Dateisystem-Bindmounts. Serielle Sperre.

Reihenfolge: bestätigte Empfangsfreigabe/Widerruf → Hub stoppen → Wurzeln
rootprivat sperren → Usermanager stoppen → UID-Prozesse TERM/KILL → bei Resten
abbrechen → fd-relativ ohne Link-Folgen löschen → geprüftes Skelett einspielen →
Hashes/Tree und aktive Teilquoten prüfen → Leerbeleg atomar schreiben →
Besitz zurückgeben → Hub starten. Kein persistentes Nutzdatenbackup. Fehler
liefern Exit ungleich 0 und keine Freigabe. Teilweise gelöschter Zustand ist
möglich und bleibt gesperrt; nicht als Transaktion/Undelete verstehen.

JSON-Erfolg: `slot`, `zustand:"leer"`, `nachweis:"vor_hubstart"`, `zeit`,
`dauer_ms`, `bytes_geloescht`, `quota_bytes`, `grenze_bytes`. Beleg unter
`/var/lib/cocopi-slots/accountNN.leer.json` root:root 0600, nur Zeit/Zähler,
keine Personen oder Dateinamen. `pruefen --ausfuehren` schreibt beim ersten
Start denselben Beleg, löscht aber nichts. Nach Hubstart gilt der Livebaum
wegen neuer Hubdateien/Prozesse nicht mehr als leer. Empfang muss den
**vorherigen** erfolgreichen Nachweis benutzen, niemals einen Fehler wegfiltern.

Quote: 9.765.625 KiB (10 GB) NVMe + 68.359.375 KiB (70 GB) Daten = 80 GB dezimal;
Soft=Hard, Inodes 200000/800000. Leer-Nachweis verlangt ≤1 MiB Gesamtbelegung
für Skelett/Verzeichnisblöcke und **exakt passende UID-Inodezahlen** innerhalb
der Wurzeln plus dem bekannten Legacy-Token. So blockieren auch kleine
Restdateien, offene gelöschte Dateien oder Hardlinks außerhalb der Wurzeln die
Freigabe. Der alte rootverwaltete Token bleibt bis zur S3-Tokenbereinigung
bestehen; der Geräteausweis/Check-in muss trotzdem Hubzugriffe begrenzen.

Skelett `/etc/cocopi/skel`: nur leere Verzeichnisse `arbeit/Projekte`,
`arbeit/Ergebnisse`, `.pi/agent/{skills,sessions}`. Keine Links, Spezialdateien,
Provider-/Authentifizierungsdaten, unkontrollierte Shellprofile oder Autoextensions.
PrivateTmp entsorgt `/tmp` und `/var/tmp` mit dem Hub; Prozesse außerhalb seiner
Cgroup oder UID-Reste werden nicht als „leer“ behauptet. Vor Pilot Hostpfade,
IPC, Spool, mögliche externe Schreiborte und Quoten-Inodeverhalten wirklich
testen. Unixkonto/Quoten sind keine vollständige Container-/Egress-Isolation.

## Proxy-Varianten (separate Netzfreigabe)

A: `proxy/Caddyfile` als öffentliche TLS-Site, geprüfte `routes.caddy` nach
`/etc/cocopi/proxy/`. B: `Caddyfile.tailscale` bindet ausschließlich
`127.0.0.1:8799`; `tailscale serve --bg --https=443 http://127.0.0.1:8799` setzt
TLS davor. Das Hilfsskript zeigt standardmäßig nur diesen Befehl; keine direkte
Weiterleitung auf Empfang/Slots, kein Funnel. Vor Änderungen existierende Serve-
Konfiguration prüfen. Kein Überschreiben ohne eigene Freigabe.

Nur `/v1/empfang/*` und `/slot/accountNN/v1/*` werden weitergegeben (SSE mit
ungepuffertem Flush). `/v1/belegung`, `/v1/freigabe` direkt UND unter allen
Slotpräfixen liefern 403. Encodierte/normalisierte Angriffspfade werden gesperrt;
sonst 404. Keine automatische PWA-/Dateistatik und keine Access-Logs. Auch
`/v1/sync/dienst/*` ist intern und muss gesperrt bleiben. Signierte
Dienstaufträge im Hub bleiben zwingend: der Proxy selbst verbindet über Loopback!

Vor Installation mit verfügbarer Software separat prüfen:

```sh
caddy validate --config /etc/cocopi/proxy/Caddyfile --adapter caddyfile
visudo -cf /usr/local/lib/cocopi-r2/sudoers/cocopi-slotctl
```

Hier sind Caddy und visudo nicht vorhanden; Syntax/Proxybetrieb damit **nicht
validiert**. Python-Fixtures sind kein Ersatz. Erst echte HTTPS-403 plus positive
Empfang-/Slot-/SSE-Abnahme nach Netzfreigabe erlauben Pilotbetrieb.

## Admin-Abnahme und offene Integration

`admin-check.sh --ausfuehren --slots account01,account02 --proxy-url https://NAME`
liest nur Konto/UID, Systemdienst/PID, lokale Listener, Quote und Poolprojektion;
ruft `POST /v1/empfang/hallo` inhaltsfrei auf und testet absichtlich ungültige
POSTs gegen externe Sperrpfade. Kein Modell, keine Namen/Sessiondateien/Token.
Kein URL-Credential, Pfad oder Query im Parameter erlaubt, TLS niemals abschalten.
Ein aktiver Dienst zeigt keine Anwesenheit; „Dienststart“ ist nicht letzte Arbeit.

**Derzeit noch Integrationsblocker (an S7/K gemeldet):**

1. S7 muss `leeren`-JSON (Exit 0 und Beleg) prüfen und darf nach Hubstart kein
   zweites `pruefen` verlangen. Rootzugriff über `slotctl-empfang`/enge sudoers,
   nicht direkt vom Systemkonto auf Roothelfer. Vorläufig verweigert Live-Prüfen
   nach Start korrekt; keine Freigabe.
2. Aktuelle inhaltsfreie Projektion `/var/lib/cocopi-empfang/pool-status.json`:
   `{protokoll:1,zeit:<ISO8601 UTC>,slots:[{slot:"account01",zustand:"frei"},…]}`,
   genau zehn Einträge, Zustände `frei|belegt|leeren|gesperrt`, Alter ≤120 s,
   atomar geschrieben, keine zusätzlichen personenbezogenen Felder. S7 muss sie
   produzieren; Admin zeigt sonst unbekannt/Exit 1, liest niemals private Pooldaten.
   Zwei-Slot-Test ist daher noch keine grüne Zehnerpoolabnahme.
3. S3/S7 signierten Archivweg, Ausweise, Erstinitialisierung und Replayverhalten
   gemeinsam nachweisen. OIDC-Scopes `groups` versus `openid profile` abstimmen.
4. Live-Quoten, sudoers, Caddy, systemd und echter Check-out inkl. `/tmp` und
   externen UID-Inodes sind nicht getestet. 72-h-Löschung/7-Tage-Logs gehören
   zum Empfang und müssen auch ohne Aktivität/bei Wiederanlauf durchgesetzt sein.

Globale Slice 800 % / 24G high / 26G max, je Slot 100 % / 2G high / 2560M max /
256 Tasks. Das sind konservative Vorlagen, keine gemessene Lastabnahme; ein
schwerer Job global bleibt Hubvertrag. Empfang-Unit setzt absichtlich weder
NoNewPrivileges noch ProtectHome=read-only, da der enge sudo-Helfer sonst
unwirksam wäre; dies verlangt besonders sorgfältiges Review des Empfangscodes.

## Nachweise lokal

`python3 deploy/r2/test_scripts.py`: 38 grün (inkl. zehn Fake-Root-Angriffstests).
Nur Tempverzeichnisse, keine System-/Netzänderungen. Bash-Syntax, Dry-Run-
Mutationssentinel, Argumente, Quoten/Slices/Listenerfixtures, Poolstatus/Proxy-
Mocks und enumerierte sudoers geprüft. Kein R2-Deployment, authentik-Login,
Modellaufruf oder produktiver End-to-End-/Securitynachweis.
