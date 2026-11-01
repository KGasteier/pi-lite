#!/usr/bin/env bash
# Erst nach Storage-/Quotenfreigabe! Keine Docker-/Coolifyinstallation.
set -euo pipefail
set +x
base=/mnt/data/cocopi-container-pilot
if [[ ${1:---dry-run} != --ausfuehren ]]; then
  printf 'PLAN: Zehn NEUE private Slotverzeichnisse und Hubtokens unter %s; keine Loeschung, keine Quotenanlage.\n' "$base"
  exit 0
fi
[[ $EUID == 0 && ${COCOPI_CONTAINER_STORAGE_FREIGABE:-} == ja ]] || { echo 'Root und explizite Freigabe nötig.' >&2; exit 1; }
[[ ! -e $base && ! -L $base ]] || { echo 'Ziel vorhanden: STOPP, keine Übernahme/Überschreibung.' >&2; exit 1; }
[[ -d /mnt/data && ! -L /mnt/data ]] || exit 1
[[ $(stat -c %u /mnt/data) == 0 ]] || exit 1
[[ $(stat -c %a /mnt/data) =~ ^[0-7][0-5][0-5]$ ]] || { echo 'Mountwurzel darf nicht gruppen-/fremdschreibbar sein.' >&2; exit 1; }
mountpoint -q /mnt/data || { echo 'Eigener geprüfter Mount fehlt.' >&2; exit 1; }
umask 077
install -d -o root -g root -m 700 "$base"
for i in $(seq 1 10); do
  slot=$(printf 'slot%02d' "$i")
  install -d -o root -g root -m 700 "$base/$slot"
  install -d -o 1000 -g 1000 -m 700 "$base/$slot/home"
  openssl rand -hex 32 > "$base/$slot/hub_token"
  chown 1000:1000 "$base/$slot/hub_token"
  chmod 400 "$base/$slot/hub_token"
done
printf 'Angelegt; noch keine Quoten, Container oder Dienste. Storage-/Quotenabnahme bleibt separat nötig.\n'
