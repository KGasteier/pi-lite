#!/usr/bin/env bash
# Nur lesende Bestandsaufnahme. Keine sudo-, Netzwerk- oder Modellaufrufe.
set -euo pipefail
set +x
printf '=== Host und Ressourcen ===\n'
hostname
uname -m
nproc
free -h
df -h / /mnt/data
printf '\n=== Mounts: Dateisystem und Quoten separat administrativ bestätigen ===\n'
findmnt -T / -o TARGET,SOURCE,FSTYPE,OPTIONS
findmnt -T /mnt/data -o TARGET,SOURCE,FSTYPE,OPTIONS
printf '\n=== cgroup und Uhr ===\n'
stat -fc %T /sys/fs/cgroup
timedatectl show -p Timezone -p NTPSynchronized
printf '\n=== gemeinsame Tools (Rootinstallation zusätzlich prüfen) ===\n'
for tool in node pi cargo rustc python3 git ffmpeg cc make quotaon setquota visudo; do
  command -v "$tool" || true
done
printf '\n=== Namens-/UID-Konflikte; vorhandene Konten NICHT übernehmen ===\n'
for i in $(seq 1 10); do
  slot=$(printf 'account%02d' "$i")
  getent passwd "$slot" || true
  getent passwd "$((20000+i))" || true
  getent group "$slot" || true
  getent group "$((20000+i))" || true
done
printf '\n=== Listener (keine Änderung) ===\n'
ss -ltn '( sport >= :8799 and sport <= :8810 )'
printf '\n=== Hinweis ===\nKein PASS/FAIL: Ausgaben fachlich prüfen; kein Nachweis aktiver Quoten, TLS oder Poolisolierung.\n'
