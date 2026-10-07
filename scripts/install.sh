#!/usr/bin/env bash
# Installiert pi-lite für den aktuellen Nutzer (kein sudo nötig).
# Voraussetzung: bereits gebaut (cd src-tauri && cargo build --release).
set -euo pipefail
cd "$(dirname "$0")/.."
BIN=src-tauri/target/release/pi-lite
[ -x "$BIN" ] || { echo "Binary fehlt – erst bauen: cd src-tauri && cargo build --release" >&2; exit 1; }
PREFIX="${PREFIX:-$HOME/.local}"
install -Dm755 "$BIN" "$PREFIX/lib/pi-lite/pi-lite"
# Desktop sessions may not inherit mise/npm's interactive-shell PATH.
PI_BIN="$(command -v pi || true)"
PI_DIR="$(dirname "${PI_BIN:-/usr/bin/pi}")"
mkdir -p "$PREFIX/bin"
printf '#!/usr/bin/env bash\nexport PATH=%q:"$PATH"\nif [ -f "${XDG_CONFIG_HOME:-$HOME/.config}/pi-lite/env.sh" ]; then source "${XDG_CONFIG_HOME:-$HOME/.config}/pi-lite/env.sh"; fi\nexec %q "$@"\n' "$PI_DIR" "$PREFIX/lib/pi-lite/pi-lite" > "$PREFIX/bin/pi-lite"
chmod 755 "$PREFIX/bin/pi-lite"
install -Dm644 packaging/pi-lite.png "$PREFIX/share/icons/hicolor/256x256/apps/pi-lite.png"
install -Dm644 packaging/pi-lite.desktop "$PREFIX/share/applications/pi-lite.desktop"
python3 - "$PREFIX" <<'PY'
import sys
from pathlib import Path
prefix = Path(sys.argv[1])
p = prefix / 'share/applications/pi-lite.desktop'
s = p.read_text().replace('Exec=pi-lite', 'Exec="' + str(prefix / 'bin/pi-lite') + '"')
p.write_text(s)
PY
command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
echo "Installiert nach $PREFIX (bin/pi-lite, Icon, .desktop)."
# Opt-out: PI_LITE_SETUP_SPEECH=0; adaptiert ausschließlich die bekannte vorhandene Extension.
if [ "${PI_LITE_SETUP_SPEECH:-1}" = 1 ]; then
  if command -v python3 >/dev/null; then python3 scripts/setup-speech.py;
  else echo "Sprache: python3 fehlt; Einrichtung übersprungen."; fi
fi
