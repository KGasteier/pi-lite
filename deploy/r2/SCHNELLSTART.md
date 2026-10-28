# R2-Pool: Schnellstart für die Administration

**Vorlage, keine Betriebsfreigabe.** Zuerst nur `account01,account02`, keine echten
Arbeiten oder Modellaufrufe. pi kann mit [`EINRICHTEN-MIT-PI.md`](EINRICHTEN-MIT-PI.md)
durch die Einrichtung führen und fragt vor jedem sudo nach einem eigenen Ja.

## Voraussetzungen

- Geprüfte zusammenpassende S3/S7/S4-Commits samt N3-Epochen- und N5-Proxytests;
  D-R1 und Poolabnahme dokumentiert. Ungeprüftes bleibt gesperrt.
- Gemeinsame Werkzeuge Node, pi, Rust, Python, Git, ffmpeg, C/Make; rootverwalteter
  Hub unter `/opt/cocopi/pi-hub`, Einstieg **`empfang/empfang.mjs`**. Keine
  persönlichen Tool-Shims oder Providerdaten kopieren.
- ext4-Userquoten auf `/` und eigener SSD `/mnt/data` bereits aktiv; cgroup v2.
  Konten-/UID-Konflikte und alte User-Hubs zuvor klären. Hier keine Paket-, Mount-
  oder Netzwerkänderungen vornehmen.
- Rootverwaltete Skriptkopie `/usr/local/lib/cocopi-r2`, Hubvertragsdatei und
  bestätigter öffentlicher Empfangsschlüssel vorbereitet (Bootstrap separat).
  [`AUTHENTIK.md`](AUTHENTIK.md): Public Client, PKCE S256, Rollen, Callback;
  `/etc/cocopi/empfang.env` root:root 0600 mit den exakten Beispielnamen.
  `EMPFANG_EPOCHEN_DIR=/var/lib/cocopi/slots`,
  `HUB_EPOCHE_DATEI=/var/lib/cocopi/slots/accountNN.epoche`.
- TLS-Proxy separat freigegeben und validiert: Caddy direkt oder Tailscale vor
  lokalem Caddy (8799), niemals direkt auf 8800/Slotports. Kein Funnel.

## Sechs Befehle, einzeln ausführen

Aus dem geprüften pi-lite-Repository. **1–3 sind lokal, ohne sudo und Netzwerk.**
Vor **5 und 6** Wirkung erklären und jeweils ein neues ausdrückliches Ja einholen.
`PUBKEY` und `NAME` vorher durch bestätigte öffentliche Werte ersetzen.

```sh
# 1 · Regression (nur Temp-Dateien und Mocks)
python3 deploy/r2/test_scripts.py
# 2 · Vollständigen Einrichtungsplan ansehen
bash deploy/r2/einrichten.sh --dry-run --slots account01,account02
# 3 · Betriebsprüfung als Vorschau
bash deploy/r2/admin-check.sh --dry-run --slots account01,account02
# 4 · Den Plan der unveränderlichen, rootverwalteten Kopie erneut ansehen
bash /usr/local/lib/cocopi-r2/einrichten.sh --dry-run --slots account01,account02
# 5 · Nur nach Abnahme dieses ganzen Plans: Konten, Quoten, Units und Starts
sudo env COCOPI_D_R1=freigegeben COCOPI_POOL_ABNAHME=bestaetigt HUB_EMPFANG_PUBKEY=PUBKEY bash /usr/local/lib/cocopi-r2/einrichten.sh --ausfuehren --slots account01,account02
# 6 · Nur lesen + Diagnoseanfragen an den freigegebenen HTTPS-Proxy
sudo bash /usr/local/lib/cocopi-r2/admin-check.sh --ausfuehren --slots account01,account02 --proxy-url https://NAME
```

## Prüfung und Stoppregel

Exitcode 0 allein ersetzt keine Abnahme: Quoten, aktuelle inhaltsfreie Poolanzeige,
Leerbeleg **vor** Hubstart, lesbare Epoche je Platz und Empfang müssen belegt sein.
N5 verlangt 403 für Belegung, Freigabe und `sync/dienst/*`, direkt und mit allen
zehn Slotpräfixen. Positiver Check-in, Sync/Archiv-Rückgabe, Leeren und Replay mit
alter Epoche sind separat mit synthetischen Daten nachzuweisen. Zwei Plätze sind
keine Zehnerpoolabnahme; die Poolanzeige kann deshalb bewusst noch Exit 1 melden.
Bei „nicht geprüft“, fehlender Epoche, gesperrtem Platz oder Fehler **stoppen**.
Keine Zähler zurücksetzen, keine Flags als Ersatz für Nachweise setzen, nichts
ungefragt löschen oder neu starten. Grenzen und nächste Schritte dokumentieren.
