#!/usr/bin/env bash
# Dialogbegleiter: zeigt erst einen lesbaren Plan. Keine Paket-/Mount-/Netzänderungen.
set -euo pipefail
set +x
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
parse_args einrichten "$@"
step() { printf '\n%s. %s\n' "$1" "$2"; }
step 1 'Freigaben und Bestandsprüfung'
note 'D-R1: vor Ausführung menschliche Freigabe, zuerst nur account01,account02.'
note 'Voraussetzungen: rootverwaltete Skript-/Hubkopie, gemeinsame Werkzeuge, aktive ext4-Quoten, authentik, Empfangsschlüssel.'
note 'Bestehende User-Hubs dürfen nicht automatisch migriert oder neu gestartet werden.'
if [[ $MODE == apply ]]; then
    [[ ${COCOPI_D_R1:-} == freigegeben ]] || fail 'D-R1 fehlt. Nach Freigabe COCOPI_D_R1=freigegeben setzen.'
    [[ ${COCOPI_POOL_ABNAHME:-} == bestaetigt ]] || fail 'S3/S7-Integration (sudo, Leerbeleg, Poolstatus, Archiv) nicht abgenommen; kein Deployment.'
    [[ $BASE == /usr/local/lib/cocopi-r2 ]] || fail 'Nur die geprüfte Kopie /usr/local/lib/cocopi-r2 ausführen.'
    root_controlled "$BASE/einrichten.sh" && root_controlled "$BASE/cocopi-slotctl" || fail 'Skripte nicht rootverwaltet.'
    require_commands visudo useradd getent systemctl python3 findmnt quotaon openssl
    check_mounts
    check_hub_contract
    [[ ${HUB_EMPFANG_PUBKEY:-} =~ ^[A-Za-z0-9_-]{43}$ ]] || fail 'Empfang-Pubkey fehlt (raw/base64url, 43 Zeichen).'
    root_controlled /etc/cocopi/empfang.env || fail 'Empfang-Konfiguration fehlt. AUTHENTIK.md abarbeiten.'
    root_controlled "$HUB_DIR/empfang/empfang.mjs" || fail 'Geprüfter Empfangseinstieg fehlt; mit S7 abstimmen.'
    [[ $(stat -c '%a' /etc/cocopi/empfang.env) == 600 ]] || fail 'empfang.env muss root:root 0600 sein.'
    visudo -cf "$BASE/sudoers/cocopi-slotctl" >/dev/null
    # Noch keine Änderung: alte aktive/aktivierte User-Units blockieren Migration.
    for slot in "${SELECTED[@]}"; do
        slot_vars "$slot"
        if [[ -n $UID_NUM ]]; then
            assert_account
            if user_systemctl is-active --quiet "$UNIT" 2>/dev/null || user_systemctl is-enabled --quiet "$UNIT" 2>/dev/null; then
                fail "Alte User-Unit für $slot gefunden. Gesonderte Migration nach Check-out nötig."
            fi
            [[ ! -e /mnt/data/cocopi/$slot ]] || fail "Alter Datenpfad für $slot vorhanden. Nicht automatisch übernehmen."
        fi
    done
else
    note 'PLAN: Quoten, Hubvertrag, OIDC-Werte, Schlüsselfingerabdruck und alte Units prüfen; bei Unklarheit stoppen.'
fi
step 2 'Empfangskonto und private Ablage'
if [[ $MODE == dry-run ]]; then
    note 'PLAN: Systemkonto cocopi-empfang ohne Login und ohne Home aus /etc/skel, Zustand /var/lib/cocopi-empfang (0700).'
elif ! getent passwd cocopi-empfang >/dev/null; then
    useradd --system --user-group --no-create-home --home-dir /var/lib/cocopi-empfang --shell /usr/sbin/nologin cocopi-empfang
else
    [[ $(getent passwd cocopi-empfang | cut -d: -f6-7) == /var/lib/cocopi-empfang:/usr/sbin/nologin &&
       $(id -u cocopi-empfang) -lt 1000 && $(id -gn cocopi-empfang) == cocopi-empfang &&
       $(id -nG cocopi-empfang) == cocopi-empfang ]] || fail 'Bestehendes Empfangskonto weicht ab.'
fi
managed_dir /var/lib/cocopi-empfang 700 cocopi-empfang cocopi-empfang
managed_dir /var/lib/cocopi-empfang/abholfach 700 cocopi-empfang cocopi-empfang
managed_dir /var/lib/cocopi-slots 700 root root
step 3 'Leeres Grundgerüst und eng begrenzter Root-Helfer'
managed_dir /etc/cocopi 755 root root
managed_dir /etc/cocopi/skel 755 root root
for relative in arbeit arbeit/Projekte arbeit/Ergebnisse .pi .pi/agent .pi/agent/skills .pi/agent/sessions; do
    managed_dir "/etc/cocopi/skel/$relative" 755 root root
 done
# Skelett nur leere Verzeichnisse; keine Providerdaten und keine Shell-Anweisungen.
managed_dir /usr/local/sbin 755 root root
managed_dir /etc/sudoers.d 750 root root
managed_file /usr/local/sbin/cocopi-slotctl 755 root root < "$BASE/cocopi-slotctl"
managed_file /etc/sudoers.d/cocopi-slotctl 440 root root < "$BASE/sudoers/cocopi-slotctl"
[[ $MODE != apply ]] || visudo -cf /etc/sudoers >/dev/null
step 4 'Poolkonten, Teilquoten und System-Hubs'
if [[ $MODE == dry-run ]]; then
    bash "$BASE/provision.sh" --dry-run --slots "$SLOT_LIST"
else
    bash "$BASE/provision.sh" --ausfuehren --slots "$SLOT_LIST"
fi
step 5 'Empfang-Unit und begrenzte Protokollablage'
managed_file /etc/systemd/system/cocopi-empfang.service 644 root root < "$BASE/systemd/cocopi-empfang.service"
managed_dir /var/log/cocopi 700 cocopi-empfang cocopi-empfang
managed_dir /etc/logrotate.d 755 root root
managed_file /etc/logrotate.d/cocopi 644 root root < "$BASE/logrotate/cocopi"
note 'Nur inhaltsfreie Betriebsmetadaten; Empfang muss Abholfach-Löschung nach 72 h selbst erzwingen (S7).'
step 6 'Proxy bewusst wählen (separate Netzfreigabe)'
note 'Caddy-Vorlagen unter proxy/ prüfen. Alternative: tailscale-serve.sh zeigt nur den TLS-Proxyweg.'
note 'Kein Proxy wird hier installiert, gestartet, umkonfiguriert oder öffentlich geöffnet.'
step 7 'Leeren und erstmaliger Start nur nach bestätigtem leerem Bestand'
if [[ $MODE == apply ]]; then
    systemctl daemon-reload
    for slot in "${SELECTED[@]}"; do
        # Kein implizites Löschen bei Einrichtung! Erst separater bestätigter Checkout.
        /usr/local/sbin/cocopi-slotctl --ausfuehren pruefen "$slot"
        systemctl enable --now "cocopi-hub@$slot.service"
    done
    systemctl enable --now cocopi-empfang.service
else
    note 'PLAN: daemon-reload; pro Slot pruefen (keine Löschung!), dann System-Hub und Empfang nur falls neu starten.'
fi
step 8 'Abnahme: erst Vorschau, echte Prüfung getrennt bestätigen'
note "PLAN: bash $BASE/admin-check.sh --dry-run --slots $SLOT_LIST"
note "DANACH separat freigeben: bash $BASE/admin-check.sh --ausfuehren --slots $SLOT_LIST --proxy-url https://NAME"
note 'Kein grüner Befund ohne Poolstatus, aktuellen Leer-Nachweis, Quoten, Empfang und echte Proxy-Sperren.'
