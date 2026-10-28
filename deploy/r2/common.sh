#!/usr/bin/env bash
# Gemeinsame Funktionen; keine Systemänderung beim Laden.
set -euo pipefail
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export LC_ALL=C
BASE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
MODE=dry-run
SLOT_LIST=
PURGE=false
EXPLICIT_SLOTS=false

fail() { printf 'FEHLER: %s\n' "$*" >&2; exit 1; }
note() { printf '%s\n' "$*"; }

# Für Root: Konfiguration und alle Eltern dürfen keinem Slot gehören / beschreibbar sein.
root_controlled() {
    local path=$1 owner mode
    [[ $path == /* && $path != *'/../'* ]] || return 1
    while :; do
        [[ -e $path && ! -L $path ]] || return 1
        owner=$(stat -c %u -- "$path")
        mode=$(stat -c %a -- "$path")
        [[ $owner == 0 ]] && (( (8#$mode & 0022) == 0 )) || return 1
        [[ $path != / ]] || break
        path=$(dirname -- "$path")
    done
}
if (( EUID == 0 )); then
    root_controlled "$BASE/slots.conf" || fail 'Root darf nur eine rootverwaltete Skriptkopie verwenden (README).'
fi
# shellcheck source=deploy/r2/slots.conf
source "$BASE/slots.conf"

parse_args() {
    local operation=$1 arg seen_mode=false
    shift
    while (($#)); do
        arg=$1
        case $arg in
            --dry-run|--apply)
                [[ $seen_mode == false ]] || fail 'Bitte genau einen Ausführungsmodus wählen.'
                MODE=${arg#--}; seen_mode=true; shift ;;
            --slots)
                (($# >= 2)) || fail 'Nach --slots fehlt die Liste (z. B. s01,s02).'
                [[ $EXPLICIT_SLOTS == false ]] || fail '--slots nur einmal verwenden.'
                SLOT_LIST=$2; EXPLICIT_SLOTS=true; shift 2 ;;
            --purge)
                [[ $operation == teardown ]] || fail '--purge gehört nur zum Rückbau.'
                PURGE=true; shift ;;
            --help|-h)
                printf 'Aufruf: %s [--dry-run|--apply] [--slots s01,s02]' "$0"
                [[ $operation != teardown ]] || printf ' [--purge] (Rückbau: --slots zwingend)'
                printf '\nStandard: Dry-Run; Apply benötigt Root und separate menschliche Freigabe.\n'
                exit 0 ;;
            *) fail "Unbekannte Option: $arg (Hilfe: --help)." ;;
        esac
    done
    if [[ $operation == teardown && $EXPLICIT_SLOTS == false ]]; then
        fail 'Rückbau benötigt ausdrücklich --slots; es gibt dafür keine Standardauswahl.'
    fi
    SLOT_LIST=${SLOT_LIST:-$DEFAULT_SLOTS}
    [[ $SLOT_LIST =~ ^s(0[1-9]|10)(,s(0[1-9]|10))*$ ]] || fail 'Slots müssen s01 bis s10 sein, getrennt durch Komma, ohne Leerzeichen.'
    IFS=, read -r -a SELECTED <<< "$SLOT_LIST"
    local seen=' '
    for arg in "${SELECTED[@]}"; do
        [[ $seen != *" $arg "* ]] || fail "Slot doppelt angegeben: $arg."
        seen+="$arg "
    done
    [[ $MODE != apply || $EUID == 0 ]] || fail '--apply benötigt Root. Es wurde nichts geändert.'
    [[ $HUB_BIND == 127.0.0.1 ]] || fail 'Hub darf ausschließlich an 127.0.0.1 binden.'
    [[ $(( NVME_QUOTA_KIB + DATA_QUOTA_KIB )) == 78125000 ]] || fail 'Teilquoten müssen zusammen exakt 80 GB dezimal ergeben.'
}

slot_vars() {
    SLOT=$1
    USER_NAME=cocopi-$SLOT
    SLOT_HOME=$HOME_BASE/$USER_NAME
    SLOT_DATA=$DATA_BASE/$USER_NAME
    PORT=$((HUB_PORT_BASE + 10#${SLOT#s}))
    SLOT_ETC=/etc/cocopi/slots/$SLOT
    UNIT=cocopi-hub@$SLOT.service
    UID_NUM=
    if getent passwd "$USER_NAME" >/dev/null; then
        UID_NUM=$(id -u "$USER_NAME")
    fi
}

require_commands() {
    local cmd
    for cmd in "$@"; do
        command -v "$cmd" >/dev/null || fail "Werkzeug fehlt: $cmd. Separat vorbereiten, dann erneut prüfen."
    done
}

run() {
    if [[ $MODE == dry-run ]]; then
        printf 'PLAN:'; printf ' %q' "$@"; printf '\n'
    else
        "$@"
    fi
}

# Keine Symlinks in verwalteten Pfaden akzeptieren, auch keine kaputten.
no_symlinks() {
    local path=$1
    [[ $path == /* && $path != *'/../'* && $path != *'/./'* ]] || return 1
    while [[ $path != / ]]; do
        [[ ! -L $path ]] || return 1
        path=$(dirname -- "$path")
    done
}

managed_dir() {
    local path=$1 mode=$2 owner=$3 group=$4
    no_symlinks "$path" || fail "Symlink im verwalteten Pfad: $path. Manuell prüfen."
    if [[ -d $path && $(stat -c '%a:%U:%G' "$path") == "$mode:$owner:$group" ]]; then
        return
    fi
    run install -d -m "$mode" -o "$owner" -g "$group" -- "$path"
}

# Nur rootverwaltete Ziele, niemals Root-Schreibzugriff innerhalb eines Nutzer-Homes.
managed_file() {
    local path=$1 mode=$2 owner=$3 group=$4 tmp
    if [[ $MODE == dry-run ]]; then
        note "PLAN: verwaltete Datei $path ($mode, $owner:$group), nur bei Änderung schreiben"
        return
    fi
    no_symlinks "$path" || fail "Symlink im verwalteten Dateipfad: $path."
    root_controlled "$(dirname -- "$path")" || fail "Dateieltern nicht rootverwaltet: $path."
    tmp=$(mktemp)
    chmod 600 "$tmp"
    # Inhalt enthält keine Geheimnisse; Token wird separat ohne Ausgabe erzeugt.
    if ! { cat > "$tmp"; }; then rm -f -- "$tmp"; fail "Dateierzeugung gescheitert: $path."; fi
    if [[ ! -f $path ]] || ! cmp -s -- "$tmp" "$path"; then
        install -m "$mode" -o "$owner" -g "$group" -- "$tmp" "$path"
    elif [[ $(stat -c '%a:%U:%G' "$path") != "$mode:$owner:$group" ]]; then
        chmod "$mode" "$path"
        chown "$owner:$group" "$path"
    fi
    rm -f -- "$tmp"
}

assert_account() {
    local record name password uid gid comment home shell groups group
    record=$(getent passwd "$USER_NAME") || fail "Konto fehlt: $USER_NAME."
    IFS=: read -r name password uid gid comment home shell <<< "$record"
    [[ $name == "$USER_NAME" && $home == "$SLOT_HOME" && $shell == /bin/bash && $comment == "cocopi managed slot $SLOT" ]] || fail "Bestehendes Konto $USER_NAME ist nicht eindeutig von diesem Skript verwaltet."
    (( uid >= 1000 )) || fail "Unerwartete System-UID für $USER_NAME."
    [[ $(id -gn "$USER_NAME") == "$USER_NAME" ]] || fail "Private Primärgruppe fehlt: $USER_NAME."
    groups=$(id -nG "$USER_NAME")
    for group in $groups; do
        [[ $group == "$USER_NAME" || $group == "$SLOT_GROUP" ]] || fail "Unerlaubte Gruppe $group bei $USER_NAME. Nicht automatisch entfernen; prüfen."
    done
    [[ " $groups " == *" $SLOT_GROUP "* ]] || fail "Slotgruppe fehlt: $USER_NAME."
    no_symlinks "$SLOT_HOME" && no_symlinks "$SLOT_DATA" || fail "Symlink am Slotpfad von $USER_NAME."
    UID_NUM=$uid
}

user_systemctl() {
    runuser -u "$USER_NAME" -- env -i HOME="$SLOT_HOME" USER="$USER_NAME" \
        LOGNAME="$USER_NAME" PATH=/usr/bin:/bin XDG_RUNTIME_DIR="/run/user/$UID_NUM" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$UID_NUM/bus" systemctl --user "$@"
}

check_mounts() {
    local mount type options
    for mount in "$NVME_MOUNT" "$DATA_MOUNT"; do
        [[ $(findmnt -n -o TARGET --target "$mount") == "$mount" ]] || fail "Kein eigener Mount an $mount."
        type=$(findmnt -n -o FSTYPE --target "$mount")
        options=$(findmnt -n -o OPTIONS --target "$mount")
        [[ $type == ext4 ]] || fail "Dateisystem $type an $mount nicht freigegeben; BEFUND aktualisieren (kein XFS-Fallback)."
        [[ ,$options, == *,usrquota,* || ,$options, == *,quota,* ]] || fail "ext4-Userquote an $mount nicht als Mountoption belegt. Separates Wartungsfenster nötig."
        quotaon -p -u "$mount" | grep -Eq '(^|[[:space:]])is on$' || fail "Userquote an $mount nicht aktiv / nicht nachweisbar."
    done
    [[ $(findmnt -n -o SOURCE --target "$NVME_MOUNT") != "$(findmnt -n -o SOURCE --target "$DATA_MOUNT")" ]] || fail 'Teilbudgets dürfen nicht auf dasselbe Dateisystem zeigen.'
}

check_hub_contract() {
    [[ $HUB_CONTRACT == "$REQUIRED_HUB_CONTRACT" ]] || fail 'Hub-Vertrag unbestätigt: A4 mit S3/S1 abschließen; kein Dienststart.'
    root_controlled "$HUB_CONTRACT_FILE" && [[ $(< "$HUB_CONTRACT_FILE") == "$REQUIRED_HUB_CONTRACT" ]] || fail 'Bestätigter Vertrag der bereitgestellten Hub-Version fehlt.'
    root_controlled "$HUB_DIR/server.mjs" || fail 'Hub-Code nicht rootverwaltet bereitgestellt.'
}

acquire_lock() {
    managed_dir "$STATE_BASE" 700 root root
    # Nicht in /tmp: keine fremd angelegte Lockdatei öffnen.
    no_symlinks "$STATE_BASE/lock" || fail 'Symlink an Sperrdatei.'
    exec 9> "$STATE_BASE/lock"
    flock -n 9 || fail 'Ein anderer Slot-Verwaltungslauf arbeitet bereits. Später erneut versuchen.'
}
