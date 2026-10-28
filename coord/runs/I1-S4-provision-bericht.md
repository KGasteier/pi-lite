# I1-S4-provision · Abschlussbericht

28.10.2026 · Rolle S4 · Worktree `/home/medienhaus/Work/pi-lite-wt/s4`,
Branch `v3/s4`, Ausgangspunkt `eb534bb`. Nur Produktpfade `deploy/r2/` geändert;
zusätzlich vereinbarter Heartbeat und dieser Bericht im Koordinationsbaum.
Kein Push/Merge, keine Änderung an R2-Konten, Diensten, Mounts oder Netzwerk.

## Commits

- `57676e3` — Befund und sichere Dry-Run-Grundlage für zwei Slots.
- `e7c68d0` — Abnahme, Rückbau, A4, Dokumentation und isolierte Regression.
- `0b98b19` — Preflight-Erfolgsstatus neuer Konten abgesichert; 14 Tests grün.

Worktree nach letztem Commit sauber (`git status --short` ohne Ausgabe).

## Dateien / Ergebnis

`deploy/r2/{slots.conf,provision.sh,check.sh,teardown.sh,common.sh,hub-launch.sh,
cocopi-hub@.service,quota-read.py,slice-read.py,test_scripts.py,BEFUND.md,
README.md,DRYRUN.txt,TESTS.txt}`.

- 10 Slots s01…s10, s10 Reserve; Standardbereitstellung nur s01,s02.
- Vorschlag feste UID/GID 20001…20010, private Gruppen + cocopi-slots;
  Homes/Daten 0700, keine sudo-/docker-Rechte, rootverwaltete Slice-Drop-ins.
- Tatsächlich beide SSDs ext4: NVMe `/`, SATA `/mnt/data`. Harte feste Teilquoten
  **10 + 70 GB dezimal**, zusammen exakt 80 GB, keine erfundene mountübergreifende
  dynamische Quote; zusätzliche Inodegrenzen.
- Hub je Slot, Loopback 127.0.0.1:8801…8810, Token-Datei 0600 ohne Inhaltsausgabe.
  Unit-/Startwrapper, keine stillen Tokenrotationen oder laufenden Hub-Restarts.
- Rückbau standardmäßig Sperre + root-only Archiv; Datenlöschung nur `--purge`
  nach frischem, geprüftem Archiv. Fortschrittsmarker und UID-Tombstone.
- README mit externer Backup-/Restoreplanung, Lasttest 2 → 9 Personen und
  **A4-Vorschlag** (Slot/Unixkonto/Hubport/Gerätetoken-Verweis/Grenzen).

## Nachweise (geprüft)

- `PYTHONDONTWRITEBYTECODE=1 python3 deploy/r2/test_scripts.py`:
  **14 Tests grün**, darunter Standard=Dry-Run, Änderungsbefehle per Sentinel
  gesperrt, Rootpflicht, Argumentfehler, explizite Rückbauauswahl, Budget/Ports,
  Symlinkabwehr, sudo-Listing (jede Erlaubnis ablehnen), Quota-/Slice-Leser,
  Mock-Mountprüfung und Hubvertragssperre, erfolgreicher neuer Konto-Preflight.
- `bash -n deploy/r2/*.sh deploy/r2/slots.conf`: grün.
- `git diff --check`: grün.
- `deploy/r2/DRYRUN.txt`: provision/check/teardown für s01,s02 tatsächlich als
  normaler Nutzer uid=1000 ausgeführt, kein Apply.
- `deploy/r2/TESTS.txt`: aufgezeichneter Testlauf und Syntaxcheck.
- Befund durch lesende lsblk/findmnt/df/free/systemctl-Version/getent/command-v-
  und Hub-Quellprüfung; Details mit Befehlen in `BEFUND.md`.

## Nicht geprüft / nicht geliefert behauptet

Kein Root-Apply, keine echten Slotkonten, kein runuser gegen echte Homes, keine
Quotaüberschreitung, keine effektiven Slice-/Linger-/Listenerprüfungen. Rückbau,
Purge und Wiederaufnahme nur statisch/teilweise logisch geprüft, **nicht** auf
Testkonten ausgeführt. Shellcheck fehlt; daher keine Shellcheck-Abnahme.
Kein Modellaufruf, kein Internet, kein Orca-Code ausgeführt/übernommen. Kein
Rust-Build nötig. Tutorial unverändert (Update erst I3).

## Blocker / offene Fragen

1. Auf R2 fehlen Quotenwerkzeuge und sichtbare usrquota-Aktivierung auf beiden
   ext4-Mounts. `/mnt/data` ist medienhaus-eigen. Gemeinsame rootverwaltete pi-/
   Rust-/ffmpeg-Ausstattung fehlt. Apply muss bis separater Vorbereitung stoppen.
2. 10/70-GB-Aufteilung, CPU 100 %, MemoryHigh 2 GiB / Max 2,5 GiB, 256 Tasks,
   UID/GID-Bereich, Backupziel/-fristen und vorgeschlagene Lasttest-Abbruchwerte
   sind **Vorschläge zur Freigabe**, keine bereits abgenommenen Betriebswerte.
3. Userquote deckt die beiden SSDs ab, nicht weitere Mounts/tmpfs/Dateien anderer
   UID; Account-Slice ist keine vollständige Sandbox. Globale Schwerlastqueue
   kann direkte Shelljobs mit vollem Tool-Use nicht ohne weitere Regeln erzwingen.
4. Externer Restore und vollständiger Provision-/Teardown-Zyklus müssen nach
   menschlicher Freigabe zuerst in separater Testumgebung abgenommen werden.

## Änderungsbitten (keine fremden Dateien geändert)

- **S3 + S1 / A4:** Tabelle in README bestätigen; Hubvertrag `cocopi-slots-v1`
  definieren und abnehmen. Vorgeschlagene Parameter: HUB_SLOT,
  HUB_MAX_OPEN_SESSIONS=2, HUB_MAX_ACTIVE_SESSIONS=1, HUB_GLOBAL_HEAVY_JOBS=1.
  Aktueller read-only Hub `3dae8ca` belegt diese Grenzen nicht. Konfiguration
  bleibt `HUB_CONTRACT=unconfirmed`, daher kein Apply-Dienststart.
- **S3:** pro Gerät widerrufbare Token mit serverseitiger Slotbindung; separates
  Bootstrap-Token nicht als Geräte-Enrollment ausgeben. Prozessübergreifende
  globale Queue für neun getrennte Hubinstanzen und Shell-Umgehungsgrenze klären.
  Widerruf, Sessiongrenzen, Registry/Resume und Fehlerzustände mit Mocks nachweisen.
- **S6:** unabhängiges Sicherheitsreview der Apply-/Rückbaupfade, insbesondere
  Unterbrechung/Archiv/Purge, sowie Tests auf freigegebener separater Testmaschine.
- **K/Mensch:** keine R2-Provisionierung ohne neue Freigabe; Voraussetzungen und
  A4 zuerst abschließen. Root darf nur eine geprüfte rootverwaltete Skriptkopie
  ausführen, nie direkt den nutzerbeschreibbaren Worktree.
