# R2-Befund · I1-S4-provision

Lesend geprüft am 28.10.2026, etwa 18:08–18:20 +01:00, als
`uid=1000(medienhaus)`. Keine Konten, Dienste, Quoten oder Mounts verändert.
Worktree `v3/s4`, Ausgangspunkt `eb534bb`; pi-hub read-only bei `3dae8ca`.

| Prüfung / Befehl | Beobachtung | Folge |
|---|---|---|
| `lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINTS` | NVMe `nvme0n1p2`, 931,5 GiB, ext4 `/`; SATA `sda1`, 1,8 TiB, ext4 `/mnt/data` | Keine Platzhalter für SATA nötig; keine Formatierung / Mountänderung durch Skripte |
| `findmnt --target /`, `findmnt --target /mnt/data` | `/dev/nvme0n1p2` und `/dev/sda1`, jeweils `rw,relatime` | `usrquota` nicht sichtbar; interne ext4-Quota-Features nicht als normaler Nutzer geprüft; aktive Quote ist NICHT nachgewiesen |
| `df -hT / /home`, `df -hT /mnt/data` | `/`: 916 GiB, 53 GiB benutzt, 817 GiB frei; SATA: 1,8 TiB, 2,1 MiB benutzt, 1,7 TiB frei | Momentaufnahme, keine Kapazitätsgarantie |
| `ls -ld /home /mnt/data /home/medienhaus /usr/local/bin` | `/home` root 0755; `/mnt/data` medienhaus 0755; bestehendes Home 0750; `/usr/local/bin` root 0755 und leer | SATA-Mountwurzel muss vor Apply separat root-kontrolliert werden. Kein Werkzeugzugriff über fremdes Home |
| `free -h` | 30 GiB RAM, 27 GiB verfügbar, Swap 8 GiB unbenutzt | Vorschlag je Slot MemoryHigh 2 GiB, MemoryMax 2,5 GiB; erst Lasttest, keine Reservierung |
| `systemctl --version`, `stat -fc %T /sys/fs/cgroup` | systemd 259, cgroup2fs | Accountgrenzen als rootverwaltetes `user-UID.slice.d`-Drop-in |
| `command -v …` und explizit `/usr/sbin/…` | Quota/Setquota/Quotaon/Repquota/XFS-Quota und Shellcheck fehlen; runuser/useradd/usermod/userdel/groupadd unter `/usr/sbin` vorhanden | Skripte ergänzen System-PATH; keine Installation. Shellcheck nicht ausführbar |
| Werkzeugsuche mit vereinbartem PATH | node `/usr/bin/node`, python3/git/cc/make/tar/openssl/ss/flock vorhanden; pi nur persönlicher mise-Shim; Rust nur persönliches `~/.cargo/bin`; ffmpeg nicht gefunden | Rootverwaltete gemeinsame pi-/Rust-/ffmpeg-Installation separat nötig; keine Shims aus medienhaus-HOME verlinken |
| `getent passwd cocopi-s01…s10`, `getent group …` | Keine Slotkonten / keine cocopi-slots-Gruppe; docker-Gruppe vorhanden (GID 116), sudo-Gruppe vorhanden | Keine vorhandenen Slotdaten migriert; privilegierte Gruppen und effektive sudo-Rechte prüfen |
| `server.mjs` bei Hub `3dae8ca` (Textprüfung) | HUB_HOST/PORT/PI_BIN/ROOTS/STATE vorhanden; Token über HUB_TOKEN oder private Datei; Registry kann Prozesse wieder starten | Neue getrennte Instanzen, beim Pilot HUB_STATE=off. Keine Modellaufrufe, kein Token aus Echtdaten gelesen |
| gleiche Textprüfung | Kein belegter Vertrag für Gerätebindung, zwei offene / eine aktive Session oder globale CPU-Queue | Apply verlangt vom Hub-Besitzer bestätigten Vertrag; Marker allein ist keine Sicherheitsprüfung |

## Methode: harte Teilbudgets statt erfundener gemeinsamer Quote

Beide tatsächlichen Datenträger sind ext4. Daher **Unix-Userquote pro UID und
Dateisystem** (`usrquota`, aktiv zu prüfen), keine XFS-Projektquote. XFS wird
bewusst abgewiesen; bei künftigem Wechsel braucht es neuen Befund und eine
Pfad-/Projekt-ID-Prüfung. Projektquote allein wäre bei vollem Shellzugriff ohne
Schreibpfadbegrenzung keine vollständige Accountquote.

Je UID: `/` hard = 9.765.625 KiB = **10.000.000.000 Byte**;
`/mnt/data` hard = 68.359.375 KiB = **70.000.000.000 Byte**.
Zusammen höchstens **80.000.000.000 Byte** auf diesen Dateisystemen, ohne
flexiblen Transfer zwischen Teilbudgets. Alle der UID gehörenden Dateien zählen,
auch außerhalb Home/Workspace. Soft = Hard; zusätzlich Inodegrenzen. Die Quota
zählt belegte Dateisystemblöcke, nicht die Summe logischer Dateilängen.

NICHT damit abgedeckt: tmpfs (`/run/user`, `/dev/shm`, ggf. `/tmp`), weitere
Mounts, Netzwerkdateisysteme, durch privilegierte Dienste erzeugte Dateien anderer
UID. RAM-Dateien müssen über cgroup/RuntimeDirectorySize und Hostpolicy geprüft
werden. Das Skript verspricht deshalb kein lückenloses globales Speicherbudget
für beliebige künftige Mounts. Vor Pilot Schreibpfade inventarisieren.

Die Skripte aktivieren Quoten nicht auf einem laufenden Root-Dateisystem, ändern
keine `/etc/fstab`, führen kein `quotacheck`, keinen Remount und keine
Paketinstallation aus. Vorbereitung/Filesystemprüfung/Mountfenster brauchen
separate menschliche Freigabe. `--apply` bricht vor Kontoanlage ab, wenn aktive
ext4-Userquoten und Werkzeuge nicht nachgewiesen sind.

## Grenzen dieser Aufnahme

Keine Root-Abfrage, keine Prüfung privater `/etc/sudoers`-Inhalte, keine
Quotaüberschreitung, kein Benutzerwechsel, kein Linger-/Dienststart, kein
Negativtest gegen echte Homes, kein Modell-/Netzwerkaufruf. Der alte persönliche
Hub wird nicht angefasst. Genauere CPU-Ausstattung und dauerhafte Lastwerte sind
nicht neu vermessen; Tutorial-Hardwareangaben bleiben Planungsgrundlage.
