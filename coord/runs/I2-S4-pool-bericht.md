# I2-S4-pool · Abschlussbericht

Stand: 28.10.2026, ca. 22:30 CET · Worktree `/home/medienhaus/Work/pi-lite-wt/s4`,
Branch `v3/s4`, HEAD `e219f59`. Eigener Git-Arbeitsbereich sauber. Änderungen
nur unter `deploy/r2/**`; dieser Bericht und Heartbeats ausschließlich an den
beauftragten Koordinationspfaden. Kein Push/Merge/Stash/Reset.

## Kurzurteil

**Lokales Deploymentpaket geliefert, Pflichtbefehl 42 Tests grün. Kein Deployment
und keine Produktiv-/Sicherheitsfreigabe.** Echte S3/S7-Integration, authentik,
sudoers/Caddy, Quoten und Systembetrieb bleiben separat abzunehmen. Einrichtung
ist absichtlich gegen unbestätigte Integration gesperrt.

## Commits (nach jeweils grünem Testlauf)

1. `0b7e75e` — `cocopi v3 I2-S4-pool: Löschhelfer mit isolierten Angriffstests ergänzen`
2. `a847369` — `cocopi v3 I2-S4-pool: Systemdienste, Einrichtungsplan und sichere Betriebsprüfung ergänzen`
3. `e219f59` — `cocopi v3 I2-S4-pool: Leerbelege und kleine Quotenreste zusätzlich absichern`

## Geliefert

- `systemd/cocopi-hub@.service`: Systemdienst `User=%i`, eigene
  `cocopi-%i.slice`, `/etc/cocopi/hub-%i.env`, privates Home/Register,
  KillMode control-group, PrivateTmp, keine ungefilterten Journalausgaben.
- `systemd/cocopi-empfang.service`: Systemkonto `cocopi-empfang`,
  `/var/lib/cocopi-empfang` 0700, tatsächlicher S7-Einstieg
  `/opt/cocopi/pi-hub/empfang/empfang.mjs`, privates EnvironmentFile.
- Globale `cocopi.slice` und zehn aus Vorlage installierte Account-Slices;
  je Platz 100 % CPU, 2G high / 2560M max, 256 Tasks. Keine gemessene Lastzusage.
- `cocopi-slotctl` ausführbar, Python-stdlib, `leeren|status|pruefen`, exakte
  Regex `^account(0[1-9]|10)$`; Standardleeren nur Vorschau. Fake-Root
  `COCOPI_ROOT` ausschließlich unprivilegiert, als Root abgewiesen.
- fd-relative O_NOFOLLOW-Traversierung, geprüfte feste UIDs/GIDs und Inventare,
  keine frei wählbaren Pfade/UIDs; innere Symlinks nur selbst entfernt,
  Wurzel-/Elternlinks abgewiesen, Mounts inklusive Bindmounts blockiert.
- Tatsächliches Leeren stoppt System-Hub/Usermanager, sperrt Wurzeln, beendet
  UID-Prozesse und verweigert bei Resten; kein Prozessargument-/Inhaltslogging.
  Leert `/home/accountNN` und `/mnt/data/accountNN`, spielt geprüftes Skelett
  ein, prüft Trees/Hashes und beide Quoten. Gesamtbytes ≤1 MiB für Skelett und
  exakte UID-Inodezahlen erkennen auch kleine externe Restdateien. Bekannter
  Legacy-Token ist genau eine erlaubte externe Inode (S3-Bereinigung offen).
- Rootprivater atomarer Leerbeleg mit Slot/Zeit/Dauer/Zählern, kein Inhalt,
  **vor** Hubstart. Frühere Belege werden vor neuem Löschversuch entfernt.
  Erstprüfung `--ausfuehren pruefen` schreibt Beleg ohne Löschung; lesendes
  `pruefen` schreibt nichts. Wiederholungen/Sperrdatei getestet.
- `sudoers/cocopi-slotctl`: ausschließlich cocopi-empfang, root, 31 exakt
  enumerierte Befehle, kein Argumentwildcard, NOSETENV, kein Shellrecht;
  `slotctl-empfang` normalisiert S7-Reihenfolge zu engem `sudo -n`.
- `einrichten.sh`: acht nummerierte Schritte, Standard Dry-Run,
  `--ausfuehren`, zusätzliche D-R1-/Poolabnahmesperren; zwei Slots zuerst.
  Keine Paket-/Mount-/Netzänderungen automatisiert, kein implizites Leeren.
- `provision.sh`, `common.sh`, `check.sh`, `hub-launch.sh`, `slots.conf` auf
  System-Hubs/neue Pfade/Empfangspubkey und `HUB_ROOTS` abgestimmt. Bestehende
  I1-Datenpfade/Linger/Userdienste blockieren statt automatischer Migration.
  Alte Root-Level-Userunit deaktiviert; I1-Rückbau mit dauerhaften Archiven
  für echte Ausführung gesperrt, da nicht E9-konform.
- `proxy/`: Caddy-TLS und alternativ Tailscale-TLS vor lokalem Caddy8799;
  niemals direkt Empfang/Slotports an `tailscale serve`, kein Funnel.
  Erlaubt `/v1/empfang/*`, `/slot/accountNN/v1/*`, sperrt Belegung/Freigabe
  direkt und unter Slots, zusätzlich `/v1/sync/dienst/*`. Abwehr von
  Encoding/Traversal/Doppelslash; sonst 404, kein Access-Log, SSE-Flush.
- `admin-check.sh`, `pool-read.py`, `proxy-check.py`: inhaltsfreie freie/belegte/
  gesperrte Plätze, Leerbelege, zwei Quoten, Systemdienst/PID/Loopback,
  inhaltsfreies Empfang-hallo, 71 HTTPS-Sperrprüfungen. Ohne aktuelle sichere
  Poolprojektion oder echten 403-Befund Exit 1 statt grüner Behauptung.
- `AUTHENTIK.md`: Public OIDC, Code+PKCE S256, Systembrowser, beliebiger
  Loopback-Port mit enger Redirectregex, `openid profile`, Gruppenmapping,
  Rollen `cocopi-seminarleitung`/`cocopi-teilnehmende`, private Env-Datei und
  Schlüsselbootstrap, Negativabnahme.
- `EINRICHTEN-MIT-PI.md`: kopierbarer deutscher interaktiver Prompt;
  zeigt jeden Befehl, neues ja vor jedem sudo, beginnt mit Dry-Run, endet
  mit sichtbarem admin-check. Keine heimliche Sammelfreigabe.
- `DATENSCHUTZ.md`: einfache Sprache, kein dauerhafter Serverspeicher,
  Checkout-/Zwangscheckout-Löschung, verschlüsseltes Abholfach ≤72h,
  Logs ≤7 Tage, Kontakt-/Rechtsgrundlagenplatzhalter und echte Grenzen.
- `README.md` vollständig auf Poolstand, Verträge und Nichtfreigabe aktualisiert;
  historische BEFUND/DRYRUN/TESTS-Dateien nicht als I2-Nachweis ausgegeben.

## Nachweise

- `python3 deploy/r2/test_scripts.py` zuletzt **42 bestanden, keine Skips**,
  ca. 4 Sekunden. Darunter 14 Helfer-/Fake-Root-Tests: Traversal/ungültige Slots,
  Links nach `/etc`, Wurzel-/Elternlinks, Prozessreste, Skelettlinks/-hardlinks,
  FIFO ohne Hängen, Quotenreste, kleine externe Inodes, parallele Sperre,
  Belegrechte/Inhalt/Fehlversuch, explizite Erstprüfung und Wiederholung.
- Alle Shellvorlagen inkl. Proxy-Skript via `bash -n` im Pflichtlauf.
- Dry-Run-Mutationssentinel für Konten/Quoten/Dienste/Dateiänderungen; keine
  schreibenden Systembefehle ausgeführt. Admin-/Proxy-/Quota-/Slice-Fixtures
  ausdrücklich keine Liveprüfungen. Proxy-Tests mocken curl vollständig.
- `git diff --check` grün; keine neuen Abhängigkeiten/Builds.
- `systemd-analyze verify --man=no --generators=no` auf Originalen meldete
  erwartbar nicht installierten Hublauncher (Exit1). Zusätzliche Tempkopien,
  ausschließlich Hub-ExecStart durch `/usr/bin/true` ersetzt: **Exit0**, Warnung
  lokales systemd habe CPUAccounting entfernt/ignoriere es. Nichts gestartet.
- Caddy und visudo lokal nicht vorhanden: deren echte Syntaxvalidierung fehlt.

## Verbindliche Änderungsbitten S7 / K (Integrationsblocker)

1. **Rootaufruf + Leerbeleg:** S7 `runSlotctl` ruft momentan als Systemkonto den
   Helper direkt auf; `EMPFANG_SLOTCTL=/usr/local/lib/cocopi-r2/slotctl-empfang`
   muss den engen sudo-Weg nutzen. S7 muss Exit0 und JSON `slot`,
   `zustand:leer`, `nachweis:vor_hubstart` prüfen. Aktuelles zweites Live-
   `pruefen` nach `leeren` ist unmöglich, da der Helfer bereits den Hub neu
   gestartet hat und der dann Prozesse/Registry erzeugt. **Nicht Prüfung/Fehler
   ignorieren**, sondern den atomaren Nachweis vor Start verbindlich übernehmen.
2. **Inhaltsfreie Adminquelle:** S7 soll atomar alle ≤120s
   `/var/lib/cocopi-empfang/pool-status.json` erzeugen, owner Empfang, 0600:
   `{protokoll:1,zeit:<ISO8601 UTC>,slots:[{slot:"account01",zustand:"frei"},…]}`.
   Genau zehn eindeutige Slots, Zustände `frei|belegt|leeren|gesperrt`, keine
   Zusatz-/Personenfelder. Fehlend/veraltet => Admin unbekannt/Exit1. Keine
   privaten Empfangspooldateien als Ersatz lesen. Zwei-Slot-Test ist noch
   keine vollständige Poolabnahme.
3. **Archiv-/Signaturpfad:** Zwischenzeitlich wurde S7s signierter Archivadapter
   lesend gesehen. S4 hat ihn NICHT Ende-zu-Ende getestet. S3/S7 müssen
   Belegung/Freigabe, Sync-Dienstaufträge, Replay/Idempotenz, Erstinitialisierung
   und vollständigen Checkout gemeinsam nachweisen. Bericht S6 ebenfalls
   beachten; fremde Dateien wurden nicht geändert.
4. **OIDC-Vertrag:** S7 hallo meldet derzeit zusätzlich `groups`; Auftrag fordert
   `openid profile`. Mit S1/S7 Gruppenclaim in profile oder dedizierten
   expliziten Scope und tatsächlichen Callback-Pfad gemeinsam abstimmen.
5. **Betriebsabnahme:** echte sudoers/Caddy-Validierung, Proxy-403 und positive
   Empfang/Slot/SSE-Routen, Quota-Inodes inkl. `/tmp`/offenen Dateien, Prozess-
   Wiederanlauf, Schlüsselbootstrap und Löschfristen auch ohne Traffic prüfen.
   Empfang braucht sudo, deshalb absichtlich kein NoNewPrivileges und kein
   geerbtes ProtectHome=read-only; eigenes Code-/Privilegienreview erforderlich.

Bis dahin bleiben `HUB_CONTRACT=unconfirmed` und Einrichtungssperren bestehen.
`COCOPI_POOL_ABNAHME=bestaetigt` niemals als Ersatz für die Nachweise setzen.

## Nicht getan / Risiken

Kein R2-Deployment, keine Installation, kein Konto/Quota/Mount/Dienst/Netz geändert,
kein authentik-Zugriff, Internet, echter Modellaufruf oder Livekonfigurationslesen.
Keine UI-/ARM-/Mac-/Handy-/WAN-Abnahme. Kein systemweiter Lösch-/Root-Test.

Unixkonto/Quoten sind keine vollständige Sandbox. Logische Dateilöschung ist
kein SSD-Secure-Erase. Beliebige externe Schreiborte, ACLs/IPC/Spool, Snapshots,
Datenträgerverschlüsselung und Hardlinks müssen vor Pilot real geprüft werden;
UID-Inodeabgleich verweigert Freigabe bei unklaren Resten, löscht aber nicht
beliebige Hostpfade. Abbruch kann teilweise geleerte Wurzeln zurücklassen;
Wiederherstellung ist nicht zugesagt. Gäste-Abholweg und Empfangsarchiv-RAMbedarf
sind S7-seitig weiter zu prüfen. Kontakt-/Rechtsgrundlagenfelder müssen ergänzt
werden. Kein aktiver Dienst wird als Anwesenheit/Arbeitsleistung interpretiert.

## Nächste Schritte

1. K/S7/S3 bestätigen kompatible Commits und lösen Punkte 1–4 mit Mock-/Gate-
   Nachweisen; K übernimmt ausschließlich geprüfte Commits.
2. Separat freigegebene synthetische Root-/Quota-/systemd-/Proxy-Erprobung erst
   auf zwei Plätzen, keine Modelle/Nutzdaten; dann zehn Plätze/FIFO.
3. Datenschutz/Betriebskontakte und Fristen abnehmen, erst danach Netz-/Pilot-
   Freigabe. Keine stillen Bestandsmigrationen oder echten Deploys aus I2.
