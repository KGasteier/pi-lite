#!/usr/bin/env bash
# Lehrenden-Sicht: Zustandsabfragen und explizite Diagnose-HTTPs, keine Modellaufrufe.
set -euo pipefail
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
parse_args admin-check "$@"

# Keine Sessiondateien, Home-Inhalte, Token, Journale oder Prozessargumente lesen.
# Noch keine geprüfte, inhaltsfreie Hubquelle für letzte Arbeitsaktivität (S3).
# Bis dahin ausschließlich den belegten Dienststart mit seiner Quelle benennen.
last_activity() {
    local stamp normalized
    stamp=$(hub_systemctl show "$UNIT" -p ActiveEnterTimestamp --value 2>/dev/null) || stamp=
    if [[ -n $stamp ]] && normalized=$(date --date="$stamp" '+%Y-%m-%d %H:%M:%S %Z' 2>/dev/null); then
        printf 'Dienststart %s' "$normalized"
    else
        printf '%s' 'unbekannt'
    fi
}

quota_usage() {
    local mount=$1 blocks=$2 inodes=$3 device
    [[ $(findmnt -n -o TARGET --target "$mount") == "$mount" &&
       $(findmnt -n -o FSTYPE --target "$mount") == ext4 ]] || return 1
    device=$(findmnt -n -o SOURCE --target "$mount") || return 1
    [[ -n $device ]] || return 1
    python3 "$BASE/quota-read.py" --usage "$device" "$UID_NUM" "$blocks" "$inodes"
}

admin_row() {
    local account=vorhanden service=unbekannt access=unbekannt usage=unbekannt activity
    local nvme data
    if [[ $MODE == dry-run ]]; then
        account='nicht geprueft'; service='nicht geprueft'; access='nicht geprueft'
        usage='nicht geprueft'; activity='nicht geprueft'
    elif [[ -z $UID_NUM ]]; then
        account=fehlt; service='kein Konto'; access='kein Konto'; activity=unbekannt
        ISSUES=$((ISSUES + 1))
    elif ! (assert_account) >/dev/null 2>&1; then
        account='bitte pruefen'; service='nicht geprueft'; access='nicht geprueft'
        activity=unbekannt
        ISSUES=$((ISSUES + 1))
    else
        # Fehlender System-Hub darf nicht als ruhende Sitzung erscheinen.
        if hub_systemctl is-active --quiet "$UNIT" 2>/dev/null; then
            service=aktiv
        else
            service='nicht aktiv'
            ISSUES=$((ISSUES + 1))
        fi
        # Dieselbe PID-/Listenerprüfung wie in der technischen Abnahme.
        if hub_loopback 2>/dev/null; then
            access='nur lokal'
        else
            access='bitte pruefen'
            ISSUES=$((ISSUES + 1))
        fi
        if nvme=$(quota_usage "$NVME_MOUNT" "$NVME_QUOTA_KIB" "$NVME_INODES" 2>/dev/null) &&
           data=$(quota_usage "$DATA_MOUNT" "$DATA_QUOTA_KIB" "$DATA_INODES" 2>/dev/null); then
            usage="NVMe $nvme; SATA $data"
            if [[ $usage == *'pruefen'* || $usage == *'voll'* ]]; then ISSUES=$((ISSUES + 1)); fi
        else
            ISSUES=$((ISSUES + 1))
        fi
        activity=$(last_activity)
    fi
    printf '%-10s  %-14s  %-14s  %-14s  %-70s  %s\n' \
        "$SLOT" "$account" "$service" "$access" "$usage" "$activity"
}

pool_checks() {
    if [[ $MODE == dry-run ]]; then
        note 'Pool: nicht geprüft. PLAN: alle zehn Slots frei / belegt / leeren / gesperrt aus aktuellem lokalem Empfangsstatus lesen.'
        note 'PLAN: freie Plätze nur mit Löschbeleg nachweisen, beide Quoten und Empfang an 127.0.0.1:8800 prüfen.'
        note 'PLAN: Proxy-Sperren direkt und unter allen zehn Slotpfaden via HTTPS testen (auch Encoding/Traversal).'
        return
    fi
    if python3 "$BASE/pool-read.py"; then :; else ISSUES=$((ISSUES + 1)); fi
    # hallo ist ein inhaltsfreier Discovery-Aufruf, keine Anmeldung/kein Modell.
    if curl --silent --fail --noproxy '*' --max-time 5 --max-filesize 65536 \
        -H 'Content-Type: application/json' -d '{"protokoll":1}' \
        http://127.0.0.1:8800/v1/empfang/hallo | python3 -c '
import json,sys
x=json.load(sys.stdin)
assert x.get("protokoll")==1 and x.get("vertrag")=="cocopi-pool-v1"
assert x.get("slots_gesamt")==10 and 0 <= x.get("slots_frei",-1) <= 10
'; then note 'Empfang: erreichbar, Poolvertrag bestätigt.'
    else note 'Empfang: nicht bestätigt.'; ISSUES=$((ISSUES + 1)); fi
    if [[ -z $PROXY_URL ]]; then
        note 'Proxy-Sperren: nicht geprüft. Bitte --proxy-url https://NAME angeben.'
        ISSUES=$((ISSUES + 1))
    elif ! python3 "$BASE/proxy-check.py" "$PROXY_URL"; then
        ISSUES=$((ISSUES + 1))
    fi
}

ISSUES=0
if [[ $MODE == apply ]]; then
    require_commands getent id stat runuser systemctl ss awk findmnt python3 date curl
    note 'R2: kurze Betriebsübersicht (Entwurf, nur gelesen). Keine Pilotfreigabe.'
else
    note 'R2: Vorschau der Betriebsübersicht. Nichts geprüft oder geändert.'
fi
pool_checks
printf '%-10s  %-14s  %-14s  %-14s  %-70s  %s\n' \
    'Konto' 'Eingerichtet' 'Verbindung' 'Zugang' 'Speicher belegt / Grenze (GB dezimal)' 'Letzte Aktivität (Quelle beachten)'
for slot in "${SELECTED[@]}"; do
    slot_vars "$slot"
    admin_row
done
if [[ $MODE == dry-run ]]; then
    note 'Plan: Konto prüfen, Verbindungsdienst abfragen, lokalen Port und Speicherbelegung lesen.'
    note 'Zum späteren Lesen braucht die Administration --apply mit Rootrechten und eine geprüfte Skriptkopie.'
else
    note 'Bei „fehlt“, „nicht aktiv“ oder „bitte pruefen“: Administration bitten, check.sh zu verwenden.'
    note '„nur lokal“: Der Port lauscht ausschließlich auf 127.0.0.1; das beweist keinen Zugang vom Mac oder Handy.'
fi
note 'Letzte Aktivität: vorerst nur Dienststart, falls bekannt; die letzte Arbeit am Hub ist noch unbekannt.'
note 'Ein aktiver Verbindungsdienst bedeutet nicht, dass jemand gerade arbeitet. Dies ist kein Anwesenheitsnachweis.'
note 'Speicherbelegung ist kein Nachweis aktiver Quoten: Die technische Abnahme bleibt erforderlich.'
note 'Es werden keine Gesprächsinhalte, Dateinamen, Gerätekennungen oder Zugangsdaten angezeigt.'
(( ISSUES == 0 ))
