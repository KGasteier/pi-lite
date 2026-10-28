# Einrichtung mit pi begleiten lassen

**Nur für eine spätere ausdrücklich freigegebene Administration.** In I2 wurden
keine Pakete, Konten, Quoten, Dienste, Proxy- oder authentik-Einstellungen geändert.
Die Vorlagen sind kein nachgewiesener Produktivbetrieb. Zuerst
[`README.md`](README.md), [`AUTHENTIK.md`](AUTHENTIK.md) und
[`DATENSCHUTZ.md`](DATENSCHUTZ.md) lesen. Ein alter I1-Dienst darf nicht ungefragt
migriert oder gestoppt werden.

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
  sudo-Wrapper; es akzeptiert den Löschbeleg VOR Hubstart, nicht ein erneutes
  Live-pruefen nach Start; aktuelle inhaltsfreie Poolstatusprojektion vorhanden;
  authentik-PKCE und tatsächlicher Callback-Pfad stimmen; Caddy und visudo haben
  die exakten Dateien validiert. Fehlt ein Nachweis: STOPP, keine Freigabeflags
  setzen. „bestaetigt“ ist kein Ersatz für einen Test.

Erst nach menschlicher D-R1-Freigabe und dokumentiertem Teststand darf eine
geprüfte, unveränderliche Skriptkopie root:root, ohne Gruppenschreibrechte,
unter /usr/local/lib/cocopi-r2 liegen. Vor Kopie jeden Quellpfad und Link prüfen;
nie sudo direkt auf Skripte im beschreibbaren Worktree anwenden. Lege keine
Providerkonfiguration oder persönliche Sitzungen ins Skelett. Bei bestehenden
Empfangsdateien niemals Vorlage darüberkopieren. AUTHENTIK.md Schritt für Schritt.
Den Empfangsschlüssel-Initialstart gesondert mit S7 klären und freigeben.

Zeige vor der Ausführung nochmals:
  bash /usr/local/lib/cocopi-r2/einrichten.sh --dry-run --slots account01,account02
Falls wirklich alle Sperren abgearbeitet sind, erkläre die gesamte Wirkung und
frage ja für genau den folgenden Ausführungslauf. PUBKEY durch den unabhängig
geprüften öffentlichen 43-Zeichen-Schlüssel ersetzen, niemals privaten Schlüssel:
  sudo env COCOPI_D_R1=freigegeben COCOPI_POOL_ABNAHME=bestaetigt HUB_EMPFANG_PUBKEY=PUBKEY bash /usr/local/lib/cocopi-r2/einrichten.sh --ausfuehren --slots account01,account02

Proxy separat wählen: Caddy-TLS ODER Tailscale-TLS vor lokalem Caddy. Nie direkt
Port 8800 oder Slotports an tailscale serve geben. Keine existierende
Serve-Konfiguration überschreiben. Caddy/visudo erst validieren. Jede notwendige
Installation/Netzänderung erneut mit Einzel-„ja“ bestätigen, niemals Funnel.
Keine realen Dateien oder KI-Aufrufe zur Abnahme.

Beende die Sitzung zwingend mit der sichtbaren Betriebsprüfung. Zuerst ohne sudo:
  bash /usr/local/lib/cocopi-r2/admin-check.sh --dry-run --slots account01,account02
Dann erkläre: liest lokale Konten, Prozesse/PIDs, Quoten und Poolstatus; stellt
inhaltsfreie hallo- sowie absichtlich ungültige POST-Anfragen an den expliziten
HTTPS-Proxy. Frage ein neues ja vor genau diesem sudo (NAME ersetzen):
  sudo bash /usr/local/lib/cocopi-r2/admin-check.sh --ausfuehren --slots account01,account02 --proxy-url https://NAME

Berichte Exitcode, echte Befunde, nicht geprüfte Teile und nächste Schritte.
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
