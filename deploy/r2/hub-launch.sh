#!/usr/bin/env bash
set -euo pipefail
# Niemals xtrace: Token bleibt aus Argumenten, URLs und Journal heraus.
set +x
[[ ${1:-} =~ ^account(0[1-9]|10)$ && ${HUB_SLOT:-} == "$1" ]] || exit 1
[[ $(id -un) == "$1" && ${HUB_HOST:-} == 127.0.0.1 ]] || exit 1
[[ ${HUB_MAX_OPEN_SESSIONS:-} == 2 && ${HUB_MAX_ACTIVE_SESSIONS:-} == 1 &&
   ${HUB_GLOBAL_HEAVY_JOBS:-} == 1 ]] || exit 1
[[ ${HUB_STATE:-} == "/home/$1/.cocopi-hub" && -d $HUB_STATE && ! -L $HUB_STATE &&
   $(stat -c '%a:%u:%g' -- "$HUB_STATE") == "700:$(id -u):$(id -g)" ]] || exit 1
[[ -n ${HUB_EMPFANG_PUBKEY:-} && ${HUB_ROOTS:-} == "/home/$1/arbeit" ]] || exit 1
[[ ${HUB_EPOCHE_DATEI:-} == "/var/lib/cocopi/slots/$1.epoche" ]] || exit 1
# Im Betrieb nie den Test-Fallback Epoche 0 nutzen; Pfad/Rechte/Format prüfen.
/usr/bin/python3 -I /usr/local/lib/cocopi-r2/epoch-read.py "$1" >/dev/null || exit 1
[[ -r /etc/cocopi/slots/$1/token ]] || exit 1
HUB_TOKEN=$(< "/etc/cocopi/slots/$1/token")
[[ ${#HUB_TOKEN} -ge 40 ]] || exit 1
export HUB_TOKEN
exec "$HUB_NODE" "$HUB_DIR/server.mjs"
