# Pilotimage; kein fertiges cocopi-Containerpool-Release.
# NODE_IMAGE beim Build als gepinnten node:22-bookworm-slim@sha256:... angeben.
ARG NODE_IMAGE
FROM ${NODE_IMAGE}
ARG PI_VERSION=1.4.0
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates git python3 python3-venv build-essential ffmpeg tini \
 && rm -rf /var/lib/apt/lists/* \
 && npm install -g @earendil-works/pi-coding-agent@${PI_VERSION} \
 && npm cache clean --force
# Rust bewusst separat nach geprüfter Toolchain ergänzen; nicht curl|sh als root.
RUN install -d -o node -g node -m 700 /home/node/arbeit /home/node/.pi/agent /home/node/.hub
COPY --chown=root:root hub/ /opt/pi-hub/
COPY --chmod=755 entrypoint.sh /usr/local/bin/cocopi-entrypoint
USER 1000:1000
ENV HOME=/home/node HUB_HOST=0.0.0.0 HUB_PORT=8787 \
    HUB_ROOTS=/home/node/arbeit HUB_STATE=/home/node/.hub \
    PI_CODING_AGENT_DIR=/home/node/.pi/agent DO_NOT_TRACK=1
WORKDIR /home/node/arbeit
EXPOSE 8787
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/cocopi-entrypoint"]
CMD ["node", "/opt/pi-hub/server.mjs"]
