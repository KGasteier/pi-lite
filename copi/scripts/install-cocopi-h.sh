#!/usr/bin/env bash
# Separate Hermes-Ausgabe installieren; vorhandene cocopi/cocopi-v3 bleiben erhalten.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
case ${1:-} in ''|--skip-build) ;; *) echo 'Aufruf: install-cocopi-h.sh [--skip-build]' >&2; exit 2;; esac
if [[ ${1:-} != --skip-build ]]; then
  free=$(df -Pk "$root" | awk 'NR==2 {print $4}')
  (( free >= 6*1024*1024 )) || { echo 'Mindestens 6 GiB frei für den Build erforderlich.' >&2; exit 1; }
  CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} cargo build --offline --locked --release \
    --features cocopi-h --bin cocopi-h --manifest-path "$root/Cargo.toml"
fi
source_bin="$root/target/release/cocopi-h"
[[ -x "$source_bin" ]] || { echo 'cocopi-h-Release fehlt.' >&2; exit 1; }
[[ $("$source_bin" --version) == 'cocopi-h '* ]] || { echo 'Falsche Binary-Identität.' >&2; exit 1; }
bin="$HOME/.local/bin/cocopi-h"
icon="$HOME/.local/share/icons/hicolor/scalable/apps/cocopi-h.svg"
desktop="$HOME/.local/share/applications/cocopi-h.desktop"
stamp=$(date -u +%Y%m%dT%H%M%S)-$$
mkdir -p "$(dirname "$bin")" "$(dirname "$icon")" "$(dirname "$desktop")"
for f in "$bin" "$icon" "$desktop"; do
  [[ ! -e "$f.new" ]] || { echo "Bereitliegende Datei zuerst prüfen: $f.new" >&2; exit 1; }
  if [[ -e "$f" ]]; then cp -a -- "$f" "$f.bak-$stamp"; echo "Backup: $f.bak-$stamp"; fi
done
install -m755 "$source_bin" "$bin.new"
expected=$(sha256sum "$source_bin" | awk '{print $1}')
[[ $(sha256sum "$bin.new" | awk '{print $1}') == "$expected" ]]
install -m644 "$root/app/assets/cocopi-h.svg" "$icon.new"
cat > "$desktop.new" <<EOF
[Desktop Entry]
Type=Application
Name=cocopi-h
GenericName=KI-Agent und Hermes-Steuerung
Comment=Separate cocopi-Ausgabe für lokale und entfernte Hermes-Hubs
Exec="$bin"
Icon=$icon
Terminal=false
Categories=Development;X-Agent;
Keywords=agent;KI;AI;pi;cocopi;Hermes;
StartupWMClass=cocopi-h
EOF
mv -- "$icon.new" "$icon"
mv -- "$bin.new" "$bin"
mv -- "$desktop.new" "$desktop"
[[ $(sha256sum "$bin" | awk '{print $1}') == "$expected" ]]
if command -v desktop-file-validate >/dev/null; then desktop-file-validate "$desktop"; fi
if command -v update-desktop-database >/dev/null; then update-desktop-database "$(dirname "$desktop")"; fi
printf 'Installiert: %s\nSHA256: %s\nLaunchpad: cocopi-h → Agents (X-Agent).\n' "$bin" "$expected"
echo 'Kein Fenster/Dienst gestartet. Konfiguration und Sessions bleiben die des cocopi-Kerns.'
