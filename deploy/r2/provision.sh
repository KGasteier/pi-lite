#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=deploy/r2/common.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
parse_args provision "$@"
note "Provisionierung: $MODE; Slots: $SLOT_LIST. Keine Pakete, Mountänderungen oder Modellaufrufe."

preflight() {
    require_commands useradd groupadd id getent install stat findmnt quotaon setquota \
        runuser systemctl loginctl openssl sudo visudo flock python3 numfmt
    check_mounts
    check_hub_contract
    root_controlled "$HOME_BASE" && root_controlled "$DATA_MOUNT" || fail 'Home-/SSD-Mountwurzel nicht rootkontrolliert (siehe BEFUND).'
    [[ $(stat -fc %T /sys/fs/cgroup) == cgroup2fs ]] || fail 'cgroup v2 fehlt.'
    root_controlled "$HUB_PI" && root_controlled "$HUB_NODE" || fail 'Gemeinsames pi / Node nicht rootverwaltet.'
    local tool resolved slot
    for tool in "${REQUIRED_TOOLS[@]}"; do
        resolved=$(PATH="$TOOL_PATH" command -v "$tool") || fail "Gemeinsames Werkzeug fehlt: $tool."
        [[ -x $resolved ]] && root_controlled "$(readlink -f -- "$resolved")" || fail "Werkzeug nicht rootverwaltet: $tool."
    done
    # Alle ausgewählten Bestandskonten vor irgendeiner Kontoänderung prüfen.
    for slot in "${SELECTED[@]}"; do
        slot_vars "$slot"
        [[ ! -e $STATE_BASE/$SLOT.teardown ]] || fail "Rückbau für $SLOT begonnen; zuerst diesen abschließen."
        if [[ -n $UID_NUM ]]; then
            assert_account
            [[ -f $STATE_BASE/$SLOT.uid && $(< "$STATE_BASE/$SLOT.uid") == "$UID_NUM" ]] || fail "UID-Inventar für $SLOT fehlt / weicht ab. Keine Übernahme fremder Konten."
        else
            [[ ! -e $SLOT_HOME && ! -L $SLOT_HOME && ! -e $SLOT_DATA && ! -L $SLOT_DATA ]] || fail "Verwaiste Slotpfade bei $SLOT; erst Recovery prüfen."
            [[ ! -e $STATE_BASE/$SLOT.uid ]] || fail "Inventar für gelöschten Slot $SLOT vorhanden. Erst Rückbau / Wiedervergabe abschließen."
            if getent passwd "$EXPECTED_UID" >/dev/null || getent group "$EXPECTED_UID" >/dev/null; then
                fail "Reservierte UID/GID $EXPECTED_UID für $SLOT bereits belegt; keine automatische Umnummerierung."
            fi
            getent group "$USER_NAME" >/dev/null && fail "Private Gruppe $USER_NAME bereits vorhanden; erst zuordnen."
        fi
    done
}

if [[ $MODE == apply ]]; then
    preflight
    acquire_lock
    # Zweiter Blick unter Sperre verhindert konkurrierende Kontoanlagen.
    preflight
else
    note 'PLAN: Vor Apply alle Voraussetzungen prüfen; beim aktuellen BEFUND abbrechen:'
    note "  ext4-Userquoten aktiv auf $NVME_MOUNT und $DATA_MOUNT; Quotenwerkzeuge vorhanden"
    note "  SSD-Mountwurzel rootverwaltet; gemeinsame Werkzeuge in $TOOL_PATH; Hub-Vertrag $REQUIRED_HUB_CONTRACT bestätigt"
    note "  installierter Hub: $HUB_DIR; persönliche medienhaus-Shims werden NICHT verwendet"
    note '  alle Bestandskonten und UID-Inventare prüfen; keine fremden Konten übernehmen'
fi

if ! getent group "$SLOT_GROUP" >/dev/null; then
    run groupadd --system "$SLOT_GROUP"
fi
managed_dir "$DATA_BASE" 755 root root
managed_dir /etc/cocopi/slots 755 root root
managed_dir /usr/local/libexec 755 root root
managed_dir /etc/systemd/user 755 root root
managed_dir /etc/sudoers.d 750 root root

managed_file /usr/local/libexec/cocopi-hub 755 root root < "$BASE/hub-launch.sh"
managed_file /etc/systemd/user/cocopi-hub@.service 644 root root < "$BASE/cocopi-hub@.service"

for slot in "${SELECTED[@]}"; do
    slot_vars "$slot"
    note "--- $SLOT ($USER_NAME), Port $PORT ---"
    if [[ -z $UID_NUM ]]; then
        # Home sofort 0700 erstellen lassen, niemals für einen Moment aus /etc/skel 0755.
        run groupadd --gid "$EXPECTED_UID" "$USER_NAME"
        run useradd --create-home --uid "$EXPECTED_UID" --gid "$USER_NAME" --groups "$SLOT_GROUP" --shell /bin/bash \
            --home-dir "$SLOT_HOME" --comment "cocopi managed slot $SLOT" \
            --key UMASK=077 --key HOME_MODE=0700 "$USER_NAME"
        if [[ $MODE == apply ]]; then
            UID_NUM=$(id -u "$USER_NAME")
            managed_file "$STATE_BASE/$SLOT.uid" 600 root root <<< "$UID_NUM"
        else
            UID_NUM='<UID nach Kontoanlage>'
        fi
    fi
    if [[ $MODE == apply ]]; then assert_account; fi
    managed_dir "$SLOT_HOME" 700 "$USER_NAME" "$USER_NAME"
    managed_dir "$SLOT_DATA" 700 "$USER_NAME" "$USER_NAME"
    managed_dir "$SLOT_ETC" 755 root root
    # Root schreibt nie in vom Slot beschreibbare Home-Unterverzeichnisse.
    # Die Werkzeugumgebung für Login-Shells kommt aus /etc/profile.d.
    managed_dir /etc/profile.d 755 root root
    managed_file /etc/profile.d/cocopi-slots.sh 644 root root <<PROFILE
# Für persönliche cocopi-Slotkonten; kein Zugriff auf fremde Home-Toolchains.
case \$(id -un) in
    cocopi-s0[1-9]|cocopi-s10)
        export PATH="\$HOME/.local/bin:$TOOL_PATH"
        umask 077
        ;;
esac
PROFILE
    managed_file "/etc/sudoers.d/cocopi-$SLOT" 440 root root <<SUDOERS
# Explizites Verbot; Gesamtkonfiguration zusätzlich mit sudo -l prüfen.
$USER_NAME ALL=(ALL:ALL) !ALL
SUDOERS
    if [[ $MODE == apply ]]; then
        visudo -cf /etc/sudoers >/dev/null
        # Das gesamte Listing muss verweigern; nicht nur ein ausgewählter Befehl.
        sudo_denied "$USER_NAME" || fail "Effektive sudo-Rechte für $USER_NAME nicht sicher ausgeschlossen. Nicht freigeben."
    fi
    managed_dir "/etc/systemd/system/user-$UID_NUM.slice.d" 755 root root
    managed_file "/etc/systemd/system/user-$UID_NUM.slice.d/50-cocopi.conf" 644 root root <<SLICE
[Slice]
CPUAccounting=yes
MemoryAccounting=yes
TasksAccounting=yes
CPUQuota=$CPU_QUOTA
MemoryHigh=$MEMORY_HIGH
MemoryMax=$MEMORY_MAX
TasksMax=$TASKS_MAX
SLICE
    note "Quote: $NVME_QUOTA_KIB KiB NVMe + $DATA_QUOTA_KIB KiB SATA = 80 GB dezimal; Soft=Hard."
    # setquota nur bei Abweichung; aktive Quota selbst wird hier nie verändert.
    if [[ $MODE == dry-run ]] || ! quota_matches "$NVME_MOUNT" "$NVME_QUOTA_KIB" "$NVME_INODES" 2>/dev/null; then
        run setquota -u "$USER_NAME" "$NVME_QUOTA_KIB" "$NVME_QUOTA_KIB" "$NVME_INODES" "$NVME_INODES" "$NVME_MOUNT"
    fi
    if [[ $MODE == dry-run ]] || ! quota_matches "$DATA_MOUNT" "$DATA_QUOTA_KIB" "$DATA_INODES" 2>/dev/null; then
        run setquota -u "$USER_NAME" "$DATA_QUOTA_KIB" "$DATA_QUOTA_KIB" "$DATA_INODES" "$DATA_INODES" "$DATA_MOUNT"
    fi
    if [[ $MODE == apply ]]; then
        quota_matches "$NVME_MOUNT" "$NVME_QUOTA_KIB" "$NVME_INODES" || fail "NVMe-Quote für $SLOT nach Setzen nicht bestätigt. Kein Dienststart."
        quota_matches "$DATA_MOUNT" "$DATA_QUOTA_KIB" "$DATA_INODES" || fail "SATA-Quote für $SLOT nach Setzen nicht bestätigt. Kein Dienststart."
    fi
    managed_file "$SLOT_ETC/hub.env" 644 root root <<ENV
HUB_SLOT=$SLOT
HUB_HOST=$HUB_BIND
HUB_PORT=$PORT
HUB_NODE=$HUB_NODE
HUB_DIR=$HUB_DIR
HUB_PI_BIN=$HUB_PI
HUB_ROOTS=$SLOT_HOME:$SLOT_DATA
HUB_STATE=off
HUB_MAX_OPEN_SESSIONS=$HUB_MAX_OPEN_SESSIONS
HUB_MAX_ACTIVE_SESSIONS=$HUB_MAX_ACTIVE_SESSIONS
HUB_GLOBAL_HEAVY_JOBS=$GLOBAL_HEAVY_JOBS
PATH=$TOOL_PATH
TMPDIR=$SLOT_HOME
DO_NOT_TRACK=1
ENV
    if [[ $MODE == dry-run ]]; then
        note "PLAN: $SLOT_ETC/token nur falls fehlend kryptografisch erzeugen (0600, $USER_NAME); Inhalt niemals anzeigen"
    else
        no_symlinks "$SLOT_ETC/token" || fail "Symlink an Token von $SLOT."
        if [[ ! -e $SLOT_ETC/token ]]; then
            # Token nur in rootprivatem temporärem Verzeichnis; nicht in argv/stdout/log.
            token_tmp=$(mktemp -d "$STATE_BASE/token.XXXXXXXX")
            chmod 700 "$token_tmp"
            openssl rand -hex 32 > "$token_tmp/value"
            install -m 600 -o "$USER_NAME" -g "$USER_NAME" "$token_tmp/value" "$SLOT_ETC/token"
            rm -f -- "$token_tmp/value"
            rmdir -- "$token_tmp"
        fi
        [[ -f $SLOT_ETC/token && $(stat -c '%a:%U:%G' "$SLOT_ETC/token") == "600:$USER_NAME:$USER_NAME" ]] || fail "Tokenrechte für $SLOT falsch; keine automatische Rotation."
    fi
    if [[ $MODE == dry-run ]]; then
        note "PLAN: systemd daemon-reload, bestehende Slice $UID_NUM nur bei Limitabweichung aktualisieren"
        note "PLAN: Linger für $USER_NAME nur falls fehlend einschalten; eigenen Usermanager starten falls inaktiv"
        note "PLAN: als $USER_NAME $UNIT nur falls nicht aktiviert/gestartet enable/start; KEIN automatischer Restart"
    else
        systemctl daemon-reload
        if systemctl is-active --quiet "user-$UID_NUM.slice"; then
            # Keine Laufzeitänderung, wenn die effektiven Werte schon stimmen.
            actual=$(systemctl show "user-$UID_NUM.slice" -p MemoryHigh -p MemoryMax -p TasksMax -p CPUQuotaPerSecUSec)
            wanted_cpu=$(python3 -c 'import sys; print(int(float(sys.argv[1].rstrip("%"))*10000))' "$CPU_QUOTA")
            if ! python3 "$BASE/slice-read.py" "$wanted_cpu" "$(numfmt --from=iec "$MEMORY_HIGH")" \
                "$(numfmt --from=iec "$MEMORY_MAX")" "$TASKS_MAX" <<< "$actual"; then
                systemctl set-property --runtime "user-$UID_NUM.slice" \
                    "CPUQuota=$CPU_QUOTA" "MemoryHigh=$MEMORY_HIGH" "MemoryMax=$MEMORY_MAX" "TasksMax=$TASKS_MAX"
            fi
        fi
        if [[ $(loginctl show-user "$USER_NAME" -p Linger --value 2>/dev/null || true) != yes ]]; then
            loginctl enable-linger "$USER_NAME"
        fi
        if ! systemctl is-active --quiet "user@$UID_NUM.service"; then
            systemctl start "user@$UID_NUM.service"
        fi
        user_systemctl daemon-reload
        if ! user_systemctl is-enabled --quiet "$UNIT"; then user_systemctl enable "$UNIT"; fi
        if ! user_systemctl is-active --quiet "$UNIT"; then user_systemctl start "$UNIT"; fi
    fi
    note "Geplant/gesetzt: $SLOT; Abnahme separat mit check.sh. Kein Geräte-Enrollment und keine Pilotfreigabe."
done
