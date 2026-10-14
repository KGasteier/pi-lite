#!/usr/bin/env bash
# Expliziter Upstream-Abgleich: kein Reset, kein Stash, kein Überschreiben.
set -euo pipefail
cd "$(dirname "$0")/../.."
[[ $(git branch --show-current) == cocopi ]] || { echo 'Nur auf cocopi ausführen.' >&2; exit 1; }
[[ -z $(git status --porcelain --untracked-files=no) ]] || { echo 'Tracked Änderungen zuerst committen.' >&2; exit 1; }
git -c credential.helper= -c 'credential.helper=!gh auth git-credential' fetch https://github.com/KGasteier/pi-lite.git copi
git merge --no-edit FETCH_HEAD
CARGO_BUILD_JOBS=2 cargo test --offline --locked --workspace --features cocopi --manifest-path copi/Cargo.toml
CARGO_BUILD_JOBS=2 cargo build --offline --locked --release --features cocopi --bin cocopi --manifest-path copi/Cargo.toml
# Kein automatischer Push und kein Austausch laufender Apps.
