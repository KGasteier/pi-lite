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
    [[ ${HUB_EMPFANG_PUBKEY:-} =~ ^[A-Za-z0-9_-]{43}$ ]] || fail 'Öffentlicher Empfangsschlüssel fehlt (raw Ed25519/base64url, 43 Zeichen).'
    root_controlled /etc/cocopi/skel || fail 'Geprüftes Skelett fehlt.'
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
            [[ $(loginctl show-user "$USER_NAME" -p Linger --value 2>/dev/null || true) != yes ]] || fail "Linger für $SLOT aktiv; Migration separat nötig."
            [[ ! -e /mnt/data/cocopi/$SLOT ]] || fail "Alter I1-Datenpfad für $SLOT vorhanden."
            [[ -f $STATE_BASE/$SLOT.uid && $(< "$STATE_BASE/$SLOT.uid") == "$UID_NUM" ]] || fail "UID-Inventar für $SLOT fehlt / weicht ab. Keine Übernahme fremder Konten."
            python3 "$BASE/epoch-read.py" "$SLOT" >/dev/null || fail "Epoche für $SLOT fehlt / unsicher; Migration vor allen Änderungen klären."
        else
            [[ ! -e $SLOT_HOME && ! -L $SLOT_HOME && ! -e $SLOT_DATA && ! -L $SLOT_DATA ]] || fail "Verwaiste Slotpfade bei $SLOT; erst Recovery prüfen."
            [[ ! -e $STATE_BASE/$SLOT.uid ]] || fail "Inventar für gelöschten Slot $SLOT vorhanden. Erst Rückbau / Wiedervergabe abschließen."
            if getent passwd "$EXPECTED_UID" >/dev/null || getent group "$EXPECTED_UID" >/dev/null; then
                fail "Reservierte UID/GID $EXPECTED_UID für $SLOT bereits belegt; keine automatische Umnummerierung."
            fi
            if getent group "$USER_NAME" >/dev/null; then
                fail "Private Gruppe $USER_NAME bereits vorhanden; erst zuordnen."
            fi
        fi
    done
}

if [[ $MODE == apply ]]; then
    # Noch vor Verwaltungspfaden sicherstellen: kein stiller Pool-/I1-Bypass.
    [[ ${COCOPI_D_R1:-} == freigegeben && ${COCOPI_POOL_ABNAHME:-} == bestaetigt ]] || fail 'Poolausführung nur nach D-R1 und dokumentierter S3/S7-Abnahme.'
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
# Öffentlich lesbare Zähler; nur root darf sie erhöhen. Nie beim Wiederlauf nullen.
managed_dir /var/lib/cocopi 755 root root
managed_dir /var/lib/cocopi/slots 755 root root
managed_dir /usr/local/libexec 755 root root
managed_dir /etc/systemd/system 755 root root
managed_dir /etc/sudoers.d 750 root root

managed_file /usr/local/libexec/cocopi-hub 755 root root < "$BASE/hub-launch.sh"
managed_file /etc/systemd/system/cocopi-hub@.service 644 root root < "$BASE/systemd/cocopi-hub@.service"
managed_file /etc/systemd/system/cocopi.slice 644 root root < "$BASE/systemd/cocopi.slice"

for slot in "${SELECTED[@]}"; do
    slot_vars "$slot"
    note "--- $SLOT ($USER_NAME), Port $PORT ---"
    NEW_ACCOUNT=false
    if [[ -z $UID_NUM ]]; then
        NEW_ACCOUNT=true
        # Home sofort 0700 erstellen lassen, niemals für einen Moment aus /etc/skel 0755.
        run groupadd --gid "$EXPECTED_UID" "$USER_NAME"
        run useradd --create-home --uid "$EXPECTED_UID" --gid "$USER_NAME" --groups "$SLOT_GROUP" --shell /bin/bash \
            --home-dir "$SLOT_HOME" --comment "cocopi managed slot $SLOT" \
            --key UMASK=077 --key HOME_MODE=0700 --skel /etc/cocopi/skel "$USER_NAME"
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
    # Nur frische Konten erhalten das geprüfte Skelett; niemals Bestandsdaten überdecken.
    if [[ $MODE == dry-run ]]; then
        note "PLAN: bei neuem Konto Skelett /etc/cocopi/skel als $USER_NAME übernehmen (ohne Links)"
    elif [[ $NEW_ACCOUNT == true ]]; then
        runuser -u "$USER_NAME" -- cp -R --no-dereference -- /etc/cocopi/skel/. "$SLOT_HOME/"
    fi
    # Register wird erst beim System-Hubstart als Slotkonto erzeugt.
    # Vorher bleibt das Konto ausschließlich Skelett, damit pruefen möglich ist.
    managed_dir "$SLOT_ETC" 755 root root
    # Root schreibt nie in vom Slot beschreibbare Home-Unterverzeichnisse.
    # Die Werkzeugumgebung für Login-Shells kommt aus /etc/profile.d.
    managed_dir /etc/profile.d 755 root root
    managed_file /etc/profile.d/cocopi-slots.sh 644 root root <<PROFILE
# Für persönliche cocopi-Slotkonten; kein Zugriff auf fremde Home-Toolchains.
case \$(id -un) in
    account0[1-9]|account10)
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
    managed_file "/etc/systemd/system/cocopi-$SLOT.slice" 644 root root < "$BASE/systemd/cocopi-account.slice.in"
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
    if [[ $MODE == dry-run ]]; then
        note "PLAN: /var/lib/cocopi/slots/$SLOT.epoche root:root 0644; nur bei neuem Konto atomar mit 0 anlegen, sonst prüfen"
    elif [[ ! -e /var/lib/cocopi/slots/$SLOT.epoche && ! -L /var/lib/cocopi/slots/$SLOT.epoche && $NEW_ACCOUNT == true ]]; then
        managed_file "/var/lib/cocopi/slots/$SLOT.epoche" 644 root root <<< 0
    else
        python3 "$BASE/epoch-read.py" "$SLOT" >/dev/null || fail "Epoche für $SLOT fehlt / unsicher; separate Migration nötig, niemals automatisch zurücksetzen."
    fi
    managed_file "/etc/cocopi/hub-$SLOT.env" 600 root root <<ENV
HUB_SLOT=$SLOT
HUB_HOST=$HUB_BIND
HUB_PORT=$PORT
HUB_NODE=$HUB_NODE
HUB_DIR=$HUB_DIR
HUB_PI_BIN=$HUB_PI
HUB_ROOTS=$SLOT_HOME/arbeit
HUB_EMPFANG_PUBKEY=${HUB_EMPFANG_PUBKEY:-BITTE_EINTRAGEN}
HUB_EPOCHE_DATEI=/var/lib/cocopi/slots/$SLOT.epoche
HUB_STATE=$HUB_STATE
HUB_MAX_OPEN_SESSIONS=$HUB_MAX_OPEN_SESSIONS
HUB_MAX_ACTIVE_SESSIONS=$HUB_MAX_ACTIVE_SESSIONS
HUB_GLOBAL_HEAVY_JOBS=$HUB_GLOBAL_HEAVY_JOBS
PATH=$TOOL_PATH
TMPDIR=/tmp
DO_NOT_TRACK=1
ENV
    if [[ $MODE == dry-run ]]; then
        note "PLAN: Hub-Umgebung: HUB_SLOT=$SLOT HUB_MAX_OPEN_SESSIONS=$HUB_MAX_OPEN_SESSIONS HUB_MAX_ACTIVE_SESSIONS=$HUB_MAX_ACTIVE_SESSIONS HUB_GLOBAL_HEAVY_JOBS=$HUB_GLOBAL_HEAVY_JOBS HUB_STATE=$HUB_STATE (Verzeichnis 0700)"
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
        note "PLAN: kein Linger, kein Usermanager; nur System-Hubs starten nach Leerprüfung"
        note "PLAN: System-Unit $UNIT nur aktivieren; Start erst nach Leer-Nachweis in einrichten.sh; KEIN automatischer Restart"
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
        if [[ $(loginctl show-user "$USER_NAME" -p Linger --value 2>/dev/null || true) == yes ]]; then
            fail "Linger für $USER_NAME aktiv; alte Userdienst-Migration separat freigeben."
        fi
        if ! systemctl is-enabled --quiet "$UNIT"; then systemctl enable "$UNIT"; fi
        note "Hub noch nicht starten: erst cocopi-slotctl pruefen $SLOT; einrichten.sh führt diesen Nachweis aus."
    fi
    note "Geplant/gesetzt: $SLOT; Abnahme separat mit check.sh. Kein Geräte-Enrollment und keine Pilotfreigabe."
done
