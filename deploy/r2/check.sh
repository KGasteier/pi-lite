#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=deploy/r2/common.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
parse_args check "$@"
note "Prüfung: $MODE; Slots: $SLOT_LIST. --apply prüft nur lesend / ohne Lasttest."
FAILURES=0
point() {
    local title=$1
    shift
    if "$@"; then
        printf 'OK: %s\n' "$title"
    else
        printf 'FEHLER: %s\n' "$title"
        FAILURES=$((FAILURES + 1))
    fi
}

as_slot() {
    runuser -u "$USER_NAME" -- env -i HOME="$SLOT_HOME" USER="$USER_NAME" \
        LOGNAME="$USER_NAME" PATH="$TOOL_PATH" /bin/bash --noprofile --norc -c "$1" bash "${@:2}"
}

private_path() {
    [[ -d $1 && ! -L $1 && $(stat -c '%a:%U:%G' "$1") == "700:$USER_NAME:$USER_NAME" ]]
}
no_privileged_groups() {
    local group
    for group in $(id -nG "$USER_NAME"); do
        [[ $group == "$USER_NAME" || $group == "$SLOT_GROUP" ]] || return 1
    done
}
no_sudo() {
    # -l fragt Berechtigung für true ab, führt es NICHT aus. Zusätzlich echter
    # unprivilegierter Negativversuch ohne Passwort und ohne Root-Nutzlast.
    ! sudo -l -U "$USER_NAME" /usr/bin/true >/dev/null 2>&1 &&
        ! as_slot 'sudo -n -- /usr/bin/true </dev/null >/dev/null 2>&1'
}
no_docker() {
    [[ " $(id -nG "$USER_NAME") " != *' docker '* ]] &&
        as_slot '[[ ! -w /var/run/docker.sock && ! -w /run/docker.sock ]]'
}
foreign_home_denied() {
    local home=$1
    # test -r/-x prüft effektive Rechte inkl. ACLs, ohne fremde Dateinamen zu lesen.
    as_slot '[[ -d "$1" && ! -r "$1" && ! -x "$1" ]]' "$home"
}
quota_exact() {
    local mount=$1 blocks=$2 inodes=$3 device
    device=$(findmnt -n -o SOURCE --target "$mount") || return 1
    python3 "$BASE/quota-read.py" "$device" "$UID_NUM" "$blocks" "$inodes"
}

slice_limits() {
    local expected_cpu expected_high expected_max actual_cpu
    expected_cpu=$(python3 -c 'import sys; print(int(float(sys.argv[1].rstrip("%"))*10000))' "$CPU_QUOTA")
    expected_high=$(numfmt --from=iec "$MEMORY_HIGH")
    expected_max=$(numfmt --from=iec "$MEMORY_MAX")
    systemctl is-active --quiet "user-$UID_NUM.slice" || return 1
    actual_cpu=$(systemctl show "user-$UID_NUM.slice" -p CPUQuotaPerSecUSec --value)
    # systemd formatiert Zeitwerte z. B. als 1s / 500ms; Python normalisiert.
    python3 - "$expected_cpu" "$actual_cpu" <<'TIME'
import re
import sys
units = {"us": 1, "ms": 1000, "s": 1000000, "min": 60000000}
text = sys.argv[2]
parts = re.findall(r"([0-9.]+)(us|ms|s|min)", text)
sys.exit(0 if parts and int(sum(float(n) * units[u] for n, u in parts)) == int(sys.argv[1]) else 1)
TIME
    [[ $(systemctl show "user-$UID_NUM.slice" -p MemoryHigh --value) == "$expected_high" &&
       $(systemctl show "user-$UID_NUM.slice" -p MemoryMax --value) == "$expected_max" &&
       $(systemctl show "user-$UID_NUM.slice" -p TasksMax --value) == "$TASKS_MAX" ]]
}
linger_active() { [[ $(loginctl show-user "$USER_NAME" -p Linger --value) == yes ]]; }
token_private() {
    [[ -f $SLOT_ETC/token && ! -L $SLOT_ETC/token &&
       $(stat -c '%a:%U:%G' "$SLOT_ETC/token") == "600:$USER_NAME:$USER_NAME" ]] &&
        # Nur Länge testen; kein Tokeninhalt und kein Token als Prozessargument.
        as_slot '[[ $(wc -c < "$1") -ge 41 ]]' "$SLOT_ETC/token"
}
hub_loopback() {
    local pid listeners line address count=0 expected
    user_systemctl is-active --quiet "$UNIT" || return 1
    pid=$(user_systemctl show "$UNIT" -p MainPID --value) || return 1
    [[ $pid =~ ^[1-9][0-9]*$ && $(stat -c %u "/proc/$pid") == "$UID_NUM" ]] || return 1
    listeners=$(ss -H -ltnp) || return 1
    expected="127.0.0.1:$PORT"
    while IFS= read -r line; do
        [[ -n $line ]] || continue
        address=$(awk '{print $4}' <<< "$line")
        # Port darf auch nicht zusätzlich einem anderen Listener gehören.
        if [[ $address == *":$PORT" ]]; then
            [[ $address == "$expected" && $line == *"pid=$pid,"* ]] || return 1
            count=$((count + 1))
        fi
        if [[ $line == *"pid=$pid,"* ]]; then
            [[ $address == "$expected" ]] || return 1
        fi
    done <<< "$listeners"
    (( count == 1 ))
}

if [[ $MODE == apply ]]; then
    require_commands runuser sudo systemctl loginctl ss findmnt python3 numfmt quotaon stat
    point 'ext4-Userquoten auf beiden SSDs aktiv' check_mounts
    point 'A4-Hub-Vertrag und rootverwalteter Hub bestätigt' check_hub_contract
fi

for slot in "${SELECTED[@]}"; do
    slot_vars "$slot"
    note "--- $SLOT ($USER_NAME), Port $PORT ---"
    if [[ $MODE == dry-run ]]; then
        note "PLAN: Konto/UID/Primärgruppe und Inventar prüfen; Home $SLOT_HOME und Daten $SLOT_DATA 0700"
        note 'PLAN: via runuser alle anderen vorhandenen Slot-Homes sowie medienhaus-HOME auf fehlendes Lese-/Suchrecht prüfen'
        note 'PLAN: effektives sudo-Listing + sudo -n true müssen verweigert werden; keine docker-/anderen Zusatzgruppen und kein Docker-Socketzugriff'
        note "PLAN: aktive ext4-Userquote + Q_GETQUOTA lesend: NVMe $NVME_QUOTA_KIB KiB, SATA $DATA_QUOTA_KIB KiB; Soft=Hard und Inodes"
        note "PLAN: effektive Account-Slice: CPUQuota=$CPU_QUOTA, MemoryHigh=$MEMORY_HIGH, MemoryMax=$MEMORY_MAX, TasksMax=$TASKS_MAX"
        note "PLAN: Linger=yes, Token 0600 ohne Inhalt, Dienst/PID-Besitzer und ausschließlich 127.0.0.1:$PORT via ss prüfen"
        note 'PLAN: Werkzeuge via runuser auffindbar; Hub-Vertrag prüfen (kein Session-/Lasttest, keine Modellaufrufe)'
        continue
    fi
    if [[ -z $UID_NUM ]]; then
        point "$SLOT: Konto vorhanden" false
        continue
    fi
    # assert_account beendet bei falschem Eigentümer sofort: fremde Konten niemals untersuchen.
    assert_account
    point "$SLOT: UID-Inventar stimmt" bash -c '[[ -f $1 && $(< "$1") == "$2" ]]' bash "$STATE_BASE/$SLOT.uid" "$UID_NUM"
    point "$SLOT: Home privat (0700)" private_path "$SLOT_HOME"
    point "$SLOT: Daten privat (0700)" private_path "$SLOT_DATA"
    for other in "${SLOTS[@]}"; do
        [[ $other != "$SLOT" ]] || continue
        other_home=$HOME_BASE/cocopi-$other
        if getent passwd "cocopi-$other" >/dev/null; then
            point "$SLOT: fremdes Home $other nicht lesbar / betretbar" foreign_home_denied "$other_home"
        fi
    done
    point "$SLOT: Admin-Home nicht lesbar / betretbar" foreign_home_denied /home/medienhaus
    point "$SLOT: nur private Gruppe + Slotgruppe" no_privileged_groups
    point "$SLOT: sudo verweigert" no_sudo
    point "$SLOT: keine docker-Gruppe / kein Docker-Socketzugriff" no_docker
    point "$SLOT: NVMe-Quote aktiv und exakt" quota_exact "$NVME_MOUNT" "$NVME_QUOTA_KIB" "$NVME_INODES"
    point "$SLOT: SATA-Quote aktiv und exakt" quota_exact "$DATA_MOUNT" "$DATA_QUOTA_KIB" "$DATA_INODES"
    point "$SLOT: Account-Slice-Grenzen effektiv" slice_limits
    point "$SLOT: Linger aktiv" linger_active
    point "$SLOT: Token vorhanden und 0600" token_private
    point "$SLOT: Hub nur an 127.0.0.1:$PORT und richtige UID/PID" hub_loopback
    for tool in "${REQUIRED_TOOLS[@]}"; do
        point "$SLOT: Werkzeug $tool auffindbar" as_slot 'command -v "$1" >/dev/null' "$tool"
    done
done
if [[ $MODE == dry-run ]]; then
    note 'DRY-RUN: nichts geprüft oder geändert; keine OK-Aussagen über die echte Maschine.'
else
    note "Prüfung beendet: $FAILURES Fehler. Sessionlimits / Widerruf / Quoteüberschreitung separat mit Mock abnehmen."
fi
(( FAILURES == 0 ))
