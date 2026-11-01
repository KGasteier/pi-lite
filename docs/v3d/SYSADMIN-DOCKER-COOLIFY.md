> Bebilderte vollständige Exportfassung mit relativen Bildpfaden und Anhängen: `~/cocopi-docs/v3-sysadmin-docker-coolify/`. Diese Repositoryfassung ist die Textreferenz; Bilder verbleiben im separaten Dokumentpaket.

# cocopi v3d · R2/Negatron mit Docker und Coolify

## Didaktisches Sysadmin-Handbuch · zehn pi/pi-hub-Container · UdK · P2P · Seminarpilot

**Neue Variante, neues Dokument.** Die Unixaccount-Version `cocopi-v3` und ihr Handbuch bleiben erhalten. Entwicklung der Containeralternative auf **`cocopi-v3d`** in pi-lite und pi-hub. Ausgangspunkte: pi-lite `d7d3f59`, Hub `e134cdd`. Diese Anleitung installiert nichts automatisch auf R2.

**Wichtigster Befund:** Zehn getrennte pi/pi-hub-Container lassen sich mit den beigefügten Vorlagen aufbauen. Der vollständige cocopi-v3-Empfang ist aber noch auf Unixpool, Loopbackkommunikation und systemd-Löschhelfer ausgelegt. **Ein fertiger Docker-SSO-P2P-Pool ist nicht vorhanden.** Die fehlenden Integrationsarbeiten sind ausdrücklich beschrieben. Der Infrastructure-Pilot ist startbar nach den Betriebsfreigaben; der vollständige Mehrpersonenlauf bleibt bis zur Umsetzung seiner Voraussetzungen gesperrt.

**Leserführung:** Zuerst Architektur/Stopppunkte (1–3), dann R2/Coolify/Image/Container (4–8), anschließend die Produktintegration (9–12), schließlich reale Abnahme und Seminar (13–15). Alle Befehle vor Ausführung erklären, Platzhalter ersetzen und Resultate prüfen. Nicht auf Verdacht Produktionsdienste neu starten.

**Abbildungen:** tatsächliche cocopi-egui-Oberfläche mit synthetischen Szenen aus der bestehenden Testversion; zusätzlich echte Coolify-UI-Aufnahmen aus der offiziellen Dokumentation, mit Herkunft bezeichnet. Keine davon ist ein Screenshot eines von uns auf R2 installierten Coolify. Keine fingierte Live-Abnahme.

<!-- PAGEBREAK -->

# 1 · Was ändert sich gegenüber Unixaccounts?

## 1.1 Zielbild

```text
Lehrperson/Studierende: cocopi Desktop
       | HTTPS + persönliche Geräteberechtigung
       v
Geprüfter Ingress/Proxy auf R2
       | öffentliche Clientrouten, interne Routen gesperrt
       v
Empfang: OIDC, Belegung, Warteschlange, Verzeichnis
       | signierte interne Nachrichten [Containerintegration fehlt]
       +--- Container account01: pi-hub + pi CLI + Codingtools
       +--- Container account02: pi-hub + pi CLI + Codingtools
       ...
       +--- Container account10: pi-hub + pi CLI + Codingtools

Coolify: Admin-Control-Plane für Images/Deployments/Überwachung
Lifecyclebroker: eng begrenztes Stoppen/Leeren/Neuaufbauen [fehlt]
UdK-API-Broker: Person → erlaubte Modelle/kurzlebiger Zugang [fehlt]
P2P: Desktop <---- direkte QUIC-Verbindung ----> Desktop
```

**Eine Person pro aktuell belegtem Container.** Darin läuft der Hub als langlebiger Prozess und startet pi als RPC-Kindprozess für Coding-Sessions. Zehn Container sind zehn getrennte Arbeitsplätze, nicht zehn fest an Personen gebundene Daueridentitäten. `account01` bleibt für den späteren Protokollvertrag die Slot-ID; im Infrastrukturpilot heißen die Services zunächst `slot01` bis `slot10`, ohne bereits eine Empfangsbelegung vorzutäuschen.

## 1.2 Was Coolify übernimmt – und was nicht

Coolify verwaltet Images, Deployments, Dockerressourcen, Domains und administrative Bedienung. Es ist **nicht** die Seminaridentitätsverwaltung, vergibt keine cocopi-Ausweise, verteilt keine persönlichen UdK-Modellkeys, realisiert keinen Checkoutvertrag und ersetzt kein P2P-Netzwerk.

Coolify-Administratoren haben praktisch Hostkontrolle. Studierende bekommen **keinen Coolify-Adminzugang** und keinen Docker-Socket. Sie arbeiten ausschließlich über den zugeteilten Hub. Ein rootähnlicher Coolify-Serviceaccount ist kein Teilnehmeraccount.

## 1.3 Erhaltene Eigenschaften und neue Risiken

- Prozesse verschiedener Container haben eigene Namespaces; alle teilen den Hostkernel. Docker ist keine VM-Isolation gegen beliebigen bösartigen Code.
- Kein Hostnetzmodus, kein `privileged`, keine Zusatzcaps, keine Docker-/containerd-Sockets im Agentencontainer.
- RootFS schreibgeschützt; HOME und kontrolliertes `/tmp` schreibbar. Compilertools dürfen innerhalb dieser Grenzen arbeiten; beliebiges `apt install` als Root ist nicht vorgesehen.
- CPU/RAM/PIDs werden begrenzt; **Speicherquoten kommen nicht automatisch mit Volumes**.
- Containerrecreate löscht ein persistentes Home nicht. Alte Sitzungen/Keys verschwinden nicht durch einen neuen Containernamen.
- Automatische Updates/Rolling Deployments können Belegungen beschädigen. Während einer Belegung keine automatischen Deployments. Ein Arbeitsplatz darf niemals parallel in zwei Containern mit demselben Home laufen.

<!-- PAGEBREAK -->

# 2 · Featureabhängigkeiten und Umsetzungslücken

| Für Feature Y | Muss X vorher erfüllt sein | Konkrete Maßnahme |
|---|---|---|
| Containerstart | Docker/Coolify, Image, Ablage | Kapitel 4–8, zunächst ein Platz |
| pi-Remote-Coding | pi CLI, Werkzeuge, privater Hubtoken | Image und RPC-Mock prüfen, danach echte Modelle |
| Zehn unabhängige Plätze | zehn Homes/Token/Netze, Ressourcen/Quoten | Composegenerator, Storageabnahme |
| UdK-Login | Authentik Public Client/PKCE, richtige Redirects/Claims | Kapitel 10 |
| Automatische Slotzuteilung | containerfähiger Empfang und Lifecycle | Kapitel 9 implementieren, nicht umgehen |
| Persönliche Modelle | API-Freigabe und Zuordnung zur Belegung | Kapitel 11: Pilot oder Broker |
| Sicherer Checkout | Archivbestätigung, Stoppen, Leeren, neue Epoche | Lifecyclebroker und Negativtests |
| Seminar/DM | verifizierte Ausweise, Verzeichnis, direktes Clientnetz | Kapitel 12 |
| QR-Handykopplung | HTTPS, eigene Geräteberechtigung | kein API-Key im QR; Gerätewiderruf testen |
| Drei-Personen-Abnahme | alle obigen Kernschritte plus Budgetfreigabe | Kapitel 13 |
| Seminar mit acht | bestandener Drei-Personen-/Last-/Löschtest | Kapitel 14 |

## 2.1 Vier belegt inkompatible Stellen

1. `pi-hub/empfang/empfang.mjs::slotConfiguration` akzeptiert derzeit ausschließlich `http://127.0.0.1:PORT`. Namen wie `http://slot01:8787` werden abgewiesen.
2. `lib/hub-v1.mjs` verlangt für Belegung/Freigabe Loopback als Socketquelle; `lib/sync-v1.mjs` für interne Syncdienste ebenfalls. Eine Dockerbridgequelle ist kein Loopback.
3. Der Empfang bindet in seinem CLI-Einstieg an `127.0.0.1`. Ein normaler Proxycontainer erreicht diesen Listener in einem getrennten Namespace nicht.
4. `deploy/r2/cocopi-slotctl` und die Einrichtung verwalten Unixkonten, Homes, Quoten und systemd. Sie dürfen nicht unverändert auf Containerhomes losgelassen werden.

**Nicht lösen durch:** Sicherheitsprüfungen deaktivieren, Hostnetz für alle Container, den Empfang mit Docker-Socket ausstatten oder auf ungeschütztes `0.0.0.0` umstellen. Produktänderungen mit eigenem Threatmodel, Tests und Review gehören in `cocopi-v3d`.

## 2.2 Weitere bestehende Lücken

Automatischer OIDC→API-Zugang fehlt. Eigenständiger Empfangsschlüsselbootstrap fehlt. Der Unix-Envprüfer erlaubt `EMPFANG_TRUST_PROXY` noch nicht; die Containerkonfiguration braucht einen eigenen konsistenten Prüfer. Eine ältere Authentik-Datei nennt `/callback`; korrekt im aktuellen Client ist **`/oidc/callback`**. Diese Punkte verschwinden nicht durch Coolify.

<!-- PAGEBREAK -->

# 3 · Verantwortlichkeiten, Freigaben und Automatisierungsgrenze

## 3.1 Vorab schriftlich klären

- R2-Betrieb: Betriebssystem, vorhandene Docker-/Proxydienste, Wartungsfenster, Backups/Rückfallkonsole, Datenträger/Quoten.
- Neue ausdrückliche **Docker-/Coolify-Freigabe**: Die frühere Unixpoolfreigabe gilt nicht automatisch. Dockerdaemon/Firewalländerungen können bestehende Dienste betreffen.
- Coolify-Admin, Updates und Registryzugriff; öffentliche Domains für Dashboard und Seminar getrennt.
- Authentik-Admin, bestätigter Issuer, Public-Client-ID, Gruppen und Gastregel.
- API-Betrieb: exakter Hostname, API-Typ, Modell-IDs, individuelle Rechte/Budgets, Lösch-/Widerrufsweg.
- Datenschutz: lokale Ergebnisse, flüchtige Serverdaten, Abholfach, Betriebslogs, Coolify-/Registry-/Backuparchive. 72 h Abholfach/7 Tage Logs sind weiterhin bestätigungspflichtige Vorschläge, keine pauschale Freigabe.
- Drei Testpersonen, echte Clients und genehmigtes Netzwerk; Lasttest erst danach.

## 3.2 Reicht ein großes Skript?

**Für den fertigen Lebenszyklus später teilweise; für die Erstinbetriebnahme heute nein.** Ein Skript kann Inventar lesen, Compose generieren, einen geprüften Imagekontext exportieren, neue Ablagen erstellen und Offlinevorlagen testen. Es kann weder DNS-Zuständigkeit noch authentik-Policies, persönliche API-Rechte, Datenschutzfreigabe oder eine erfolgreiche Sicherheitsabnahme erzeugen.

Der offizielle Coolify-Installer ist selbst ein großes Rootskript: lädt weitere Dateien, installiert/prüft Pakete und Docker und kann dessen Dienstkonfiguration ändern. Ein einmaliger SHA256 des Einstiegsskripts pinnt **nicht** alle nachgeladenen Komponenten. Für reproduzierbaren Betrieb alle tatsächlichen Versionen/Digests inventarisieren und Releases in einem eigenen Wartungsverfahren testen.

## 3.3 Was der Assistent übernehmen kann

Nach gezieltem Ja: lesendes R2-Inventar, Export/Buildvorbereitung, geprüfte Konfigurationen, Tests, Implementierung auf `cocopi-v3d`, freigegebene SSH-/sudo-Schritte und inhaltsfreie Abnahmeprotokolle. Installation/Neustart/Portöffnung/Datenträgereingriff/echte Modellkosten brauchen jeweils erläuterte Zustimmung. Keine Passwörter/Keys im Chat; lokale sichere Übergabe. Authentik-/API-Adminzugang ist nicht durch vorhandenen SSH-Zugang ersetzt.

**In dieser Arbeit:** neue Dokumente/Vorlagen und Offlineprüfungen; keine Rootinstallation, kein Imagebuild, kein Containerstart, keine Änderung vorhandener R2-Dienste. Die bestehende persönliche pi-hub-Instanz bleibt unangetastet.

<!-- PAGEBREAK -->

# 4 · R2 vorbereiten: Host, Speicher, Netzwerk

## 4.1 Lesendes Inventar

Aus dem entpackten Dokumentpaket:

```bash
bash anhang/01-bestand.sh
cat /etc/os-release
command -v docker || true
docker compose version
# Dockerdaemonzugriff nur mit bereits erlaubtem Zugang:
docker info
```

`docker info` kann administrativen Zugriff benötigen; Dockergruppenmitgliedschaft ist rootähnlich und wird nicht für Studierende eingerichtet. Keine automatische sudo-Eskalation. Das Bestandskript stammt aus der Unixvariante; seine UID-/Slotkontenprüfung dient hier nur dem Konfliktinventar, nicht einer Aufforderung zur Kontenanlage.

Erfassen: Architektur amd64/arm64, CPUs/RAM, freie Blöcke/Inodes, Mounts, bestehende Listener auf 22/80/443/8000/6001/6002 und Loopback 18801–18810, vorhandene Docker-/Proxykonfiguration, bestehende Belegungen. Docker adress pools dürfen nicht mit LAN/VPN kollidieren.

## 4.2 Ressourcen planen

Coolify nennt als Mindestbedarf 2 CPUs, 2 GB RAM und 10 GB Disk für **Coolify selbst**, nicht für zehn Codingarbeitsplätze. Zehn Container mit max. 2560 MiB bedeuten zusammen bis zu 25 GiB RAM plus Host, Coolify, Proxy und Caches. Bei 32 GiB ist wenig Reserve; reale Buildlast messen. Ein Image bauen und zehnmal referenzieren statt zehn paralleler npm-/Compilerbuilds.

Das Pilotimage enthält Node/pi, Python, Git, C/Make und ffmpeg. **Rust ist noch nicht enthalten** und wird als separat geprüfte gepinnte Toolchain ergänzt, bevor Rustprojekte zugelassen werden. Der konkrete pi-Paketstand 1.4.0 verlangt Node **ab 22.19.0**, obwohl der Hub allein Node ≥20 unterstützt. Ein altes Node20-Hubimage reicht daher nicht.

## 4.3 Speicherquoten sind ein eigenes Arbeitspaket

Vorschlag: je flüchtigem Containerhome 80 GB dezimal auf eigenem freigegebenem Datenmount, zusätzlich begrenzte Tmpfs/Logs und Hostreserve für Images/Buildcache/Coolify. Das verändert die bisherige 10+70-GB-Unixaufteilung; Betreiberentscheidung dokumentieren.

Normale Named Volumes und Bindmounts haben **keine Größenbegrenzung**. `mem_limit`, `tmpfs size` oder Coolify-Storageanzeigen begrenzen kein Homevolume. Ohne Usernamespace-Mapping teilen die Pilotcontainer Host-UID1000; Unix-Userquota nach UID trennt sie somit nicht.

Produktionsoption: **Projektquoten pro Homeverzeichnis**, abhängig vom tatsächlichen Filesystem und aktivierter Projektquota-Unterstützung. Alternativ vorab getrennte, sauber gemountete begrenzte Volumes. XFS/EXT4-Konfiguration nicht blind übernehmen. Storageadmin muss pro Slot harte Block-/Inodegrenzen setzen und testen: ein Platz läuft kontrolliert voll, ein anderer kann weiter schreiben. Keine loopback-Dateisysteme oder fstab-Umbauten automatisch erzeugen. Bis zur Abnahme nur kleine synthetische Daten, kein unkontrollierter Modell-/Buildbetrieb.

<!-- PAGEBREAK -->

# 5 · Coolify installieren und absichern

## 5.1 Installationsquelle prüfen

Offizielle Quelle: `https://coolify.io/docs/start-with-self-hosted`, Installer `https://cdn.coollabs.io/coolify/install.sh`. Die Websiteantwort war hier teilweise 403; die zugehörigen offiziellen GitHub-Dokumentquellen wurden gelesen und im Paket als Referenz beigelegt. Nicht als eingefrorene zukünftige Coolify-API verstehen.

Der Hersteller zeigt `curl ... | bash`. Für einen nachvollziehbaren R2-Change stattdessen **herunterladen, prüfen, dann freigeben**:

```bash
mkdir -p "$HOME/coolify-install-review"
cd "$HOME/coolify-install-review"
curl --fail --show-error --location \
  https://cdn.coollabs.io/coolify/install.sh -o install.sh
sha256sum install.sh
bash -n install.sh
less install.sh
```

Prüfen: unterstütztes R2-OS, Zielpfad `/data/coolify`, vorhandene Dockerinstallation, Dockerdaemonänderungen, Portkonflikte, nachgeladene Komponenten, Backup und Wartungszeit. SHA256 als Inventar speichern, nicht als Herstellersignatur ausgeben.

Nach explizitem Ja:

```bash
sudo bash "$HOME/coolify-install-review/install.sh"
```

**Wirkung:** Rootinstallation mit Netzdownloads/Paket-/Docker-/Dienständerungen. Aktuelles Skript kann Docker installieren und neu konfigurieren/neustarten. Auf einem bereits genutzten Negatron besonders sorgfältig vorprüfen. Nicht während fremder laufender Codingarbeit ausführen.

## 5.2 Erste Coolify-Adminanmeldung

Den vom Installer ausgegebenen Zugriff **nur im freigegebenen Adminnetz** verwenden. Erstanmeldung sofort selbst durchführen, bevor Fremde den Account anlegen können. Keine offenen Registrierungsfenster ins Internet stellen. Starkes Adminpasswort, angebotene MFA aktivieren, Recovery und dedizierte Adminzuständigkeit sichern. Dashboard ist nicht das Studierendenportal.

Offizielle Ports: 22 SSH; 80/443 Proxy/ACME; 8000 Dashboard bei direktem Zugriff; 6001 Echtzeit und 6002 Webterminal bei direktem Zugriff. Nur tatsächlich benötigte Quellen zulassen. Nach eingerichtetem Dashboard über HTTPS-Domain können direkte öffentliche 8000/6001/6002-Zugriffe geschlossen werden. SSH erhalten und jede Regel von zweitem Gerät gegenprüfen.

**Docker/UFW-Falle:** veröffentlichte Dockerports können normale UFW-Regeln umgehen. Providerfirewall oder zum Docker-Firewallbackend passendes geprüftes Regelwerk einsetzen; nicht bloß `ufw deny` als Nachweis. Coolify nennt u. a. ufw-docker, dessen Eingriff separat zu reviewen ist. Auch IPv6 prüfen.

## 5.3 Bestehenden Proxy nicht verdrängen

Coolifys Proxy und vorhandener Caddy können nicht beide dieselben Hostports 80/443 belegen. Entscheiden: ein einziger zuständiger Ingress oder separat geplante Proxyverkettung. Bestehende Zertifikate/Domains und persönlichen Hub erhalten. Ein funktionierendes Dashboard sagt nichts über die cocopi-internen Routensperren aus.

Projekt `cocopi-v3d`, Umgebung `pilot` und Zielserver R2 anlegen/validieren. Automatische Deployments/Previewinstanzen für personenbezogene Slotservices deaktivieren. Coolify-Adminzugriffe und Registrycredentials privat sichern.

<!-- PAGEBREAK -->

# 6 · pi/pi-hub-Image einmal bauen

## 6.1 Sicheren Buildkontext erzeugen

Im Paket liegen `anhang/image/Dockerfile` und `entrypoint.sh`. Hubquellen werden **nicht aus einem persönlichen Home kopiert**, sondern aus einem geprüften Commit exportiert:

```bash
bash anhang/05-image-kontext.sh \
  "$HOME/Work/pi-hub" "$HOME/cocopi-v3d-image-kontext"
```

Ziel muss neu sein; getrackte Hubdateien müssen sauber sein. Export erlaubt nur getrackte Runtimebereiche: `server.mjs`, JS-Module aus `lib/`/`empfang/` und `client/`. Keine Sessions, `.env`, persönliche Providerkonfiguration oder Tokens. Exportstand steht in `QUELLSTAND.txt`. Trotzdem manuelle Quellprüfung/Secretprüfung vor Build.

## 6.2 Gepinntes Basisimage wählen

Registryzugriff und Builds erzeugen Netzverkehr und Plattenbedarf, deshalb vorher genehmigen. Ein passendes gepflegtes Node22-Bookworm-Slim-Image prüfen, Runtime ≥22.19.0 bestätigen und dessen echten Digest verwenden:

```bash
# Erst nach Netzwerk-/Buildfreigabe:
docker pull node:22-bookworm-slim
docker image inspect node:22-bookworm-slim \
  --format '{{json .RepoDigests}}'
```

Den ausgegebenen Digest in `NODE_IMAGE` setzen; `sha256:...` ist hier kein kopierbarer echter Digest:

```bash
cd "$HOME/cocopi-v3d-image-kontext"
docker build \
  --build-arg NODE_IMAGE='node:22-bookworm-slim@sha256:BESTAETIGTER_DIGEST' \
  --build-arg PI_VERSION=1.4.0 \
  -t REGISTRY/cocopi-v3d-pilot:RELEASE .
```

APT/npm-Lieferkette bleibt trotz gepinntem Einstieg teilweise zeitabhängig. Für reproduzierbare Produktion Paketlock/Snapshot, Image-SBOM und Schwachstellenprüfung ergänzen. Image signieren bzw. Digests über vertrauenswürdigen Kanal verteilen. Kein Key als Buildarg oder Dockerfile-ENV.

## 6.3 Prüfen, dann Registry

```bash
docker run --rm --entrypoint /usr/local/bin/pi \
  REGISTRY/cocopi-v3d-pilot:RELEASE --version
docker run --rm --entrypoint /usr/local/bin/node \
  REGISTRY/cocopi-v3d-pilot:RELEASE --version
# Nach Freigabe in eigene private Registry pushen:
docker push REGISTRY/cocopi-v3d-pilot:RELEASE
```

Anschließend den **Registry-Manifestdigest** als `PILOT_IMAGE=REGISTRY/...@sha256:...` in Coolify verwenden; lokaler Image-ID-Hash ist nicht automatisch derselbe Registrydigest. amd64 auf R2, ARM für etwaige andere Containerhosts separat bauen. Desktopbinary bleibt davon getrennt.

Image läuft als UID/GID1000 (`node`), Hub auf Containerinterface 0.0.0.0:8787, kein Root. Das ist nur mit isoliertem Netz, nicht veröffentlichten internen Routen und geprüftem Ingress vertretbar. Dockerfile alleine liefert noch keine Empfangsintegration.

<!-- PAGEBREAK -->

# 7 · Private Ablagen und zehn Services erzeugen

## 7.1 Neue Pilotablagen

Vorhandene Pfade nicht überschreiben. Standard ist ein Plan:

```bash
bash anhang/03-pilot-ablagen.sh
```

Nach Storageprüfung und eigenem Ja:

```bash
sudo env COCOPI_CONTAINER_STORAGE_FREIGABE=ja \
  bash anhang/03-pilot-ablagen.sh --ausfuehren
```

Es entstehen zehn getrennte Verzeichnisse unter `/mnt/data/cocopi-container-pilot/slotNN`, darin privates `home` und individueller 64-Zeichen-Hubtoken. **Keine Quoten, keine Container, keine Löschlogik** werden erzeugt. Mountwurzel muss separat gemountet und rootkontrolliert sein. Homes gehören UID1000, Token 0400. Geheimnisse nicht ausgeben, in Git committen oder in Coolify-Screenshots zeigen.

Für regelmäßigen Betrieb Key-/Volumeverwaltung nicht dauerhaft in diesem Pilotschema festschreiben: Belegungsepoche, Widerruf und Löschvertrag aus Kapitel 9 umsetzen.

## 7.2 Compose generieren

```bash
bash anhang/02-compose-erzeugen.sh 10 > compose.pilot.yaml
PILOT_IMAGE='REGISTRY/IMAGE@sha256:BESTAETIGTER_DIGEST' \
  docker compose -f compose.pilot.yaml config --quiet
```

Generator kann 1–10 Services erzeugen. Zehn Services sind bewusst **kein** `--scale 10`: Jeder benötigt eigenes Home, Token, Netz und Lifecycle. Nicht zehn Instanzen auf dasselbe Volume skalieren.

Vorlage: read-only RootFS, CapDrop ALL, no-new-privileges, je 1 CPU/2560 MiB/256 PIDs, 256 MiB `/tmp`, persistentes Home, ausschließlich Hostloopbackports **18801–18810**. Kein Docker-Socket, keine Hostnetzfreigabe. Runtime-Logs standardmäßig deaktiviert; Build-/Coolify-/Proxylogs bleiben separat zu kontrollieren. Bei Supportfehlern temporäre inhaltsfreie Diagnostik genehmigen, nicht einfach alle Prompts loggen.

Eigene Servicenetze reduzieren direkte Nachbarverbindungen. Sie sind nicht `internal: true`, denn pi braucht API-Egress. Damit ist aber kein Egress-/SSRF-Schutz gegen den Host, Metadatenservices oder andere erreichbare Netze garantiert. Bei Coolifyimport prüfen, ob zusätzliche gemeinsame Netze entstehen. Solche Autoänderungen müssen in die Netzwerkabnahme.

<!-- PAGEBREAK -->

# 8 · In Coolify importieren und Infrastrukturpilot starten

## 8.1 Schritt für Schritt im tatsächlichen UI

1. Projekt `cocopi-v3d` → Umgebung `pilot` → **+ New**.
2. Für reproduzierbare Quellen Gitrepository mit **Docker Compose** wählen; alternativ **Docker Compose Empty** zum Einfügen des erzeugten YAML.
3. Server/Ziel R2 wählen. Bei Git den freigegebenen Branch/Commit und Composepfad setzen; nicht automatisch jeden Push deployen.
4. Composeinhalt prüfen. `PILOT_IMAGE` auf geprüftes Registryimage mit Digest setzen; Registryzugriff privat hinterlegen.
5. Bindmountpfade, Rechte, Ressourcen und Ports kontrollieren. **Keine Domain erzeugen, keine zufällige öffentliche URL aktivieren.** Der Infrastrukturpilot bleibt nur über Hostloopback erreichbar.
6. Erst einen Platz bereitstellen/Deploy prüfen. Danach drei, dann nach Lastabnahme alle zehn. Zehn definierte Services müssen nicht alle ungeprüft gleichzeitig starten.
7. Deployment-/Containerstatus und tatsächliche Dockerkonfiguration am Host prüfen. Nicht allein auf ein grünes Coolifylabel vertrauen.

*Bild in der bebilderten Exportfassung: Echtes Coolify-UI: neue Ressource anlegen. Quelle: offizielle Coolify-Dokumentation, kein R2-Livescreenshot.*

*Bild in der bebilderten Exportfassung: Echtes Coolify-UI: Compose-Buildstrategie. Offizielle Dokumentationsaufnahme; installierte Version kann abweichen.*

*Bild in der bebilderten Exportfassung: Echtes Coolify-UI: Branch/Basisverzeichnis/Composepfad. Offizielle Dokumentationsaufnahme.*

## 8.2 Konfiguration am Host prüfen

Den echten Containernamen aus Coolify verwenden; er kann einen generierten Suffix haben:

```bash
docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'
docker inspect --format '{{.Config.User}} {{.HostConfig.ReadonlyRootfs}} {{.HostConfig.Memory}} {{.HostConfig.PidsLimit}}' CONTAINER
docker inspect --format '{{json .HostConfig.PortBindings}}' CONTAINER
docker inspect --format '{{json .NetworkSettings.Networks}}' CONTAINER
```

Nicht pauschal `docker inspect` komplett ausgeben: Umgebungen können Credentials enthalten. Erwartung: UID1000, read-only true, echte Memory/PID-Grenzen, keine extern veröffentlichten 188xx, keine Docker-Sockets/privilegierten Mounts. Speicherquote separat testen.

## 8.3 Betreuter Test über SSH-Tunnel

Auf einem Testlaptop mit ausdrücklich erlaubtem SSH-Zugang:

```bash
ssh -N -L 18801:127.0.0.1:18801 medienhaus@194.95.203.142
```

Nur wenn dies der bestätigte R2-Zugang ist. Danach `http://127.0.0.1:18801` im Browser öffnen; individuellen Pilot-Hubtoken ausschließlich über das private vorgesehene UI übergeben, nicht als URLparameter teilen. Teilnehmer brauchen für den späteren Betrieb keinen Admin-SSH-Zugang. Für den Pilot kann ein Admin einen gesondert begrenzten Zugang bereitstellen; persönliches R2-SSH nicht verteilen.

Der vorhandene allgemeine Hub-Webclient kann hier einen **Legacy-Pilot** ermöglichen. Das ist ausdrücklich **nicht** cocopi-v3-Authentik/automatische Seminarbelegung. Der Readiness-Healthcheck prüft nur authentifiziertes Hub-HTTP, keine Modellberechtigung. Zuerst Mockharness und Session-/RPCfluss prüfen; reale pi-Prompts erst nach APIfreigabe.

`HUB_URL` ist in dieser Pilotvorlage die jeweilige Loopback-Tunneladresse. Keine öffentlichen QR-/Geräteausweise daraus ableiten. Für Produktions-HTTPS und die v1-Belegung neue geprüfte Konfiguration liefern.

<!-- PAGEBREAK -->

# 9 · Vollständige cocopi-v3d-Integration: konkretes Entwicklungspaket

**Dieser Abschnitt ist ein Umsetzungsauftrag mit Abnahmekriterien, kein bereits gelieferter Broker.** Ohne ihn bleibt nur der betreute Infrastruktur-/Legacy-Pilot. Reine Containerreplikation reicht nicht für die verlangten neuen Features.

## 9.1 Interne Kommunikation ersetzen, nicht schwächen

Aktuell prüft Loopback, dass ein Aufruf lokal vom vertrauenswürdigen Dienst kommt. In Docker ist „selbes Netz“ keine ausreichende Identität. Neue interne Dienste brauchen verifizierte Signaturen/mTLS oder gleichwertige starke Dienstauthentisierung, Action-/Slot-/Epochenbindung, Fristen und Replayabwehr. Vorhandene Empfangssignaturen erhalten. Keine frei wählbaren Zielhosts/Ports/Pfade aus Client-JSON.

Schritte:

1. Separate konfigurierte interne Slot-Service-URLs zulassen, streng erlaubnisbasiert und unabhängig von öffentlichen Client-URLs.
2. Interne Belegung/Freigabe/Sync durch neue Dienstauthentisierung absichern; alte Unsigned-/Fremdslot-/Replayaufrufe ablehnen. Nicht `remoteAddress`-Prüfung ersatzlos löschen.
3. Empfangslistener konfigurierbar innerhalb seines Dienstnetzes binden; öffentliche Routen nur durch kontrollierten Ingress erreichbar.
4. Pro Slot isolierte Netze/Sidecars oder gleichwertige Regeln. Empfang darf die Hubs erreichen, Peers dürfen sich nicht gegenseitig als Empfang ausgeben.
5. Gateway vor allen Slotrouten: internen Zugriff extern **immer** blockieren, auch unter Slotpräfixen, mit Normalisierungs-/Encodingtests. Eigene End-to-end-Tests durch den tatsächlichen Coolifyproxy.
6. Gemeinsamen versionierten Containervertrag dokumentieren und mit Offline-/Bridgefixtures testen.

## 9.2 Lifecyclebroker ohne Docker-Socket im Empfang

Coolify behält Deploymenthoheit, der Broker erhält eng begrenzte Aktionen für genau zehn registrierte Slotressourcen. Ein Docker-Socket oder frei nutzbarer Coolify-API-Token im Agenten/Empfang wäre Rootdelegation. Falls die Coolify-API keine genügend feinen Berechtigungen bietet, Broker außerhalb davon mit rootkontrollierter Whitelist betreiben; Credential nur dort, über schmale authentisierte Unixsocket-/mTLS-Schnittstelle. Keine Shellstrings, beliebigen Composepfade, Label-/Volume-IDs oder Container-IDs aus untrusted Requests akzeptieren.

**Checkout-Zustandsautomat:**

```text
BELEGT → ARCHIV BESTÄTIGT → SLOT GESPERRT → ALLE SLOT-PROZESSE GESTOPPT
→ HOME/STATE/CACHES GELEERT → NEUE EPOCHE ATOMAR GESPEICHERT
→ LEERBELEG VOR HUBSTART → NEUER HUB BEREIT → ERST DANN WIEDERVERGABE
```

Auch Logs, Tmpfs, Buildreste, persönliche pi-Auth und gerätebezogener Hubstate berücksichtigen. Rootkontrollierte Epochen liegen außerhalb des Agentenhomes. Brokerjournal enthält nur minimale Metadaten. Alter Client darf neue Epoche nicht erreichen. Crash mitten im Ablauf bleibt gesperrt; Recovery führt nicht zur doppelten Belegung. Kein automatischer Rollback auf altes persönliches Volume.

## 9.3 Coolify und Checkout gegen Rennen schützen

Keine parallelen Rollingreplicas auf einem Home. Pro Slot Lifecyclelock, Deploymentdrain, maximal ein aktiver Container. Coolify-Redeploy darf ein gerade geleertes altes Volume nicht wieder anhängen oder aus Backup restaurieren. Coolify-Webhooks/Auto-Deploys während Belegung deaktivieren. Healthchecks unterscheiden „Prozess gesund“ von „für Wiedervergabe leer“.

`docker compose down -v` kann mehr als den beabsichtigten Platz löschen und bind mounts bleiben trotzdem bestehen. `rm -rf` nach einem untrusted Pfad ist kein sicherer Broker. Nicht im Workshop improvisieren.

## 9.4 Abnahme vor Produktfreigabe

Echte temporäre Volumes/Container in isolierter Umgebung: leeren, abstürzen, Host/Hub/Broker neu starten, Coolifyrecreate auslösen, alte Ausweise replayen, Löschfehler provozieren. Nachweis, dass kein App-/Shellprozess weiter auf alte Dateien zugreift und kein fremder Slot beeinträchtigt wird. Zuerst ein/zwei synthetische Slots, erst dann Freigabe für zehn. Funktions-/Sicherheitsänderungen gehören auf `cocopi-v3d`, nicht als stiller Patch in `cocopi-v3`.

<!-- PAGEBREAK -->

# 10 · Authentik und erste Einwahl der Lehrperson

## 10.1 Welche Anmeldung ist gemeint?

**Coolify-Adminlogin** ist Administration. **UdK-Login in cocopi** ist Person/Rolle für den Seminarpool. **UdK-KI-API-Login** berechtigt zu Modellen. Diese drei Systeme sind getrennt. Coolify hinter Authentik zu schützen verbindet nicht automatisch cocopi mit UdK-Modellen.

## 10.2 Authentik vorbereiten

Mit IAM-Adminfreigabe, am bestätigten zuständigen Authentik:

1. Gruppen `cocopi-seminarleitung` und `cocopi-teilnehmende` administrativ vergeben.
2. Dedizierte Anwendung und OAuth2/OpenID Public Client für cocopi erstellen.
3. Authorization Code + **PKCE S256** erzwingen. Kein Clientsecret im Desktop/QR/Containerfrontend. Kein Implicit Flow.
4. Exakten Issuer einschließlich Slash, öffentliche Client-ID und JWKS mit unterstütztem RS256/ES256 notieren.
5. Aktueller Empfang bewirbt `openid profile groups`; diese Scopes mit minimalem Gruppenmapping konsistent bereitstellen.
6. Callback passend zum **tatsächlichen Code** erlauben, als eng verankerte Regex:

```text
^http://127\.0\.0\.1:[0-9]{1,5}/oidc/callback$
```

Kein `.*`, kein fremder Host/Pfad; Clientport ist vom OS vergeben. Zwei freie Ports positiv testen, falschen Host/Pfad/fehlende PKCE negativ. Browser läuft beim Nutzer, Listener nur dessen Loopback, nicht auf R2.

Minimaler Mappingausdruck, gegen die konkrete Authentikversion prüfen:

```python
return {
    "name": request.user.name,
    "groups": [g.name for g in request.user.ak_groups.all()
               if g.name in ["cocopi-seminarleitung", "cocopi-teilnehmende"]],
}
```

Signiertes ID-Token braucht iss/aud/sub/exp/iat/nonce/name/groups. Gruppen ist Stringarray; kein clientseitig frei erfundenes Rollenfeld. Nichtmitglied ablehnen; bei beiden Gruppen Leitungspriorität explizit abnehmen.

## 10.3 Empfang und öffentliche Vertrauenswurzel

Empfang auf eigenem nichtprivilegiertem Dienst mit privatem persistentem Signierstate betreiben. Signierschlüssel nicht in Images/Agentenhomen/Compose speichern. Separater idempotenter Bootstrap erzeugt den privaten Schlüssel und gibt nur den öffentlichen Pin/Fingerprint aus; **dieser Bootstrap muss noch geliefert werden**. Pin unabhängig bestätigen und in Clients/Slotdiensten konfigurieren. Fehlender Pin wird nicht automatisch aus irgendeinem Server übernommen.

Containerenv enthält tatsächliche Issuer-/Client-ID-/Originwerte und rootkontrollierte Belegungsepochen; für das neue Containerdeployment eigener validierter Vertrag statt Unix-Envdateien unverändert übernehmen. `EMPFANG_TRUST_PROXY=1` nur mit zuverlässiger Proxyheadernormalisierung und Tests; 429/Retry-After nicht umgehen.

## 10.4 Erste Lehrperson: Reihenfolge

Nach bestandener Container-/Empfangsintegration: passendes Desktopbinary installieren, Remote-only beibehalten, freigegebene HTTPS-Adresse und Pin unabhängig bestätigen, UdK-Login im Systembrowser, Leitungsrolle und Containerbelegung prüfen. Ein freier Container wird zugewiesen, keine feste Nummer für die Lehrperson. Erst nach APIkonfiguration eine neue Remote-Coding-Session öffnen. Danach B/C nacheinander anmelden.

*Bild in der bebilderten Exportfassung: Tatsächliches cocopi-Check-in-UI mit synthetischen Daten, kein installierter Dockerpool.*

**Aktiviert durch Authentik:** verifizierte Identität/Rolle, Zulässigkeit zur Poolnutzung und darauf aufbauender Geräteausweis. **Nicht dadurch aktiviert:** Coolifyrechte, eigene Modellkeys, NATdurchdringung, Publishing oder automatische Mitsteuerzustimmung. Authentiklogout ist nicht automatisch Widerruf aller bereits ausgestellten Geräteausweise.

<!-- PAGEBREAK -->

# 11 · UdK-API-Keys und Modellfreischaltung lösen

## 11.1 Domain und Verfahren beim Betreiber bestätigen

`api-udk.digital`, `api.udk.digital` und `api-genki.udk.digital` sind verschiedene Domains. Nicht raten. Die lokale Entwicklerkonfiguration nennt `udk2`, `https://api-genki.udk.digital/v1`, `openai-completions`. Das ist **kein Nachweis**, dass Studierende über diesen Host mit denselben Rechten arbeiten dürfen. Persönlichen Entwicklerkey niemals replizieren.

Vom API-Betrieb benötigt: exakte HTTPS-baseUrl, Protokoll, Modell-IDs, API-seitig erlaubte Modelle pro Person/Projekt, Budgets/Rategrenzen, Laufzeit und Widerruf. Authentik-ID-Token ist kein API-Key. Eine lokale Modellliste ist keine serverseitige Autorisierung.

## 11.2 Betreuter Pilot, heute realistisch

Je Person einen individuellen kurzlebigen befristeten APIzugang anfordern. Admin prüft die aktuelle Zuordnung Person→Container und installiert die Konfiguration **nur dort**. Das beigefügte `04-pilot-modelle-container.sh` läuft als UID1000 und legt nur neue private Modelle/Auth/Settings an. Kein Netzwerkaufruf, kein Key als Prozessargument, keine Überschreibung.

Geprüftes Skript in den zugeordneten Container übertragen; `CONTAINER_B` ist ein Platzhalter für den echten Namen:

```bash
docker cp anhang/04-pilot-modelle-container.sh CONTAINER_B:/tmp/pilot-modelle.sh
docker exec -it -u 1000 \
  -e COCOPI_PILOT_KEY_FREIGABE=ja \
  CONTAINER_B bash /tmp/pilot-modelle.sh
```

Die Person/der verantwortliche Admin gibt baseUrl/Protokoll/Modell-IDs und den geheimen Kurzzeitkey im privaten Terminal ein. Innerhalb von read-only RootFS ist `/tmp` absichtlich schreibbar. Kein dauerhafter Tool-/Rootzugang wird dafür erteilt. Bereits laufende pi-Prozesse ggf. geregelt durch neue Session ersetzen; nicht fremde Arbeit restartieren.

```bash
docker exec -u 1000 CONTAINER_B pi --offline --list-models
```

Listet lokal verfügbare Modelle, beweist noch keine API-Berechtigung. Nach Budgetfreigabe eine echte kurze Anfrage/Toolaufgabe testen. Nicht berechtigtes Modell muss serverseitig scheitern, kein stiller Wechsel auf fremde Zugänge.

**Risiko:** Ein Agent kann seine eigene `auth.json` lesen. Keine privilegierten Langzeitkeys. Vor echten Keys sicherstellen, dass Sync/Checkout/Abholfach/Backups keine Credentials übernehmen; mit eindeutigem synthetischem Fake-Key im Archiv testen. Bis dahin keine echten produktiven Credentials in den neuen Pool. Nach Pilot serverseitig widerrufen, nicht nur Datei löschen.

## 11.3 Lösung für wiederkehrende Seminare: Credentialbroker

Zu implementieren, nicht als bestehendes Feature behaupten:

```text
Authentik sub + belegter Slot + aktuelle Epoche
          ↓ serverseitig autorisierter Broker
kurzlebiger, nur dort nutzbarer APIzugang + erlaubte Modelle + Budget
          ↓ Checkout/Widerruf/Ablauf
Zugang serverseitig unbrauchbar
```

API-Betrieb muss einen passenden Delegations-/Tokenaustauschmechanismus bereitstellen oder individuelle kurzlebige Tokens provisionieren. Kein erfundenes OIDC→API-Exchange-Endpoint. Brokercredential niemals in zehn Images replizieren. Bindung an stabile `sub`, nicht an Anzeigename; keine Tokenzuordnung nur über Containername. Fehler/Widerruf/Replay/Slotwechsel und fehlende Modellfreigaben negativ testen.

Für den Drei-Personen-Pilot ist die manuelle kurzlebige Variante vertretbar nach Freigabe. Für unbeaufsichtigte regelmäßige Seminare erst den Broker oder ein gleichwertig überprüftes Betriebsverfahren abnehmen.

<!-- PAGEBREAK -->

# 12 · P2P und QR: was Docker verändert und was nicht

## 12.1 P2P bleibt zwischen den Clients

Der Desktop nutzt direkten iroh-QUIC-Transport ohne öffentliche Relay-/Discoverydefaults. Empfang liefert signierte Peer-/Adressmetadaten, aber kein Relay. Ein Dockerhub im Internet macht zwei Laptops hinter NAT nicht automatisch gegenseitig erreichbar.

Erster Test: drei reale Laptops im gleichen freigegebenen LAN, keine WLAN-Clientisolation, direktes UDP erlaubt. Alternativ geroutetes VPN **zwischen allen Clients** mit tatsächlich bekanntgegebenen erreichbaren Adressen. Tailscale nur auf R2 genügt nicht. Dynamische Peerports messen und gezielt freigeben; keine Firewallabschaltung. Container-Netzwerke und Coolifyproxy betreffen die Hubverbindung, nicht automatisch das Peer-UDP.

Nach verifiziertem Check-in Seminar bewusst verbinden; Ausweis/Netzschlüssel und signiertes Verzeichnis prüfen. Bis zu acht direkte Adressen je Peer, keine DNS-/öffentliche Discoveryabkürzung. Sessionankündigungen enthalten nur begrenzte Metadaten: höchstens 32, Labels bis 120 Zeichen, Aktualisierung bei Änderung/periodisch. Kein automatisches Teilen des privaten Agentenchats.

*Bild in der bebilderten Exportfassung: Tatsächliche Seminaroberfläche mit synthetischen Peers.*

## 12.2 QR-Codes sind keine Generalschlüssel

**Gerätekopplung:** Desktop erzeugt einen kurzlebigen Einmalcode, eigenes Handy scannt ihn, prüft die richtige HTTPS-Zieladresse und erhält eigene widerrufbare Geräteberechtigung. Kein API-Key, kein Root-/SSH-/Coolifyzugang im QR. Ein QR mit Pilot-Loopbackadresse funktioniert auf einem externen Handy nicht; dafür braucht man die richtige freigegebene HTTPS-Origin.

**Identitätsabgleich:** Peeridentität und vollständigen Schlüssel unabhängig bestätigen. Ein etwaiger Kontakt-QR kann Adresse/Schlüssel übertragen, ersetzt aber nicht die bewusste Bestätigung. Ein QR löst weder Authentikrollen noch NAT noch Modellfreischaltung.

**Nicht teilen:** laufende Kopplungs-QRs in Kursfolien/Chat. Illustrationen nur mit synthetischen abgelaufenen Codes. Nach Verlust Handygerät widerrufen; Desktop darf weiterlaufen, wenn dessen eigene Berechtigung gültig bleibt.

*Bild in der bebilderten Exportfassung: Tatsächliches QR-Kopplungs-UI, synthetischer Code; nicht scannen.*

## 12.3 Zustimmung und Grenzen

DM, Seminarchat, Metadaten und Mitsteuern separat testen. Keine automatische Zustimmung durch Leitungsrolle. Mitsteuern anfragen, ablehnen, neu anfragen, bewusst annehmen, beenden. Empfangene Texte sind Daten, keine Agentenaufträge. Sessionnamen keine geheimen Pfade/Keys enthalten lassen.

*Bild in der bebilderten Exportfassung: Tatsächliche DM-Oberfläche mit synthetischen Daten.*

*Bild in der bebilderten Exportfassung: Tatsächliche Mitsteueranfrage, synthetische Szene.*

<!-- PAGEBREAK -->

# 13 · Drei reale Personen: vom Infrastrukturtest zur Produktabnahme

## 13.1 Zwei sauber getrennte Testarten

**A · Infrastruktur-/Legacy-Pilot:** Drei getrennte pi-Hubcontainer, individuelle Pilotkeys und kontrollierte Tunnel; jeder führt eine Codingaufgabe aus. Prüft Container/Modelle/Toolausführung. **Beweist keine** Authentikslotzuteilung, cocopi-Seminarintegration oder automatische Löschung.

**B · Vollständiger cocopi-v3d-Lauf:** Erst nach Kapitel 9–11 samt Offline-/Bridgetests und Sicherheitsabnahme. Drei Desktopclients loggen sich mit eigenen UdK-Identitäten ein, bekommen eigene Container, nutzen ihre freigeschalteten Modelle und P2P, dann vollständiger Checkout. Dieser zweite Lauf ist die Freigabegrundlage für das Achterseminar.

Keine grüne Gesamtabnahme aus Testart A ableiten.

## 13.2 Konkrete kleine Codingaufgabe

Jede Person in ihrem eigenen leeren Testprojekt, jeweils bewusst ein freigeschaltetes Modell wählen und nach Kostenfreigabe senden:

```text
Arbeite nur in meinem aktuellen Testprojekt auf R2.
Erzeuge mit Python-Standardbibliothek eine statische HTML-Seite
mit drei farbigen geometrischen Formen und drei eindeutigen Namen.
Schreibe einen automatisierten Test für die Namen und führe ihn aus.
Installiere nichts, starte keinen öffentlichen Server,
veröffentliche nichts und lies keine Zugangsdaten aus.
Nenne die erzeugten Dateien und das tatsächliche Testergebnis.
```

Abnahme: Modellantwort **plus** wirkliche Toolausführung im richtigen Container, erzeugte Dateien, bestandener Test und eigener Modellzugang. Eine bloße Textbehauptung reicht nicht. Kein pauschales `docker exec env` zur Diagnose, keine Secretscreenshot.

*Bild in der bebilderten Exportfassung: Tatsächlicher Remote-Hauptchat, synthetische UI-Szene.*

## 13.3 Abnahmematrix für den vollständigen Lauf B

| ID | Aktion | Erwartung / Beleg |
|---|---|---|
| D01 | A Leitung, B/C Teilnahme einloggen | drei korrekte Rollen und unterschiedliche Belegungen |
| D02 | Nichtmitglied, falsches Token/Nonce | kein Zugriff |
| D03 | je eigenes freigeschaltetes Modell nutzen | API-seitige Berechtigung und Toolausführung |
| D04 | fremden Slot/alte Epoche aufrufen | abgewiesen, kein fremder Inhalt |
| D05 | eigenes Home füllen/Last begrenzen | Quote/CPU/RAM/PIDs greifen, andere Slots bleiben arbeitsfähig |
| D06 | Seminarchat/DM/Sessionankündigung | gegenseitig sichtbar, private DM nicht Gruppennachricht |
| D07 | Mitsteuern ablehnen/annehmen/beenden | kein Zugriff ohne Zustimmung, Ende wirksam |
| D08 | eigenes Handy koppeln/widerrufen | eigener Gerätezugang, Widerruf wirksam |
| D09 | Desktop trennen, Handy aktiv | Aktivität bleibt der richtigen Belegung zugeordnet |
| D10 | WLAN kurz aus/erneut verbinden | nachvollziehbarer Reconnect, keine blinden Doppelprompts |
| D11 | Checkout mit Ergebnisübernahme | lokal korrekt, echtes Stoppen/Leeren/neue Epoche |
| D12 | Fehler beim Leeren/Volume/API | Slot bleibt gesperrt, kein Datenverlust durch Freigabe |
| D13 | Coolifyredeploy/Brokercrash im Test | kein doppelter Container, kein altes Home wieder angebunden |
| D14 | gleiche Slot-ID neu vergeben | keine alten Dateien/Sessions/Keys/Ausweise nutzbar |
| D15 | Proxyangriffe direkt/Slotpräfixe | interne Routen bleiben 403, auch Encoding/Forwarded-Spoof |

A ist Lehrperson, nicht automatisch API-/Coolifyadministrator. B/C eigene Menschen mit eigenen Clients. Log: Pseudonym, Version/Digest, Zeit/Fall/Ergebnis, minimale Belege; keine Tokens/Prompts/Keywerte.

## 13.4 Langzeittests nicht verkürzen

15 Minuten Handyaktivität beweisen keinen 8-Stunden-Idlefall. Dafür eigener Zeit-/Mocktest plus geplanter echter Langzeittest. Mobile gültige SSE/Anfragen verhindern Idlezwangscheckout; Tagesende bleibt hart. Hubneustart setzt RAM-Aktivität zurück bis zur neuen Anfrage/SSE. Nicht mit echten Projekten einen Zwangscheckout provozieren.

Pickup-/Archivdownload: Datenfluss darf nicht an einem kurzen Gesamttimeout scheitern; 45 Sekunden ohne Daten sollen abbrechen. Reverseproxy kann zusätzlich abbrechen; Produktionspfad testen. Große Archive/Overlimit zuerst synthetisch. Keine dauernde Wiederholung teurer Prompts bei unklarer Versandlage.

## 13.5 Abschluss

Ausgewählte Ergebnisse lokal übernehmen und Prüfsummen/Inhalte bestätigen. Erst danach Checkout, Brokerleeren und neue Epoche. Kurze API-Pilotkeys serverseitig widerrufen. Neue Testbelegung darf kein altes `auth.json`, Projekt, Hubgerät oder Session sehen. Archive/Logs/Backups nach bestätigter Policy löschen. Scheitert D04/D11/D12/D14/D15: sofort Stopp, kein Achterseminar.

*Bild in der bebilderten Exportfassung: Tatsächliche Checkout-Vorschau mit synthetischen Daten.*

<!-- PAGEBREAK -->

# 14 · Seminar mit acht Personen starten

## 14.1 Freigabegate

- Zehn tatsächlich abgenommene Containerplätze, je eigenes Home/Token/Quote/Netz/Lifecycle.
- Vollständiger Drei-Personen-Lauf B bestanden; offene Abweichungen dokumentiert und verantwortet.
- Acht insgesamt oder acht Studierende plus Lehrperson? Im zweiten Fall neun belegte Container nötig.
- Identitäten, API-Budgets und Modelle vorbereitet. Ohne Broker bleibt die manuelle Einrichtung pro Person notwendig; ausreichend Zeit und Admin dafür einplanen.
- Alle Desktopclients passend zur Plattform gebaut. Kein R2-x86-Desktopbinary in ARM-VM kopieren.
- Seminar-LAN/VPN am echten Ort geprüft; Dashboard nicht für Studierende geöffnet.
- Automatische Coolifyredeploys aus, keine parallelen Imagesbuilds im Unterricht, geregelter Notfallkontakt.
- Tagesende/Zeitzone, Abholfachfristen und lokale Speicherung erklärt.

## 14.2 Vorschlag für 120 Minuten

| Zeit | Ablauf |
|---|---|
| 0–15 | zeitversetzter Login in kleinen Gruppen, Rolle/Container/Modell prüfen |
| 15–25 | kleiner Coding-/Tooltest je Person |
| 25–45 | individuelle kreative Arbeit, schwere Builds nacheinander |
| 45–60 | Chat, Sessionankündigungen, freiwillige DM/Mitsteuerung |
| 60–90 | Projektarbeit und bewusstes Ergebnisteilen |
| 90–110 | lokale Ergebnissicherung mit Vorschau/Prüfung |
| 110–120 | geregelter Checkout aller Personen, Leerbelege/Widerruf |

Nonce/Check-in-Limits: Kapazität zehn pro IP, Nachfüllen ein Token pro sechs Sekunden; 429 mit Retry-After beachten. Hinter gemeinsamem Seminar-NAT Loginwellen entzerren, Proxytrust korrekt einstellen, Limits nicht abschalten.

## 14.3 Beobachtung und Notfall

```bash
docker stats --no-stream
free -h
df -h / /mnt/data
```

Nur inhaltsfreie Metriken erfassen; Coolify-Systemressourcen und Platten-/Inodegrenzen mitbeobachten. CPUlimit je Container ist keine globale Schedulerwarteschlange. Echte parallele Pythonaufgaben können problemlos sein, acht schwere Rust-/Videobuilds trotzdem nicht. Kapazität messen statt Limits spontan entfernen.

Bei Cross-Slot-Zugriff, Leak, fehlgeschlagenem Leeren, falscher Rolle oder unkontrollierten APIkosten: neue Einwahlen stoppen, betroffene Berechtigungen widerrufen, gezielte Diagnose im Wartungsverfahren. Keine Container-/Volume-Gesamtlöschung. Kein gemeinsamer Adminkey als Notlösung. Rückfall auf lokale Arbeit oder betreute Vorführung, nicht unsichere Notöffnung.

<!-- PAGEBREAK -->

# 15 · Anhänge, Tests und ehrlicher Lieferstand

## 15.1 Neue Quellanhänge

Im Paket unter `anhang/`, im Code auf `cocopi-v3d` unter `deploy/docker-coolify/`:

| Datei | Zweck | Wirkung |
|---|---|---|
| `01-bestand.sh` | Hostinventar | nur lesen, kein Netzwerk/sudo |
| `02-compose-erzeugen.sh` | 1–10 individuelle Services | nur YAML auf stdout |
| `03-pilot-ablagen.sh` | privater neuer Datenbaum/Hubtoken | standardmäßig Plan; Apply separat Root/freigegeben |
| `04-pilot-modelle-container.sh` | individueller Kurzzeit-Modellzugang | neue private Dateien als UID1000, kein APIaufruf |
| `05-image-kontext.sh` | geprüfte Hubruntime exportieren | nur neues Buildverzeichnis |
| `image/Dockerfile` | nichtprivilegiertes pi/Hub-Pilotimage | Build braucht explizite Onlinefreigabe |
| `image/entrypoint.sh` | privater Hubtoken/start | kein Key im Image |
| `test_vorlagen.py` | Syntax/Negativpfade/Composeparser | offline, kein Dockerdaemon nötig |
| `README.md` | Einsatzgrenzen | kein Betriebsfreigabedokument |

Offline selbst prüfen:

```bash
python3 anhang/test_vorlagen.py
```

Voraussetzungen Python3, Bash und Docker-Compose-CLI. Dockerdaemon oder Root wird für diese Tests nicht benötigt. Sechs Tests decken Bashsyntax, Anzahlvalidierung, zehn getrennte Servicedefinitionen, Dry-Run, Freigabesicherung und Composeparser ab. Das ersetzt keinen Imagebuild, Bridge-/Quoten-/Coolify-/Sicherheits-/Handytest.

## 15.2 Tatsächliche Prüfung dieser Lieferung

Die neuen Vorlagen wurden offline geprüft; sechs Tests bestanden. Ein sauberer Hubruntimekontext wurde exportiert. Registrymetadaten bestätigen pi-Paket `@earendil-works/pi-coding-agent` 1.4.0 mit Nodeanforderung ≥22.19.0. Offizielle Coolifydokumentquellen/Installer wurden lesend geprüft, nicht ausgeführt. Keine Containerservices oder neuen Rootpfade auf R2 installiert. Dokumentationsbilder sind ausdrücklich gekennzeichnet.

**Offen:** Imagebuild-/Runtimeprüfung, zehn gestartete Coolifyservices, sichere Storagequoten, containerfähiger Empfang/Broker, OIDC-/APIintegration und echte Mehrpersonenabnahme. Das Schlussziel des vollständigen Seminarstarts ist erst nach Umsetzung/Abnahme der Kapitel 9–13 erreicht. Dieser offene Stand darf nicht durch eine Erfolgsmeldung ersetzt werden.

## 15.3 Empfohlene nächste Arbeitsaufträge

1. R2-Inventar und Coolify-/Docker-/Storageinstallation freigeben, Konflikte mit vorhandenen Diensten prüfen.
2. Image bauen/scannen, ein dann drei Container mit Mocks und kurzlebigen individuellen Pilotzugängen abnehmen; auf zehn vorbereiten.
3. Containervertrag/Lifecyclebroker/Empfangsnetz auf `cocopi-v3d` implementieren und unabhängig sicherheitlich prüfen.
4. Authentikadmin und APIbetrieb einbinden; Fake-Key-Sync-/Archivtest, kurzlebige Credentials/Broker abnehmen.
5. Vollständiger Drei-Personen-Lauf B, danach Achterseminar.

# 16 · Quellen und Bildnachweise

- Coolify Self-hosting: `https://coolify.io/docs/start-with-self-hosted`
- Coolify Compose: `https://coolify.io/docs/applications/builds/docker-compose`
- Coolify Firewall: `https://coolify.io/docs/core/infrastructure/servers/firewall`
- Offizielle Dokumentquellen: `https://github.com/coollabsio/coolify-docs` (gelesene MDX-Dateien in `quellen/`). Websitepfade/GUI können sich mit Releases ändern.
- Coolify-UI-Bilder: offizielles Repo, `public/images/builds/packs/compose/{1,4,5}.webp`; als Referenz unverändert übernommen. Rechte liegen bei den jeweiligen Projekturhebern.
- Docker Engine Ubuntu: `https://docs.docker.com/engine/install/ubuntu/`, insbesondere Firewall-/Paketkonflikthinweise. R2-OS zuerst prüfen; keine Ubuntuinstallation auf anderem OS erzwingen.
- pi 1.4.0: installierte Dokumentation `models.md`, `models-configuration.md`, `configuration.md`, `cli.md`; konkrete Provider-/Credentialkonfiguration vor Betrieb nochmals gegen gepinnte Version prüfen.
- cocopi-Quellbefunde: `pi-hub/{server.mjs,empfang/empfang.mjs,lib/hub-v1.mjs,lib/sync-v1.mjs}`, `pi-lite/copi/crates/pi-backend/src/oidc.rs` und bisheriges `deploy/r2/`.
- cocopi-Abbildungen: bestehende tatsächliche UI-Renderings mit synthetischen Testdaten aus dem v3-Tutorial; keine erfundenen Coolify-/R2-Livefenster.

**Dateiname:** `cocopi-v3d-Sysadmin-Docker-Coolify`. Die vorherigen Sysadmin-/Tutorialdateien bleiben unverändert erhalten.
