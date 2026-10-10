#!/usr/bin/env bash
# copi für den aktuellen Benutzer installieren; keine laufenden Fenster beenden.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
if [[ ${1:-} != --skip-build ]]; then
  command -v cargo >/dev/null || { echo 'Rust/Cargo fehlt (rustup installieren).' >&2; exit 1; }
  (cd "$root" && CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} cargo build --release --locked)
fi
[[ -x "$root/target/release/copi" ]] || { echo 'Release-Binary fehlt.' >&2; exit 1; }
command -v pi >/dev/null || echo 'Hinweis: pi fehlt im PATH. Demo funktioniert, Agentenbetrieb noch nicht.' >&2
config=${XDG_DATA_HOME:-$HOME/.local/share}
mkdir -p "$HOME/.local/bin" "$config/icons/hicolor/scalable/apps" "$config/applications"
install -m755 "$root/target/release/copi" "$HOME/.local/bin/copi.new"
mv -f "$HOME/.local/bin/copi.new" "$HOME/.local/bin/copi"
install -m644 "$root/app/assets/copi.svg" "$config/icons/hicolor/scalable/apps/copi.svg"
# Desktop-Exec-Pfade zitieren (Leerzeichen im HOME erlaubt).
exe=${HOME//\\/\\\\}; exe=${exe//\"/\\\"}; exe=${exe//\$/\\\$}; exe=${exe//\`/\\\`}
# Prozentzeichen sind Feldcodes im Desktop-Exec.
exe=${exe//%/%%}
printf '[Desktop Entry]\nType=Application\nName=copi\nGenericName=KI-Agent\nComment=Native Oberfläche für pi\nExec="%s/.local/bin/copi"\nIcon=copi\nTerminal=false\nCategories=Development;X-Agent;\nKeywords=agent;KI;AI;pi;copi;\nStartupWMClass=copi\n' "$exe" > "$config/applications/copi.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$config/applications" || true
printf 'Installiert: %s/.local/bin/copi\nBitte laufendes copi schließen und neu starten.\n' "$HOME"
