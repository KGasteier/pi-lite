# Dein Auftrag: S4 · R2-System, Automatisierung — I1-S4-provision (≤ 110 min)

Rolle S4 (§5, §8). Worktree `/home/medienhaus/Work/pi-lite-wt/s4`, Branch `v3/s4`.
Du schreibst nur `deploy/r2/` (neu). **Nichts ausführen, was R2 verändert** (kein useradd, kein systemctl
enable/start, kein setquota, kein sudo). Nur lesende Systemabfragen sind erlaubt (z. B. `lsblk`, `findmnt`,
`df`, `systemctl --version`, `id`, `getent`, `quota --version`, `command -v`), um das Skript an die reale
Maschine anzupassen. Befunde in `deploy/r2/BEFUND.md`.

Grundlage: E1 (Prompt §2), Tutorial Kap. 7–10, 18; Prompt §8 S4.

Liefere (Bash, `set -euo pipefail`, shellcheck-sauber wenn `shellcheck` vorhanden, **Standard `--dry-run`**,
echtes Ausführen nur mit `--apply` UND Root-Prüfung; idempotent: erst Zustand prüfen, dann handeln):
1. `deploy/r2/slots.conf` — Slots `s01…s10` (s10 = Reserve), Unix-Name `cocopi-sNN`, Gruppe `cocopi-slots`,
   Grenzwerte als Variablen (CPUQuota, MemoryMax/MemoryHigh, TasksMax, Quote 80 GB über beide SSDs,
   max. 2 offene Sessions / 1 aktive als Hub-Parameter), Pfade für zweite SSD (aus BEFUND, sonst Platzhalter).
2. `deploy/r2/provision.sh [--dry-run|--apply] [--slots s01,s02]` — Konto, Home 0700, Werkzeugzugang (kein sudo,
   nicht in docker-Gruppe, Prüfung), systemd-User-Slice-Drop-in `user-<uid>.slice.d/` mit Grenzen, Linger,
   Quote (Methode je Dateisystem aus BEFUND: ext4 usrquota / XFS-Projektquote; wenn über zwei Datenträger,
   dokumentiere wie die 80 GB summiert durchgesetzt werden oder ehrlich, was nicht geht), Hub-User-Dienst je Slot
   (Unit-Vorlage, Port je Slot, Bindung 127.0.0.1, Token-Datei 0600, Token NIE ausgeben/loggen).
   Erste Ausbaustufe: **2 Testkonten** (`--slots s01,s02`).
3. `deploy/r2/check.sh [--slots …]` — Prüfskript mit Negativtests (als Root Ausführung via `runuser`):
   fremdes Home nicht lesbar, sudo verweigert, nicht in docker, Quote aktiv, Slice-Grenzen gesetzt, Linger,
   Hub nur auf 127.0.0.1. Ausgabe OK/FEHLER je Punkt. Im Dry-Run nur anzeigen, was geprüft würde.
4. `deploy/r2/teardown.sh [--dry-run|--apply] --slots …` — Rückbau (Home vorher als Archiv sichern, nicht löschen
   ohne `--purge`).
5. `deploy/r2/README.md` — Ablauf, Backup-/Restore-Plan (extern), Lasttestplan (2 Personen, dann 9; ein schwerer
   CPU-Job global), **A4-Vorschlag** Tabelle Slot ↔ Unix-Konto ↔ Hub-Instanz/Port ↔ Geräte-Token ↔ Grenzen.

Dry-Run-Ausgabe von provision/check/teardown für s01,s02 in `deploy/r2/DRYRUN.txt` ablegen (als normaler Nutzer
ausgeführt). Checkpoint-Commits. Bericht nach `coord/runs/I1-S4-provision-bericht.md`.
Tutorial-Update kommt erst in I3 — jetzt nicht.
