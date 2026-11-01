#!/bin/sh
set -eu
set +x
umask 077
[ "$(id -u)" = 1000 ] || { echo 'Nur nichtprivilegierter Containeruser.' >&2; exit 1; }
[ -s /run/secrets/hub_token ] || { echo 'Privater Hubtoken fehlt.' >&2; exit 1; }
HUB_TOKEN=$(cat /run/secrets/hub_token)
[ "${#HUB_TOKEN}" -ge 40 ] || { echo 'Hubtoken zu kurz.' >&2; exit 1; }
export HUB_TOKEN
mkdir -p "$HOME/arbeit" "$HOME/.pi/agent" "$HOME/.hub"
exec "$@"
