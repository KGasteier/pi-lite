# R2: persönliche cocopi-Slots vorbereiten

**Stand I1: Skripte und lokaler Dry-Run, keine installierte Slotumgebung.**
Keine Konten angelegt, keine Dienste gestartet, keine Quoten aktiviert. Die
reale Maschine und die bewusst offenen Voraussetzungen stehen in [BEFUND.md](BEFUND.md).
Der alte Hub von `medienhaus` bleibt unangetastet. Diese Anleitung ist für die
Administration; Studierende benötigen später nur ihre bestätigte Anmeldung.

## 1. Was geliefert ist

- `slots.conf`: zehn Slots, account10 Reserve; Standardauswahl nur **account01,account02**.
- `provision.sh`: erst Voraussetzungen prüfen, dann persönliche Konten, private
  Datenpfade, Account-Slice, Linger und getrennte Hub-Userdienste einrichten.
- `check.sh`: pro Punkt `OK` / `FEHLER`; `--apply` bedeutet hier ausschließlich
  lesende Abnahme und unprivilegierte Negativversuche (kein Lasttest).
- `admin-check.sh`: ruhige Lehrenden-Tabelle (Entwurf), rein lesend: Konto,
  Verbindungsdienst, lokaler Port, Speicherbelegung und belegter Dienststart.
  Kein Zugriff auf Gespräche, Dateien oder Token; Arbeitsaktivität noch unbekannt.
- `teardown.sh`: ausdrücklich ausgewählte Konten sperren, Dienste geordnet
  anhalten, Home/Daten/Konfiguration archivieren. Nur `--purge` löscht Nutzdaten.
- `common.sh`, `quota-read.py`, `slice-read.py`: gemeinsame sichere Hilfen,
  Quota-Abfrage über Linux **Q_GETQUOTA**, Vergleich effektiver systemd-Grenzen.
- `cocopi-hub@.service`, `hub-launch.sh`: rootverwaltete Dienstvorlage und
  Startwrapper; keine eingebetteten Zugangsdaten.
- `test_scripts.py`: lokale Regression mit temporärem HOME und gesperrten
  Änderungsbefehlen. Keine echten Modellaufrufe, keine echten Configdaten.
- `DRYRUN.txt`, `TESTS.txt`: reproduzierbarer Nachweis, **keine Pilotabnahme**.

Alle Shellskripte verwenden Bash und `set -euo pipefail`. Standard ist stets
`--dry-run`, auch bei Root. Nur `--apply` **und** Root erlauben Bereitstellung /
Rückbau. `check.sh --apply` verlangt Root für runuser, sudo-Listing, Quoten und
Listener-PID-Zuordnung. Kein Skript installiert Pakete, ändert Mounts/Netzwerk,
verwendet Docker oder ruft ein Modell auf.

## 2. Jetzt gefahrlos prüfen (normaler Nutzer)

Im Repository-Root:

```bash
bash deploy/r2/provision.sh --slots account01,account02
bash deploy/r2/check.sh --slots account01,account02
bash deploy/r2/admin-check.sh --slots account01,account02
bash deploy/r2/teardown.sh --slots account01,account02
PYTHONDONTWRITEBYTECODE=1 python3 deploy/r2/test_scripts.py
for f in deploy/r2/*.sh deploy/r2/slots.conf; do bash -n "$f" || exit 1; done
```

Dry-Run beschreibt Schritte; er erfindet weder UIDs noch Erfolgsnachweise.
Eine leere, doppelte oder ungültige Slotliste wird abgewiesen. Rückbau hat
absichtlich **keine** Standardauswahl. `--purge` ist nur dort zulässig.

## 3. Vor jeder späteren echten Einrichtung

**Diese Schritte benötigen separate menschliche Freigabe. Nicht im I1 ausführen.**

1. A4 aus `coord/FREEZE-I1.md` ist verbindlich. S3 implementiert den Hubvertrag;
   K muss ihn nach S3-Review freigeben. Identitäts-/Gerätebindung,
   Sessionlimits und Widerruf mit Mock nachweisen. Die alten Hub-Variablen allein
   beweisen diese Funktionen nicht.
2. Externes, verschlüsseltes Backupziel und Zuständigkeit festlegen; Restore
   auf getrennter Testmaschine proben. Root-/Filesystem-Sicherung vor Änderungen.
3. Für beide vorhandenen **ext4**-Dateisysteme Userquoten administrativ vorbereiten:
   Quota-Tools, geprüfte Mount-/ext4-Konfiguration mit `usrquota`, aktive
   Durchsetzung. Root-Dateisystem nicht während des Seminarbetriebs blind remounten
   oder `quotacheck` ausführen. Skript macht ausdrücklich **nichts davon**.
   Interne Quota-Features ohne belegte Mountoption werden konservativ abgewiesen.
4. `/mnt/data` ist derzeit medienhaus-eigen. Mountwurzel und Eltern der
   rootverwalteten Slotpfade separat root-kontrolliert bereitstellen; keine
   bestehenden Daten rekursiv umchownen. Home-Basis muss ebenso rootverwaltet sein.
5. Gemeinsame, rootverwaltete Werkzeuge bereitstellen: Node ≥20, pi, Rust/Cargo,
   Python, Git, ffmpeg, C-Compiler und make. Konfiguration erwartet
   `/opt/cocopi/bin` + Systemwerkzeuge. **Keine** Symlinks in das Home von
   medienhaus, keine Übernahme privater mise-/Provider-/pi-Konfigurationen.
   Slotkonten dürfen eigene Bibliotheken/Buildwerkzeuge ohne sudo nutzen.
6. Geprüfte, unveränderliche Hubkopie einschließlich benötigter Begleitdateien
   nach `/opt/cocopi/pi-hub` bereitstellen (nicht den Arbeitsbaum eines Nutzers).
   K muss den Vertrag `cocopi-slots-v1` nach S3-Review freigeben. Erst danach in
   `slots.conf` `HUB_CONTRACT=cocopi-slots-v1` setzen und den gleichnamigen Text in
   `slots-contract` der geprüften Hubauslieferung bereitstellen. **Der Marker ist
   eine administrative Freigabe, kein automatischer Sicherheitsbeweis.**
7. Skripte vollständig prüfen und als root-eigene, nicht gruppen-/weltschreibbare
   Kopie unter einem rootverwalteten Pfad bereitstellen (z. B.
   `/usr/local/lib/cocopi-r2/`). **Nicht als Root aus dem nutzerbeschreibbaren
   Worktree starten.** `common.sh` prüft Konfiguration/Eltern zusätzlich; diese
   Prüfung ersetzt nicht den Schutz der bereits geladenen Skripte.
8. Anfangs nur account01,account02; keine automatischen SSH-Schlüssel, Passwörter, Provider-
   oder Gerätezutritte. Diese werden in einem separaten begleiteten Enrollment
   vergeben. Konten sind neu mit gesperrtem Passwort angelegt.

Spätere, ausdrücklich freizugebende Root-Befehle (hier NICHT ausgeführt):

```bash
bash /usr/local/lib/cocopi-r2/provision.sh --dry-run --slots account01,account02
bash /usr/local/lib/cocopi-r2/provision.sh --apply --slots account01,account02
bash /usr/local/lib/cocopi-r2/check.sh --apply --slots account01,account02
```

Jeder Fehler blockiert Freigabe. Prüfung erneut ausführen, nicht blind weitermachen.
Vorhandene Konten müssen Kommentar, UID-Inventar, Home und private Primärgruppe
passen; weitere Gruppen werden abgewiesen, nicht automatisch entfernt.
Nur die Slotgruppe `cocopi-slots` kommt hinzu; diese erteilt **keinen** gemeinsamen
Datenzugriff. Keine sudo-/docker-/lxd-/disk-/adm-Gruppen. Zusätzlich sudoers-
Verbot und effektives Listing; jede erlaubende Regel ist ein Fehler.

## 4. Limits: was sie tatsächlich bedeuten

| Grenze | Vorschlag je Slot | Durchsetzung / Grenze des Nachweises |
|---|---|---|
| CPU | CPUQuota 100 % (ein CPU-Kern als Zeitbudget) | rootverwaltete `user-UID.slice`, umfasst auch SSH-/Usermanager-Prozesse; keine garantierte CPU-Reservierung |
| RAM weich/hart | MemoryHigh 2 GiB / MemoryMax 2,5 GiB | ganze Account-Slice, TasksMax 256; Last-/OOM-Test separat in Testumgebung |
| Platte NVMe | 10.000.000.000 Byte | Userquote auf `/`, Soft=Hard 9.765.625 KiB; 200.000 Inodes |
| Platte SATA | 70.000.000.000 Byte | Userquote auf `/mnt/data`, Soft=Hard 68.359.375 KiB; 800.000 Inodes |
| Platte gesamt | höchstens 80 GB **dezimal** | feste Teilbudgets, nicht 80 GB pro SSD und nicht dynamisch summiert |
| Sessions | höchstens 2 offen, 1 aktiv je Slot | **S3-Hub-Vertrag nötig**, nicht durch systemd bewirkt |
| Schwerer CPU-Job | 1 global über alle Hubinstanzen | **globale S3-Queue nötig**, getrennte Env-Werte erzeugen keinen gemeinsamen Semaphore |

Ext4-Userquoten zählen die UID auf dem gesamten jeweiligen Dateisystem, auch
Dateien außerhalb Home. Homes liegen auf NVMe unter `/home/accountNN`, Daten auf
SATA unter `/mnt/data/cocopi/accountNN`, beide **0700** mit privater Primärgruppe.
Unix-Konto und Slot-ID sind gleich; alte Slotbezeichnungen werden abgewiesen.
Dies ist keine automatische Migration bestehender Konten oder Daten.
Nur zustandssetzende Abweichungen werden korrigiert; vorhandene Token bleiben
unverändert, laufende Hubs werden **nicht automatisch neu gestartet**. Änderungen
an Unit/Umgebung benötigen nach Zustimmung einen getrennten Wartungsneustart;
`daemon-reload` allein aktiviert keine neue Prozessumgebung.

Tmpfs/weitere Mounts und privilegierte Dienste anderer UID sind keine Diskquote.
Vor Pilot alle Schreibpfade und `/run/user`/`/dev/shm` prüfen. Eine User-Slice ist
keine vollständige Sandbox: voller Tool-Use bleibt erlaubt, Netzwerk/Egress und
fremde öffentlich lesbare Systemdateien sind nicht gesperrt. Ein globales
Schwerlastlimit kann direkte Shelljobs ohne zusätzliche Ausführungsregeln nicht
technisch erzwingen; Pilotpolicy und Hubqueue haben diese ehrliche Grenze.

## 5. Hub und Geheimnisse

Pro Slot `cocopi-hub@accountNN.service` unter dem eigenen systemd-Usermanager;
rootverwaltete gemeinsame Unit in `/etc/systemd/user`, Drop-in in
`/etc/systemd/system/user-UID.slice.d/50-cocopi.conf`. Linger nur bei Bedarf setzen.
Hubbindung **127.0.0.1**, account01 Port 8801, …, account10 Port 8810. Kein öffentlicher Port,
kein TLS-/Tailscale-/SSH-Setup durch diese Skripte. Ein späterer geprüfter
Zugang muss auf die richtige Instanz und Geräteberechtigung abbilden.

Der Startwrapper prüft Unixkonto/Slot/Loopback und lädt ein kryptografisch
zufälliges, 0600 geschütztes Bootstrap-Token aus
`/etc/cocopi/slots/accountNN/token` (Slotkonto-eigen, Eltern root-eigen).
Token nie in Argumenten, URLs, Dry-Run, Git oder Journal. xtrace wird abgeschaltet;
stdout/stderr des Hubs gehen im konservativen Pilot nicht ins Journal. Daher sind
Status-/Exitcodes sichtbar, Inhaltslogs aber nicht verfügbar. Der Token wird nicht
rotiert, wenn er bereits existiert. Es ist **kein Ersatz für Geräte-Token**.
Root und das Slotkonto können den eigenen Token lesen; nicht vor seinem eigenen
Agenten geschützt. Keine fremden Home-Konfigurationen werden gelesen/kopiert.
`HUB_STATE=/mnt/data/cocopi/accountNN/hub-state` ist im Vertrag
`cocopi-slots-v1` ein **Verzeichnis**, nicht eine JSON-Datei und nicht `off`.
Es wird als Slotkonto (nicht als Root innerhalb des beschreibbaren Datenbereichs)
mit 0700 angelegt; Besitzer/Gruppe werden danach geprüft. Es zählt zur SATA-Quote
und enthält das Geräte-Register des Slots. Vorhandene Inhalte bleiben unberührt.
Der Startwrapper prüft Pfad und Rechte vor dem Tokenlesen.

Die Hub-Umgebung verwendet exakt `HUB_SLOT=accountNN`,
`HUB_MAX_OPEN_SESSIONS=2`, `HUB_MAX_ACTIVE_SESSIONS=1`,
`HUB_GLOBAL_HEAVY_JOBS=1` und `HUB_STATE` wie oben. Der bisherige Hub deutet
`HUB_STATE` als Datei; er ist damit **nicht kompatibel**. S3 passt das parallel an.
`HUB_CONTRACT=unconfirmed` bleibt bis zur Freigabe durch K nach S3-Review bestehen.
Ein Verzeichnis beweist weder sichere Persistenz noch verbotenen Auto-Resume;
beides ist im Hubreview und in Mocktests gesondert abzunehmen.

## 6. Abnahme und Negativtests

### Kurze Lehrenden-Sicht (Aylin; Entwurf)

```bash
bash deploy/r2/admin-check.sh --slots account01,account02  # nur Vorschau
# Erst später, separat freigegeben, als Root aus der geprüften Skriptkopie:
bash /usr/local/lib/cocopi-r2/admin-check.sh --apply --slots account01,account02
```

`--apply` liest hier **nur**; es startet/stoppt keinen Dienst, schreibt keine Datei
und fragt keinen Hub über das Netzwerk ab. Keine zusätzlichen Lehrendenrechte
auf private Konten: Die Administration führt den Bericht aus. Die Tabelle zeigt:

- **Eingerichtet:** Konto vorhanden und eindeutig verwaltet, fehlt oder bitte prüfen.
- **Verbindung:** persönlicher Hubdienst aktiv oder nicht aktiv. Aktiv heißt nicht
  „Studentin arbeitet“, sondern nur „Verbindungsdienst läuft“.
- **Zugang:** „nur lokal“ nur nach gleicher Dienst-/PID-/Portprüfung wie `check.sh`;
  zusätzliche Wildcard-/IPv6-/fremde Listener werden nicht als sicher ausgegeben.
  Das ist kein Test der Erreichbarkeit vom Mac oder Handy.
- **Speicher:** belegte/harte GB je SSD aus lesendem Q_GETQUOTA (NVMe 10 + SATA 70).
  Falsche Grenzen heißen „Grenze pruefen“, erschöpfte Byte- **oder Inodequote**
  heißt „voll“. Kein Lesen oder Summieren privater Dateinamen.
- **Letzte Aktivität:** vorerst ausdrücklich **Dienststart** aus systemd mit
  Datum/Uhrzeit/Zeitzone, nicht letzte Anmeldung, Nachricht oder Agentenarbeit.
  Fehlende/ungültige Messung bleibt „unbekannt“. Für echte Arbeitsaktivität fehlt
  eine geprüfte, inhaltsfreie Hubquelle; Änderungsbitte an S3 bleibt offen.

Die Vorschau zeigt ausschließlich „nicht geprueft“. Fehlende/falsche Konten,
inaktive Dienste, fehlender Loopbacknachweis, fehlende Quotenabfrage sowie volle
oder abweichende Quoten ergeben Exitcode 1, sonst 0. Eine unbekannte letzte
Arbeitsaktivität ist eine erklärte Entwurfsgrenze, kein Anwesenheitsnachweis.
Der Dienststart ist auch bei ruhenden Sessions kein Beleg für einen KI-Auftrag.
Kein Journal, keine `/proc`-Argumente, Home-/Sessioninhalte, Geräte-IDs oder Token
werden gelesen. Nur für die Listenerzuordnung wird der Besitzer von `/proc/PID`
per `stat` abgefragt. Kein Ersatz für die technische Abnahme unten oder den Pilot.

### Technische Abnahme

`check.sh --apply` verlangt Root und prüft ohne Inhalte fremder Homes zu lesen:

- UID-Inventar, Home/Daten 0700 und passende Besitzer;
- via `runuser`: fehlendes Lese-/Suchrecht auf andere vorhandene Slot-Homes und
  `/home/medienhaus` (inklusive effektiver ACL-Wirkung);
- keine privilegierten Zusatzgruppen, kein schreibbarer Docker-Socket;
- vollständiges `sudo -l -U` muss verweigern, zusätzlich `sudo -n true` als Slot;
- aktive ext4-Userquote und über **nur lesendes** Q_GETQUOTA exakte Hard-/Soft-
  und Inodegrenzen sowie keine schon überhöhte Belegung;
- effektive Slice-Grenzen, Linger, privater Token ohne Inhaltsausgabe;
- aktiver eigener Hub, PID-Besitzer und genau ein Listener
  `127.0.0.1:Port`; weder `0.0.0.0` noch IPv6-Wildcard/weitere TCP-Listener des Hubs;
- Werkzeuge im Slot-PATH auffindbar (kein Ausführen von pi/Modellen).

Das ersetzt NICHT: echte Quotenüberschreitung, RAM-/Prozessbombentest,
Sessiongrenzen, Gerätewiderruf, Hostidentität, TLS, API-Pfadausbrüche oder Lasttest.
Diese zuerst auf separater Testmaschine/klein quotiertem Wegwerfkonto mit Mock
abnehmen; niemals 80 GB Testfüllung oder Forkbomben auf dem laufenden R2.

## 7. Rückbau, Sicherung und Wiederherstellung

### Rückbau (separate Freigabe nötig)

```bash
bash /usr/local/lib/cocopi-r2/teardown.sh --slots account01,account02             # nur Plan
bash /usr/local/lib/cocopi-r2/teardown.sh --apply --slots account01,account02     # sperrt + sichert; behält Daten/Konto
bash /usr/local/lib/cocopi-r2/teardown.sh --apply --slots account01,account02 --purge  # zusätzlich löscht
```

S3 muss Geräte-/Enrollmentzugänge **vor Wiedervergabe** widerrufen. Das Skript
macht keine Netzwerkanfrage. Es stoppt ausschließlich Dienste der expliziten
Slotkonten, sperrt Passwort/Accountablauf/Login-Shell und beendet deren
Usermanager. Bei weiteren Prozessen bricht es ab, statt breit Prozesse zu töten.
Erst dann Archiv von Home, SATA-Daten, Slotkonfiguration und UID-Inventar unter
`/var/backups/cocopi-slots/accountNN/`, root-only, mit numerischen UIDs, ACLs, xattrs,
SHA-256 und Lesbarkeitsprüfung. Symlinks werden nicht dereferenziert, untergeordnete
Mounts blockieren. Platzmangel/Archivfehler: keine Quelldatenlöschung.

Standard behält Konto und sämtliche Home-/Datenquellen. `--purge` erstellt
unmittelbar vor Löschung ein zusätzliches frisches Archiv (erfasst zwischenzeitlich
hinzugekommene Dateien). Marker `preparing → archived → purging → purged` erlauben
Wiederaufnahme. Unterbrochene Sicherung ohne Manifest verlangt manuelle Recovery;
niemals einfach eine neue Prüfsumme über einen verdächtigen Altbestand erzeugen.
Lokale Snapshots sind **kein externes Backup** und enthalten Geheimnisse.

UID-Inventar/Tombstone, Quoten und Slice-Limits bleiben nach Purge erhalten:
Restdateien außerhalb Home/Datapfad gehören weiter derselben UID. Keine automatischen
UID-Recyclings oder Slot-Neuanlagen nach begonnenem Rückbau. UID-/Restdatei-Inventar,
Gerätewiderruf und externe Sicherung administrativ abschließen; sonst könnten neue
Konten alte Dateien erben. Gemeinsame Gruppe/Unit/Wrapper bleiben für andere Slots.
Löschen des restlichen Verwaltungsgerüsts nur separat und bei null belegten Slots.

### Externer Backupplan (Vorschlag, noch nicht eingerichtet)

1. Verantwortliche Person und institutionelles Ziel **außerhalb R2**, z. B.
   verschlüsseltes Repository auf getrenntem Speicher, festlegen. Keine Ziel-URLs
   oder Schlüssel hier eintragen. Zweite interne SSD ist kein Backup.
2. Täglich inkrementell, vor Provisionierung/Migration zusätzlich Vollsicherung;
   zunächst 7 tägliche + 4 wöchentliche + 3 monatliche Stände als zu bestätigender
   Aufbewahrungsvorschlag. Private Datenfristen mit Seminar/Datenschutz abstimmen.
3. Home, SATA-Daten, Slotkonfiguration, UID/GID-Inventar, Slice-/Dienstvorlagen,
   rootverwaltete Deploymentversion und Quota-Sollwerte sichern; reproduzierbare
   Caches optional ausschließen, Sessiondateien nicht. Konsistente Sicherung nach
   Nutzerankündigung/Slotruhe; nicht parallel in aktive JSONL-Dateien schreiben.
4. Backupverschlüsselung + getrennte Schlüsselablage, Zugriff nur zuständige
   Administration, Prüfprotokoll ohne Token/Inhalte; automatische Fehleralarmierung
   ohne vertrauliche Nutzdaten. Lokales Archiv erst nach extern bestätigter Kopie
   und getesteter Wiederherstellung gemäß Frist entfernen (nicht dieses Skript).
5. Restore zunächst in isoliertes Temp-/Testziel, Archivpfade und Prüfsumme prüfen,
   keine blinde Extraktion als Root über das echte `/`. Danach kontrolliert mit
   ursprünglicher UID/GID, Rechten/ACLs, Quoten und Slice herstellen. Geräte- und
   Bootstrap-Token nach Vorfall neu ausstellen, alte widerrufen; Providerdaten
   separat und persönlich wieder einrichten. Hub zuerst mit Mock und ohne Registry-
   Auto-Resume prüfen. Niemals alte Requests blind erneut senden.
6. Vor Pilot eine Datei + komplette Session wiederherstellen; anschließend monatlich
   Stichprobe, quartalsweise kompletter Slot auf separatem Host. RPO ≤24 h / RTO
   ≤4 h als **Zielvorschlag**, nicht heute gemessene Zusage.

## 8. Lasttestplan: zwei Personen, dann neun

Nur nach Freigabe, mit Mocks und synthetischen unkritischen Daten. Keine echten
Modelle, keine fremden Prozesse/Windows steuern. Zehnter Slot bleibt Reserve.

| Stufe | Ablauf | Nachweis / Abbruch |
|---|---|---|
| 0 getrennte Testumgebung | kleine Quote füllen, Speicher-/Tasklimit kontrolliert erreichen, Restore/Widerruf | reproduzierbarer Fehler statt fremdem Zugriff; keine Forkbombe auf R2 |
| 1 zwei Personen account01/account02 | je 2 offene, max. 1 aktive Mocksession; dritte abweisen; Prompt, Rückfrage, Abbruch, Reconnect, Datei, Loginabbruch | keine doppelten Requests, private Homes/Token unerreichbar, verständliche Fehlermeldung |
| 2 ein schwerer CPU-Job global | kontrollierter Build oder synthetischer Export; anderer Slot reiht zweiten Job ein | globale Queue über beide Hubs belegen; direkte Shelljobs als Policygrenze separat dokumentieren |
| 3 schrittweise 3 → 6 → 9 | gleicher Ablauf, ein schwerer Job bleibt global; je Stufe 15 min plus Lastspitze | 9 Personen erst nach stabiler Vorstufe; Reserve unverändert |
| 4 Neustart-/Recoveryfenster | nur Test-Hub/Slot nach Zustimmung, Replay/Restore/Tokenwiderruf | neue Prozesse korrekt benennen, kein blindes Resume, geprüfte Sicherung |

Je Stufe Basis + Peaks messen: CPU/PSI, Account-cgroup-RAM, MemoryAvailable,
Swap-I/O, Tasks, Dateisystembelegung/Inodes/Quota, I/O-Wartezeit, Queuezeit,
UI-/Mockantwortzeiten. Werkzeuge nur sofern vorhanden, keine Paketinstallation
im Test. Rohdaten ohne Token/Prompts, Messzeit und Deploymentcommit festhalten.

Vorgeschlagene Pilot-Stopps: OOM-Kill, Rechte-/Quoteverletzung, doppelte Requests,
>90 % Dateisystembelegung, MemoryAvailable <4 GiB für 30 s oder dauerhaftes
Swap-Thrashing; UI-Steuerung >2 s wiederholt unter synthetischer Last. Bei Fehler
keine weitere Last hinzufügen, eigenen Test geordnet abbrechen und zur letzten
stabilen Belegung zurück. Werte sind vor Test mit Betrieb abzustimmen. Zweiter
schwerer Job / zwei aktive Sessions erst nach neuer ausdrücklicher Entscheidung.

## 9. A4-Kontomodell (eingefroren)

**Verbindlich: `coord/FREEZE-I1.md` §A4. Hubumsetzung noch unbestätigt.**
Slot-ID ist stabil und kein Anzeigename. Person,
Unixkonto, Hubinstanz und Geräte-ID werden serverseitig gebunden. Ein Port ist
keine Autorisierung. Pro Gerät eigener widerrufbarer Token, niemals im Inventar
als Klartext; Tabelle enthält nur einen symbolischen Verweis.

| Slot | Unixkonto | Hub-Userdienst / Loopbackport | Geräte-Token (nur Verweis) | Grenzen |
|---|---|---|---|---|
| account01 | account01 | cocopi-hub@account01 / 8801 | account01/<device-id> | G |
| account02 | account02 | cocopi-hub@account02 / 8802 | account02/<device-id> | G |
| account03 | account03 | cocopi-hub@account03 / 8803 | account03/<device-id> | G |
| account04 | account04 | cocopi-hub@account04 / 8804 | account04/<device-id> | G |
| account05 | account05 | cocopi-hub@account05 / 8805 | account05/<device-id> | G |
| account06 | account06 | cocopi-hub@account06 / 8806 | account06/<device-id> | G |
| account07 | account07 | cocopi-hub@account07 / 8807 | account07/<device-id> | G |
| account08 | account08 | cocopi-hub@account08 / 8808 | account08/<device-id> | G |
| account09 | account09 | cocopi-hub@account09 / 8809 | account09/<device-id> | G |
| account10 Reserve | account10 | cocopi-hub@account10 / 8810 | erst bei Vergabe | G |

Private Primärgruppen und feste UID/GID **20001…20010** sind eingefroren;
`SLOT_UID_BASE=20000` muss vor Apply gegen lokale/NSS-Kollisionen geprüft werden.
Zwischen Slots werden keine UIDs automatisch wiederverwendet. Hostweite
Administrationswerkzeuge müssen diesen Bereich separat reservieren; ein
Tombstone allein verhindert außerhalb dieser Skripte keine UID-Wiedervergabe.

**G:** CPU 100 %, RAM High 2 GiB / Max 2,5 GiB, 256 Tasks, 10+70 GB,
200.000+800.000 Inodes, 2 offene / 1 aktive Session. Schwerer Job **1 global**.
Grenzen sind keine garantierten Ressourcen, Änderungen nur nach Lastbefund.

### Konkrete Änderungsbitten an S3

- Vertrag/Variablennamen bestätigen: `HUB_SLOT`, `HUB_MAX_OPEN_SESSIONS=2`,
  `HUB_MAX_ACTIVE_SESSIONS=1`, `HUB_GLOBAL_HEAVY_JOBS=1`, `HUB_STATE` als
  privates Verzeichnis 0700 für das Geräte-Register, Loopback/Port/ROOTS.
  Aktueller Hub ignoriert die neuen Grenzen; deshalb Apply-Sperre nicht entfernen.
- Geräte-Token dem authentifizierten Slot binden; fremde Slots/Roots/IDs abweisen;
  Ablauf/Widerruf inklusive bestehender Streams abnehmen. Bootstrap-Token nicht als
  Gerätetoken oder Einladung behandeln; Adminpfad getrennt halten.
- Prozessübergreifende globale Schwerlastqueue definieren (neun Hubs!), ohne
  gemeinsame vertrauliche Sessions. Direkter Unix-Shellzugriff kann sie umgehen;
  keine stärkere Garantie behaupten, als tatsächlich erzwungen wird.
- Vertrag `cocopi-slots-v1` nach Negativtests K zum Review/Freigeben vorlegen; Pfad/Registry/Persistenz,
  Restart/Schlafmodus und Queue-Autorisierung schriftlich festlegen.

Tutorial bleibt in I1 unverändert; Überarbeitung erst I3 nach Freigabe von S1.
