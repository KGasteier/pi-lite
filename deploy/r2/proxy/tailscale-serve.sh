#!/usr/bin/env bash
set -euo pipefail
set +x
MODE=dry-run
case ${1:---dry-run} in
    --dry-run) ;;
    --ausfuehren) MODE=ausfuehren ;;
    *) printf 'Bitte --dry-run oder --ausfuehren wählen.\n' >&2; exit 1 ;;
esac
(($# <= 1)) || exit 1
printf '%s\n' 'Voraussetzung: geprüfter Caddy auf 127.0.0.1:8799 mit routes.caddy; separate Netzfreigabe.'
printf '%s\n' 'N5-Sperren kommen aus routes.caddy, auch unter /slot/accountNN/: /v1/belegung, /v1/freigabe, /v1/sync/dienst/*.'
printf '%s\n' 'Befehl: tailscale serve --bg --https=443 http://127.0.0.1:8799'
printf '%s\n' 'Kein Funnel. Bestehende Serve-Konfiguration vorher administrativ prüfen, nicht überschreiben.'
if [[ $MODE == ausfuehren ]]; then
    [[ ${COCOPI_NETZFREIGABE:-} == ja ]] || { printf 'Netzfreigabe fehlt.\n' >&2; exit 1; }
    tailscale serve --bg --https=443 http://127.0.0.1:8799
fi
