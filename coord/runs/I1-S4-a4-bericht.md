# I1-S4-a4 · Abschlussbericht

Stand: 28.10.2026, 18:57 CET. Arbeitsbaum `/home/medienhaus/Work/pi-lite-wt/s4`,
Branch `v3/s4`. Auftrag im Zeitbudget abgeschlossen; keine Live-Provisionierung.

## Checkpoints

- Ausgangspunkt `0b98b19`, Arbeitsbaum sauber geprüft.
- Zuerst `cocopi-v3` eingemergt: `01f10f2`; `coord/FREEZE-I1.md` §A4 gelesen.
- `a23ec76` — **cocopi v3 I1-S4-a4: Accountnamen und privaten Hubzustand an A4 angleichen**.
- `1187a2b` — **cocopi v3 I1-S4-a4: Lesende Lehrendenübersicht mit isolierten Negativtests ergänzen**.
- Beide eigenen Commits mit konfigurierter Autoridentität KGasteier; ausschließlich explizite S4-Dateipfade gestagt.
- Arbeitsbaum nach Abschluss sauber. Kein Push, kein weiterer Merge.

## Umsetzung

### A4-Namen und Deploymentvertrag

Alle zehn Deploymentziele heißen jetzt `account01` bis `account10`; Unixkonto und
Slot-ID sind identisch. Standardauswahl: `--slots account01,account02`.
Ports bleiben 8801…8810, UID/GID bleiben 20001…20010; account10 bleibt Reserve.
Homes `/home/accountNN`, SATA `/mnt/data/cocopi/accountNN`, Dienste
`cocopi-hub@accountNN.service`. Alte Slotnamen werden als CLI-Eingabe abgewiesen.
Keine automatische Migration alter Konten/Daten.

Erzeugte Hubumgebung verwendet exakt:

```text
HUB_SLOT=accountNN
HUB_MAX_OPEN_SESSIONS=2
HUB_MAX_ACTIVE_SESSIONS=1
HUB_GLOBAL_HEAVY_JOBS=1
HUB_STATE=/mnt/data/cocopi/accountNN/hub-state
```

`HUB_STATE` ist ein privates Verzeichnis 0700 im quotierten Datenbereich. Es wird
unprivilegiert als Slotkonto angelegt, damit Root nicht in einem vom Slot
beschreibbaren Unterpfad arbeitet; Besitzer/Gruppe/Rechte werden geprüft.
Bestehende Registerinhalte werden nicht gelesen oder überschrieben. Der
Startwrapper prüft Kontoname, Slot, Loopbackbindung, Grenzwerte und Registerpfad
vor dem Tokenlesen. `check.sh` prüft das private Registerverzeichnis ebenfalls.

**Vertragsmarker unverändert `HUB_CONTRACT=unconfirmed`.** Kein Freigabeanspruch.
K muss nach S3-Review freigeben. Der im bisherigen Hub schreibgeschützt gelesene
Stand behandelt `HUB_STATE` noch als Datei bzw. `off`; er ist zum neuen
Verzeichnisvertrag nicht kompatibel. S3 muss dies implementieren/abnehmen.
Die globale CPU-/Heavy-Job-Queue über gemeinsame flock-Datei bleibt I2; die neue
Umgebung beweist keine bereits wirksame kontoübergreifende Queue.

### Lehrenden-Sicht / Aylin

Neu: `deploy/r2/admin-check.sh`, Aufruf mit `bash`, standardmäßig Vorschau.
Das spätere `--apply` ist ausschließlich lesend und verlangt Root sowie eine
rootverwaltete Skriptkopie. Kein Hub-/HTTP-/Modellaufruf.

Pro Slot zeigt die ruhige Tabelle Konto, aktiven persönlichen Hubdienst,
Loopbacknachweis, NVMe-/SATA-Belegung und Aktivitätsquelle. Wiederverwendete
technische Prüfung verlangt genau den richtigen Dienst-PID/Besitzer und
`127.0.0.1:PORT`; zusätzliche öffentliche, IPv6- oder fremde Listener werden
nicht grün gemeldet. Quotenbelegung stammt aus Q_GETQUOTA, nicht aus privaten
Dateinamen. Fehlende/falsche Grenzen sowie volle Byte- oder Inodequoten sind
sichtbar. Probleme ergeben Exitcode 1; rein erfolgreiche belegte Betriebswerte 0.

**Aktivitätsgrenze:** Aktuell ist nur ein ausdrücklich als **Dienststart**
beschrifteter systemd-Zeitpunkt verfügbar (mit Datum/Uhrzeit/Zeitzone).
Fehlende/ungültige Werte bleiben unbekannt. Letzte Hubnachricht/Agentenarbeit
bleibt unbekannt; weder Dienststart noch Dateiänderungen werden als Arbeit oder
Anwesenheit ausgegeben. Dafür fehlt eine geprüfte inhaltsfreie Hubquelle von S3.
Die Tabelle ist somit ein vorgezogener I2-Entwurf, keine vollständig abgenommene
Lehrenden-/Aktivitätsansicht und kein Ersatz für `check.sh` oder den Pilot.

Es werden keine Gesprächsinhalte, Home-/Sessiondateien, Token, Gerätekennungen,
Journale oder Prozessargumente gelesen. `/proc/PID` wird nur zur Besitzerprüfung
per stat verwendet. Die Administration führt den Bericht aus; keine neuen
Lehrendenzugriffe auf private Konten.

README, aktuelle DRYRUN-/TESTS-Nachweise und Einordnung des historischen BEFUND
aktualisiert. Historische alte Namen im BEFUND bleiben ausdrücklich als damalige
Prüfbefehle erhalten; alte Namen in Tests sind Negativfälle.

## Prüfung und Belege

- `python3 deploy/r2/test_scripts.py`: **23/23 erfolgreich**, zuletzt 1,570 s.
- `bash -n` einzeln für alle `deploy/r2/*.sh` und `slots.conf`: erfolgreich.
- `git diff --check`: erfolgreich.
- Dry-Runs für provision/check/admin-check/teardown mit
  `--slots account01,account02`: erfolgreich; protokolliert in `deploy/r2/DRYRUN.txt`.
- Testausgabe und einzelne Syntaxnachweise: `deploy/r2/TESTS.txt`.
- Isoliertes temporäres HOME/XDG_CONFIG_HOME, mutierende Befehle durch
  Abbruchattrappen geschützt; keine echten `~/.config`-Testdaten.
- Mockfälle: alle zehn Zuordnungen und stabilen IDs; erzeugte genaue
  Umgebungsnamen/-werte; 0700-Registerplan; alte/ungültige Slotnamen;
  unbestätigter Vertrag vor Dienstaktionen; Konto fehlt/falsch; aktiver/inaktiver
  Dienst; öffentlicher/IPv6/fremder/zusätzlicher Listener; falscher PID-Besitzer;
  fehlende Quote/falsche Grenzen/volle Bytes oder Inodes; unbekannter und korrekt
  beschrifteter Dienststart; ungültige Zeitwerte ohne Durchreichen des Rohtexts.

## Grenzen / nächste Entscheidungen

1. S3: Verzeichnisvertrag, Slotgerätebindung und Session-/Heavy-Job-Limits reviewen;
   K danach Freigabeentscheidung. Marker bis dahin **unconfirmed** lassen.
2. S3/K: geprüfte, inhaltsfreie Quelle für tatsächliche letzte Hubaktivität festlegen;
   Dienststart bleibt nur ein klar benannter Ersatzindikator, kein Arbeitsnachweis.
3. Menschliche Freigabe für Quoten/Ressourcengrenzen und weitere Apply-Voraussetzungen
   aus README/BEFUND bleiben offen. Q_GETQUOTA-Belegung allein beweist keine aktive
   Quotenbegrenzung. Keine tatsächlichen Konten, Dienste, Ports oder Quoten abgenommen.
4. Keine realen Konto-, Service-, Mount-, Paket- oder Netzwerkänderungen; keine
   Internet-/Modellaufrufe, keine fremden Prozesse angefasst. Kein Live-Test/Lasttest,
   keine Behauptung einer Pilotfreigabe. Hub nur schreibgeschützt angesehen.

Heartbeats an `/home/medienhaus/Work/pi-lite/coord/heartbeat/s4` fortgeschrieben.
Dieser Bericht liegt wie beauftragt im Hauptarbeitsbaum und wurde nicht in den
S4-Deploymentcommit aufgenommen; fremde Koordinationsänderungen blieben unberührt.
