#!/usr/bin/env bash
set -euo pipefail
# Niemals xtrace: Token bleibt aus Argumenten, URLs und Journal heraus.
set +x
[[ ${1:-} =~ ^s(0[1-9]|10)$ && ${HUB_SLOT:-} == "$1" ]] || exit 1
[[ $(id -un) == "cocopi-$1" && ${HUB_HOST:-} == 127.0.0.1 ]] || exit 1
[[ -r /etc/cocopi/slots/$1/token ]] || exit 1
HUB_TOKEN=$(< "/etc/cocopi/slots/$1/token")
[[ ${#HUB_TOKEN} -ge 40 ]] || exit 1
export HUB_TOKEN
exec "$HUB_NODE" "$HUB_DIR/server.mjs"
