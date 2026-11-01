#!/usr/bin/env bash
# Compose-Generator: schreibt nur stdout. Keine Container-/Hoständerung.
set -euo pipefail
count=${1:-10}
[[ $count =~ ^([1-9]|10)$ ]] || { echo 'Anzahl 1–10' >&2; exit 1; }
cat <<'YAML'
# BETREUTER INFRASTRUKTURPILOT; kein Authentik-/cocopi-v3-Pool.
# Vor Coolify-Import IMAGE auf ein geprüftes Registryimage@sha256:... setzen.
services:
YAML
for ((i=1;i<=count;i++)); do
  slot=$(printf 'slot%02d' "$i"); port=$((18800+i))
  cat <<YAML
  $slot:
    image: \${PILOT_IMAGE:?Geprueftes Image mit Digest fehlt}
    restart: "no"
    read_only: true
    user: "1000:1000"
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
    cpus: "1.0"
    mem_limit: 2560m
    pids_limit: 256
    tmpfs:
      - /tmp:rw,nosuid,nodev,size=256m,mode=1777
    environment:
      HUB_URL: http://127.0.0.1:$port
    volumes:
      - type: bind
        source: /mnt/data/cocopi-container-pilot/$slot/home
        target: /home/node
        bind:
          create_host_path: false
      - type: bind
        source: /mnt/data/cocopi-container-pilot/$slot/hub_token
        target: /run/secrets/hub_token
        read_only: true
        bind:
          create_host_path: false
    ports:
      - "127.0.0.1:$port:8787"
    networks: [$slot]
    healthcheck:
      test: ["CMD", "node", "-e", "fetch('http://127.0.0.1:8787/api/ping',{headers:{Authorization:'Bearer '+require('node:fs').readFileSync('/run/secrets/hub_token','utf8').trim()}}).then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"]
      interval: 30s
      timeout: 5s
      retries: 3
    logging:
      driver: none
YAML
done
printf 'networks:\n'
for ((i=1;i<=count;i++)); do printf '  slot%02d: {}\n' "$i"; done
