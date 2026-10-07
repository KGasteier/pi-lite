#!/usr/bin/env bash
# Omarchy/Arch: prerequisites, architecture-native build and user installation.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v pi >/dev/null || { echo 'pi fehlt im PATH. Bitte zuerst pi installieren und konfigurieren.' >&2; exit 1; }
command -v pacman >/dev/null || { echo 'Dieser Setup-Installer unterstützt Omarchy/Arch Linux.' >&2; exit 1; }
packages=(base-devel pkgconf webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg python nodejs desktop-file-utils)
missing=()
for package in "${packages[@]}"; do pacman -Q "$package" >/dev/null 2>&1 || missing+=("$package"); done
if ((${#missing[@]})); then
  printf 'Benötigte Systempakete: %s\n' "${missing[*]}"
  sudo pacman -S --needed "${missing[@]}"
fi
if ! command -v cargo >/dev/null; then
  if command -v mise >/dev/null; then
    mise install rust@stable
    export PATH="$(mise where rust@stable)/bin:$PATH"
  else
    sudo pacman -S --needed rust
  fi
fi
node --check ui/app.js
node scripts/test-history.cjs
node scripts/test-preview.cjs
(cd src-tauri && cargo test --locked && cargo build --release --locked)
./scripts/install.sh
printf '\nFertig: pi-lite im Omarchy-App-Launcher suchen. pi behält seine bestehende Konfiguration.\n'
