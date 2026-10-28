# Einrichtung mit pi begleiten lassen

**Nur für eine spätere ausdrücklich freigegebene Administration.** In I2 wurden
keine Pakete, Konten, Quoten, Dienste, Proxy- oder authentik-Einstellungen geändert.
Die Vorlagen sind kein nachgewiesener Produktivbetrieb. Zuerst
[`README.md`](README.md), [`AUTHENTIK.md`](AUTHENTIK.md) und
[`DATENSCHUTZ.md`](DATENSCHUTZ.md) lesen. Ein alter I1-Dienst darf nicht ungefragt
migriert oder gestoppt werden.

[`SCHNELLSTART.md`](SCHNELLSTART.md) enthält die sechs Einstiegsbefehle.

## Diesen Prompt in eine eigene pi-Sitzung kopieren

```text
Begleite mich interaktiv bei der cocopi-Pool-Einrichtung. Du bist kein
unbeaufsichtigter Installer. Arbeite nur mit deploy/r2 aus meinem geprüften
Commit. Verändere nie meine laufende pi-Sitzung und starte kein Modell zum Testen.

Regeln:
- Beginne mit git branch --show-current und git status --short, danach lies
  deploy/r2/README.md, AUTHENTIK.md, DATENSCHUTZ.md und einrichten.sh vollständig.
- Zeige jeden vorgesehenen Befehl VOR Ausführung in einem eigenen Codeblock und
  erkläre Wirkung, betroffene Pfade und Rücknahme. Keine Befehlsketten verstecken.
- Frage VOR JEDEM sudo oder sudoedit ausdrücklich „Diesen einzelnen Befehl jetzt
  ausführen? Bitte ja oder nein.“ Warte auf ein neues ausdrückliches ja. Ein ja
  gilt genau für diesen Befehl, nicht für spätere Schritte. Bei nein abbrechen.
- Auch bei unprivilegiertem Netzwerkzugriff, Dienststart, Löschen, Installation,
  authentik- oder Tailscale-Änderung vorher genau die Wirkung erklären und ja
  abwarten. Kein sudo -s, keine dauerhafte Rootshell, kein sudo bash -c-Bündel,
  keine automatisch akzeptierten Rückfragen und keine --apply-Abkürzung.
- Ein sudo-Aufruf von einrichten.sh beinhaltet mehrere beschriebene Änderungen.
  Zeige dafür vorher den vollständigen nummerierten Dry-Run und fordere eine
  gesonderte Freigabe genau dieses Plans. Nicht als harmlosen Einzelbefehl tarnen.
- Lies/zeige keine Tokens, privaten Schlüssel, Prozessumgebungen, Sessioninhalte
  oder authentik-Livekonfiguration. Geheimnisse nur mit sudoedit eintragen lassen.
  Öffentliche Client-ID/Issuer/Fingerprint bewusst getrennt behandeln.
- Nutze nur account01 und account02 bis zur ausdrücklichen Zehnerpool-Freigabe.
- Installiere keine Pakete und ändere keine Mounts/Quotenaktivierung/Firewall
  automatisch. Fehlende Voraussetzungen separat notieren und hier stoppen.

Beginne ohne sudo mit:
  python3 deploy/r2/test_scripts.py
  bash deploy/r2/einrichten.sh --dry-run --slots account01,account02
  bash deploy/r2/admin-check.sh --dry-run --slots account01,account02

Prüfe danach gemeinsam die noch offenen S3/S7-Integrationsnachweise:
  produktiver Archivweg; signierte Belegung/Freigabe; Empfang nutzt engen
  sudo-Wrapper; N7: Exit 0 und letzte leeren-JSON-Zeile mit zustand:"leer",
  nachweis:"vor_hubstart", passendem Slot und neuer Epoche auswerten. Danach
  KEIN pruefen, auch nicht --beleg, durch den Empfang. K1-Abnahme: Vollständiger
  Check-out gibt mit echtem cocopi-slotctl --fake im Tempverzeichnis frei,
  nicht nur mit gefaktem clearSlot. S4-Helfertests ersetzen diesen S7-Test nicht.
  Administration liest Belege mit cocopi-slotctl pruefen --beleg account01,
  ohne Live-Prozessprüfung; aktuelle inhaltsfreie Poolstatusprojektion vorhanden;
  authentik-PKCE und tatsächlicher Callback-Pfad stimmen; Caddy und visudo haben
  die exakten Dateien validiert; N3-Replaytest nach Leeren ist grün. Empfang liest
  EMPFANG_EPOCHEN_DIR=/var/lib/cocopi/slots, Hub verlangt
  HUB_EPOCHE_DATEI=/var/lib/cocopi/slots/accountNN.epoche. Belegung und Freigabe
  enthalten die aktuelle Epoche. Fehlende Datei ist im Betrieb ein Fehler, nicht 0.
  Fehlt ein Nachweis: STOPP, keine Freigabeflags
  setzen. „bestaetigt“ ist kein Ersatz für einen Test.

N8–N10 ebenfalls mit Mocks nachweisen, keine echten Modellaufrufe:
  N8: aktiv{verbunden:true} alle <=60 s bei Desktop- ODER Mobilverbindung,
  false beim sauberen Trennen. Zwangs-Check-out nur !verbunden und >8 h seit
  letztem Signal oder nächstes Tagesende >= Check-in. Dienststart beweist
  keine Aktivität. Prüfe aktive und getrennte Geräte sowie Check-in nach 23:30.
  N9: GET /v1/empfang/seminar nur eingecheckt/geräte-signiert, Antwortsignatur
  prüfen; Rollen weiterhin nur aus dem Geräteausweis des Peers, nicht aus
  Verzeichnis/Client-JSON. Bestehende Proxyroute /v1/empfang/* reicht.
  N10: Zwangsarchiv maximal 384 MiB Klartext (402653184 Bytes). Darüber
  erzwingen_blockiert, Backoff 5/15/60 min und sichtbarer Status für die Leitung;
  Daten bleiben bis manuell auf R2. Niemals Fehler durch Leeren/Freigabe beheben.
  Die Abholfrist von <=72 h gilt für das erzeugte verschlüsselte Abholfach,
  nicht als Erlaubnis zum Löschen eines blockierten, noch nicht gesicherten Slots.

Erst nach menschlicher D-R1-Freigabe und dokumentiertem Teststand darf eine
geprüfte, unveränderliche Skriptkopie root:root, ohne Gruppenschreibrechte,
unter /usr/local/lib/cocopi-r2 liegen. Vor Kopie jeden Quellpfad und Link prüfen;
nie sudo direkt auf Skripte im beschreibbaren Worktree anwenden. Lege keine
Providerkonfiguration oder persönliche Sitzungen ins Skelett. Bei bestehenden
Empfangsdateien niemals Vorlage darüberkopieren. AUTHENTIK.md Schritt für Schritt.
Den Empfangsschlüssel-Initialstart gesondert mit S7 klären und freigeben.
Der gemergte Empfangseinstieg ist /opt/cocopi/pi-hub/empfang/empfang.mjs
(nicht empfang/server.mjs). In /etc/cocopi/empfang.env exakt setzen:
  EMPFANG_PORT=8800
  EMPFANG_STATE=/var/lib/cocopi-empfang
  EMPFANG_SLOTS=account01,account02
  EMPFANG_ORIGIN=https://NAME
  EMPFANG_OIDC_ISSUER=<exakter HTTPS-Issuer inklusive Pfad>
  EMPFANG_OIDC_CLIENT_ID=<öffentliche Client-ID>
  EMPFANG_SLOTCTL=/usr/local/lib/cocopi-r2/slotctl-empfang
  EMPFANG_TAGESENDE=23:30
  EMPFANG_EPOCHEN_DIR=/var/lib/cocopi/slots
Keine zusätzlichen erfundenen EMPFANG_*-Schlüssel, keine Shell-Ausdrücke.
N7–N10 benötigen keine neuen Variablen: vorhandener Helfer/Epochenpfad,
Tagesende und Schlüssel reichen; N10-Grenze und Backoff sind feste Vertragswerte.
Die
Prüfung in einrichten.sh liest nur Daten und zeigt keine Werte an. Die Namen
stammen aus S7-Code, nicht aus älteren Entwürfen; Epochen-Einbindung im Hub und
Empfang muss K vor Deployment gemeinsam integrieren und nachweisen.
Neue Konten erhalten eine root:root-Datei 0644 mit Epoche 0. Vorhandene Zähler
bleiben unverändert. Fehlende Bestandsdateien niemals einfach mit 0 ersetzen:
Check-ins stoppen, Freigaben/Replayrisiko prüfen, Migration gesondert bestätigen.

Zeige vor der Ausführung nochmals:
  bash /usr/local/lib/cocopi-r2/einrichten.sh --dry-run --slots account01,account02
Falls wirklich alle Sperren abgearbeitet sind, erkläre die gesamte Wirkung und
frage ja für genau den folgenden Ausführungslauf. PUBKEY durch den unabhängig
geprüften öffentlichen 43-Zeichen-Schlüssel ersetzen, niemals privaten Schlüssel:
  sudo env COCOPI_D_R1=freigegeben COCOPI_POOL_ABNAHME=bestaetigt HUB_EMPFANG_PUBKEY=PUBKEY bash /usr/local/lib/cocopi-r2/einrichten.sh --ausfuehren --slots account01,account02

Proxy separat wählen: Caddy-TLS ODER Tailscale-TLS vor lokalem Caddy. Nie direkt
Port 8800 oder Slotports an tailscale serve geben. Keine existierende
Serve-Konfiguration überschreiben. N5: /v1/belegung, /v1/freigabe und
/v1/sync/dienst/* sind direkt UND mit /slot/accountNN-Präfix zu sperren.
Caddy/visudo erst validieren. Jede notwendige
Installation/Netzänderung erneut mit Einzel-„ja“ bestätigen, niemals Funnel.
Keine realen Dateien oder KI-Aufrufe zur Abnahme.

Beende die Sitzung zwingend mit der sichtbaren Betriebsprüfung. Zuerst ohne sudo:
  bash /usr/local/lib/cocopi-r2/admin-check.sh --dry-run --slots account01,account02
Dann erkläre: liest lokale Konten, Prozesse/PIDs, Quoten und Poolstatus sowie
für freie Plätze den historischen N7-Leerbeleg mit aktueller Epoche (kein
Live-pruefen nach Hubstart); stellt
inhaltsfreie hallo- sowie absichtlich ungültige POST-Anfragen an den expliziten
HTTPS-Proxy. Frage ein neues ja vor genau diesem sudo (NAME ersetzen):
  sudo bash /usr/local/lib/cocopi-r2/admin-check.sh --ausfuehren --slots account01,account02 --proxy-url https://NAME

Berichte Exitcode, echte Befunde, sichtbare Epoche je Platz, nicht geprüfte Teile und nächste Schritte.
„nicht geprüft“, fehlende Statusdatei, belegte/gesperrte Plätze, falsche Quoten
oder nicht bestätigte 403-Sperren dürfen niemals als erfolgreiche Abnahme gelten.
Die Zwei-Platz-Erprobung ersetzt keinen Test mit zehn Plätzen. Ende ohne Push,
Merge, Modellaufruf oder Neustart meiner Anwendungen.
```

## Wichtige Grenzen

- `--ausfuehren` bei `admin-check.sh` bedeutet **nur lesen und Diagnoseanfragen**;
  beim Einrichten bedeutet es reale lokale Systemänderungen. Nicht verwechseln.
- Die erforderlichen beiden Freigabevariablen sind bewusst eine Sperre gegen
  versehentliche Bereitstellung des derzeit nicht durchgehend integrierten Pools.
- Der Löschhelfer bekommt niemals einen freien Pfad oder frei wählbare UID.
  `leeren` ohne `--ausfuehren` zeigt nur den Plan. Ein tatsächlicher manueller
  Löschaufruf braucht separate Freigabe und vorherigen bestätigten Check-out.
- Keine Nutzdatenarchive des alten I1-`teardown.sh` anlegen: dessen echte
  Ausführung ist im Pool gesperrt. Kontenabbau ist eine eigene Aufgabe nach
  vollständiger Rückgabe, nicht der normale Check-out.
