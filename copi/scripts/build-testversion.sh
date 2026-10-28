#!/usr/bin/env bash
# cocopi-v3 Testversion auf DIESEM Rechner aus Quellcode bauen (ARM64/aarch64 oder x86_64, Linux/macOS).
# Es werden keine fertigen Binärdateien kopiert. Installiert nur nach ausdrücklicher Option.
#
#   bash copi/scripts/build-testversion.sh            # prüfen + bauen + Demo-Screenshot
#   bash copi/scripts/build-testversion.sh --install  # zusätzlich ~/.local/bin/cocopi (Sicherung .bak)
#
# Umgebung: CARGO_BUILD_JOBS (Standard 2), COCOPI_TARGET_DIR (Standard copi/target).
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
repo=$(cd -- "$root/.." && pwd)
install=0
for a in "$@"; do
  case "$a" in
    --install) install=1 ;;
    -h|--help) sed -n 2,8p "$0"; exit 0 ;;
    *) echo "Unbekannte Option: $a" >&2; exit 2 ;;
  esac
done

say() { printf '\n== %s\n' "$*"; }
arch=$(uname -m); os=$(uname -s)
say "Rechner: $os $arch"
branch=$(git -C "$repo" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')
head=$(git -C "$repo" rev-parse --short HEAD 2>/dev/null || echo '?')
echo "Quelle: $repo  Branch $branch @ $head"
[[ $branch == cocopi-v3 ]] || echo "Hinweis: Testversion ist für Branch cocopi-v3 gedacht (aktuell $branch)."

command -v cargo >/dev/null || { echo 'Rust/Cargo fehlt: https://rustup.rs (oder mise use rust@stable).' >&2; exit 1; }
rustc --version
if [[ $os == Linux ]]; then
  # egui/eframe brauchen zur Laufzeit Wayland/X11 + xkbcommon; beim Bauen genügt ein C-Linker.
  command -v cc >/dev/null || { echo 'C-Compiler fehlt (z. B. sudo pacman -S base-devel).' >&2; exit 1; }
fi

free_kb=$(df -Pk "$root" | awk 'NR==2{print $4}')
if (( free_kb < 6*1024*1024 )); then
  echo "Achtung: nur $((free_kb/1024/1024)) GiB frei; ein Release-Build braucht etwa 3–6 GiB." >&2
fi

target=${COCOPI_TARGET_DIR:-$root/target}
say "Baue cocopi (Release, $arch) – das dauert beim ersten Mal 5–20 Minuten"
CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} CARGO_TARGET_DIR="$target" \
  cargo build --locked --release --features cocopi --bin cocopi --manifest-path "$root/Cargo.toml"
bin="$target/release/cocopi"
[[ -x $bin ]] || { echo "Build fehlt: $bin" >&2; exit 1; }
if command -v file >/dev/null; then file "$bin"; fi
sha=$(sha256sum "$bin" 2>/dev/null | cut -c1-16 || shasum -a 256 "$bin" | cut -c1-16)
echo "SHA256 (Anfang): $sha"

say "Selbsttest ohne Fenster (Demo-Szene, kein Netz, kein pi)"
shot=${TMPDIR:-/tmp}/cocopi-v3-testversion.png
if COPI_SCENE=v3-start "$bin" --shot "$shot" >/dev/null 2>&1; then
  echo "Screenshot: $shot"
else
  echo "Screenshot nicht möglich (ohne GPU/Display normal). Das Fenster selbst ist davon nicht betroffen."
fi

if (( install )); then
  say "Installiere nach ~/.local/bin/cocopi"
  mkdir -p "$HOME/.local/bin"
  [[ -e $HOME/.local/bin/cocopi ]] && cp -p "$HOME/.local/bin/cocopi" "$HOME/.local/bin/cocopi.bak" && echo "Sicherung: ~/.local/bin/cocopi.bak"
  install -m755 "$bin" "$HOME/.local/bin/cocopi.new"
  mv -f "$HOME/.local/bin/cocopi.new" "$HOME/.local/bin/cocopi"
  "$root/scripts/check-desktop.sh" 2>/dev/null | tail -n 2 || true
  echo "Zurück zur alten Version: mv ~/.local/bin/cocopi.bak ~/.local/bin/cocopi"
else
  echo; echo "Nicht installiert. Direkt starten: $bin"
  echo "Installieren: bash copi/scripts/build-testversion.sh --install"
fi
cat <<'EOF'

Erste Schritte: cocopi starten → „Einchecken“ (Browser öffnet auth.udk.digital) → Dateien wählen → arbeiten.
Ohne eingerichteten R2-Empfang zeigt cocopi einen verständlichen Fehler; Demo ohne Server:
  COPI_SCENE=v3-start cocopi      (weitere Szenen: siehe docs/v3/TESTVERSION.md)
EOF
