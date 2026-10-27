#!/usr/bin/env bash
# cocopi separat installieren; copi/pi-lite und laufende Fenster bleiben unverändert.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
if [[ ${1:-} != --skip-build ]]; then
  command -v cargo >/dev/null || { echo 'Rust/Cargo fehlt.' >&2; exit 1; }
  CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} cargo build --locked --release --features cocopi --bin cocopi --manifest-path "$root/Cargo.toml"
fi
[[ -x "$root/target/release/cocopi" ]] || { echo 'cocopi-Release fehlt.' >&2; exit 1; }
mkdir -p "$HOME/.local/bin"
install -m755 "$root/target/release/cocopi" "$HOME/.local/bin/cocopi.new"
mv -f "$HOME/.local/bin/cocopi.new" "$HOME/.local/bin/cocopi"
# Nur Anzeige: warnt, falls cocopi irrtümlich als Terminal/Browser eingetragen ist.
"$root/scripts/check-desktop.sh" | tail -n 2 || true
printf 'Installiert: %s/.local/bin/cocopi\nStart im Terminal: ~/.local/bin/cocopi\nKein Autostart, kein Netzwerk ohne Klick.\n' "$HOME"
