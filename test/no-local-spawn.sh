#!/usr/bin/env bash
# Beobachtung, KEIN Ersatz für den zentralen Spawn-Guard-Test.
# bash test/no-local-spawn.sh <cocopi-bin> [scene]
set -euo pipefail
[[ $# -ge 1 && $# -le 2 ]] || { echo "Aufruf: $0 <cocopi-bin> [scene]" >&2; exit 2; }
bin=$(cd -- "$(dirname -- "$1")" && printf '%s/%s' "$PWD" "$(basename -- "$1")")
[[ -x "$bin" ]] || { echo "Nicht ausführbar: $bin" >&2; exit 2; }
command -v timeout >/dev/null || { echo 'timeout fehlt' >&2; exit 2; }
scene=${2:-${COPI_SCENE:-start}}
[[ -n "$scene" ]] || { echo 'Szene darf nicht leer sein' >&2; exit 2; }
logs=$(mktemp -d "${TMPDIR:-/tmp}/no-local-spawn.XXXXXXXX")
mkdir -p "$logs/fake-bin" "$logs/home" "$logs/tmp" "$logs/config" "$logs/cache" "$logs/data" "$logs/agent"
# Fake-Programme beenden sich sofort; es läuft niemals echtes pi/Node über PATH.
for exe in pi node; do
    printf '%s\n' '#!/bin/sh' 'printf "%s\n" "${0##*/}" >> "$NO_LOCAL_SPAWN_MARKER"' 'exit 97' >"$logs/fake-bin/$exe"
    chmod +x "$logs/fake-bin/$exe"
    # Positivkontrolle jedes Traps, getrennt vom eigentlichen Marker.
    rc=0
    NO_LOCAL_SPAWN_MARKER="$logs/control.marker" "$logs/fake-bin/$exe" || rc=$?
    [[ $rc == 97 ]] && grep -qx "$exe" "$logs/control.marker" || { echo 'Fake-Positivkontrolle fehlgeschlagen' >&2; exit 2; }
done
rc=0
(cd "$logs/home" && env -i "PATH=$logs/fake-bin:$PATH" "HOME=$logs/home" "USERPROFILE=$logs/home" \
    "TMPDIR=$logs/tmp" "XDG_CONFIG_HOME=$logs/config" "XDG_CACHE_HOME=$logs/cache" "XDG_DATA_HOME=$logs/data" \
    "PI_CODING_AGENT_DIR=$logs/agent" "PI_SESSIONS_DIR=$logs/home/sessions" \
    "COPI_STATE_MIRROR_DIR=$logs/cache/copi/agentscope-remote" \
    "COPI_SCENE=$scene" "NO_LOCAL_SPAWN_MARKER=$logs/spawn.marker" LC_ALL=C.UTF-8 \
    timeout --kill-after=5s "${NO_LOCAL_SPAWN_TIMEOUT:-60}s" "$bin" --shot "$logs/shot.png" --size 640x420) >"$logs/app.log" 2>&1 || rc=$?
{
    date -Is
    printf 'binary=%s\nscene=%s\nexit=%s\n' "$bin" "$scene" "$rc"
    sha256sum "$bin"
} >"$logs/context.log"
if [[ -e "$logs/spawn.marker" ]]; then
    printf 'FAIL no-local-spawn | pi/node über PATH aufgerufen; scene=%s log=%s\n' "$scene" "$logs/app.log"
    exit 1
fi
if (( rc != 0 )) || [[ ! -s "$logs/shot.png" ]]; then
    printf 'FAIL no-local-spawn | exit=%s oder Screenshot fehlt; kein Beweis, scene=%s log=%s\n' "$rc" "$scene" "$logs/app.log"
    exit 1
fi
printf 'PASS no-local-spawn | Marker fehlt; NUR Headless-Beobachtung, kein Remote-only-Nachweis; scene=%s log=%s\n' "$scene" "$logs/app.log"
