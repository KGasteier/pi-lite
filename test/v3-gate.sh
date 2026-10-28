#!/usr/bin/env bash
# Merge-Gate: bash test/v3-gate.sh <worktree> [hub-worktree ...]
# Alternativ/zusätzlich: V3_GATE_HUBS="/pfad/hub-a /pfad/hub-b" (ohne Leerzeichen in Pfaden).
# Logs bleiben unter /tmp; nur eine Ergebniszeile je Gate auf stdout.
set -euo pipefail
usage() { echo "Aufruf: $0 <worktree> [hub-worktree ...]" >&2; exit 2; }
[[ $# -ge 1 ]] || usage
worktree=$(cd -- "$1" && pwd -P) || usage
git -C "$worktree" rev-parse --verify HEAD >/dev/null 2>&1 || usage
[[ -f "$worktree/copi/Cargo.toml" ]] || usage
shift
hubs=()
env_hubs=()
read -r -a env_hubs <<< "${V3_GATE_HUBS:-}"
for candidate in "$@" "${env_hubs[@]}"; do
    hub=$(cd -- "$candidate" && pwd -P) || usage
    git -C "$hub" rev-parse --verify HEAD >/dev/null 2>&1 || usage
    # Auch reine Empfang-/Sync-Worktrees sind zulässig. Doppelte nur einmal prüfen.
    duplicate=0
    for previous in "${hubs[@]}"; do [[ "$hub" != "$previous" ]] || duplicate=1; done
    (( duplicate )) || hubs+=("$hub")
done
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
for tool in cargo rustc rustfmt node python3 timeout git; do
    command -v "$tool" >/dev/null || { echo "Fehlt: $tool" >&2; exit 2; }
done
name=$(basename -- "$worktree")
name=${name//[^a-zA-Z0-9_.-]/_}
# Absichtlich nicht aus CARGO_TARGET_DIR: kein versehentlich geteilter Target-Cache.
target="$(dirname -- "$worktree")/target-$name"
mkdir -p -- "$target"
lock="$target/.v3-gate-lock"
mkdir -- "$lock" 2>/dev/null || { echo "Gate bereits aktiv / Lock prüfen: $lock" >&2; exit 2; }
trap 'rmdir -- "$lock"' EXIT
logs=$(mktemp -d "${TMPDIR:-/tmp}/v3-gate.$name.XXXXXXXX")
mkdir -p "$logs/home" "$logs/tmp" "$logs/config" "$logs/cache" "$logs/data" "$logs/agent" "$logs/sessions"
# Toolchain-Caches bleiben lesbar; alle Anwendungsdaten sind isoliert.
original_home=${HOME:?}
safe_env=(env -i "PATH=$PATH" "HOME=$logs/home" "USERPROFILE=$logs/home"
    "TMPDIR=$logs/tmp" "XDG_CONFIG_HOME=$logs/config" "XDG_CACHE_HOME=$logs/cache"
    "XDG_DATA_HOME=$logs/data" "PI_CODING_AGENT_DIR=$logs/agent"
    "PI_SESSIONS_DIR=$logs/sessions" "COPI_STATE_MIRROR_DIR=$logs/cache/copi/agentscope-remote"
    "CARGO_HOME=${CARGO_HOME:-$original_home/.cargo}" "RUSTUP_HOME=${RUSTUP_HOME:-$original_home/.rustup}"
    "CARGO_TARGET_DIR=$target" "CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3}"
    CARGO_NET_OFFLINE=true CARGO_TERM_COLOR=never RUSTUP_AUTO_INSTALL=0
    "PI_HUB_DIR=$logs/no-hub" HUB_HOST=127.0.0.1 HUB_TOKEN=v3-gate-mock HUB_STATE=off
    "HUB_ROOTS=$logs/tmp" "HUB_PI_BIN=$logs/no-real-pi" "LC_ALL=C.UTF-8"
    "I2_SEMINAR_DIR=${I2_SEMINAR_DIR:-$worktree}")
failed=0
run() {
    local id=$1 dir=$2 rc=0; shift 2
    (cd -- "$dir" && "${safe_env[@]}" timeout --kill-after=10s "${V3_GATE_TIMEOUT:-1800}s" "$@") >"$logs/$id.log" 2>&1 || rc=$?
    printf '%s\n' "$rc" >"$logs/$id.exit"
    if (( rc == 0 )); then
        if [[ "$id" == js-tests ]] && grep -q '^KNOWN ' "$logs/$id.log"; then
            printf 'KNOWN %s | bekannte Basisfehler, keine neuen Fehler; log=%s\n' "$id" "$logs/$id.log"
        elif [[ "$id" == i2-* ]] && grep -Eq '^# (skipped|todo) [1-9][0-9]*$' "$logs/$id.log"; then
            printf 'SKIP %s | I2-Abnahme unvollständig; log=%s\n' "$id" "$logs/$id.log"
        elif [[ "$id" == fmt ]]; then
            printf 'PASS %s | %s log=%s\n' "$id" "$(head -n 1 "$logs/$id.log")" "$logs/$id.log"
        else
            printf 'PASS %s | log=%s\n' "$id" "$logs/$id.log"
        fi
    else
        printf 'FAIL %s | exit=%s log=%s\n' "$id" "$rc" "$logs/$id.log"
        failed=1
    fi
}
# Provenienz, keine Credentials oder vollständige Prozessumgebung loggen.
{
    date -Is
    git -C "$worktree" rev-parse HEAD
    git -C "$worktree" status --short
    "${safe_env[@]}" cargo --version
    "${safe_env[@]}" rustc --version
    "${safe_env[@]}" node --version
    printf 'worktree=%s\ntarget=%s\n' "$worktree" "$target"
    for hub in "${hubs[@]}"; do
        printf 'hub=%s\n' "$hub"
        git -C "$hub" rev-parse HEAD
        git -C "$hub" status --short
    done
} >"$logs/context.log" 2>&1
run fmt "$worktree" python3 "$script_dir/gate-checks.py" fmt "$worktree" "${V3_GATE_BASE:-cocopi-v3}"

# JSON-Diagnosen zählen, nicht die menschlichen „generated N warnings“-Summen.
# Identische Warnungen beider include!(main.rs)-Binaries werden nur einmal gezählt.
clippy_rc=0
(cd -- "$worktree/copi" && "${safe_env[@]}" timeout --kill-after=10s "${V3_GATE_TIMEOUT:-1800}s" \
    cargo clippy --offline --locked --workspace --features copi/cocopi --message-format=json) >"$logs/clippy.log" 2>&1 || clippy_rc=$?
printf '%s\n' "$clippy_rc" >"$logs/clippy.exit"
metrics_rc=0
python3 "$script_dir/gate-metrics.py" "$logs/clippy.log" "$script_dir/clippy-baseline.json" >"$logs/clippy-metrics.json" 2>"$logs/clippy-metrics.err" || metrics_rc=$?
printf '%s\n' "$metrics_rc" >"$logs/clippy-metrics.exit"
if (( clippy_rc == 0 && metrics_rc == 0 )); then
    printf 'PASS clippy | %s log=%s\n' "$(<"$logs/clippy-metrics.json")" "$logs/clippy.log"
else
    printf 'FAIL clippy | exit=%s metrics-exit=%s %s log=%s\n' "$clippy_rc" "$metrics_rc" "$(<"$logs/clippy-metrics.json")" "$logs/clippy.log"
    failed=1
fi
run rust-tests "$worktree/copi" cargo test --offline --locked --workspace --features copi/cocopi
if [[ -f "$worktree/copi/crates/pi-sync/Cargo.toml" ]]; then
    run sync-rust-tests "$worktree/copi" cargo test --offline --locked -p pi-sync
else
    printf 'SKIP sync-rust-tests | copi/crates/pi-sync/Cargo.toml nicht vorhanden\n'
fi
run copi-check "$worktree/copi" cargo check --offline --locked -p copi --bin copi
# nullglob + explizite Leerprüfung verhindert ein stilles Grün ohne Prüflinge.
run js-syntax "$worktree" bash -c '
    shopt -s nullglob; files=(ui/*.js); ((${#files[@]})) || exit 2
    failed=0; for f in "${files[@]}"; do echo "CHECK $f"; node --check "$f" || failed=1; done
    echo "FILES ${#files[@]}"; exit "$failed"'
run js-tests "$worktree" python3 "$script_dir/gate-checks.py" js "$worktree" "$script_dir/js-baseline.txt"
if compgen -G "$worktree/mobile/test/*.mjs" >/dev/null; then
    run mobile-tests "$worktree" bash -c 'files=(mobile/test/*.mjs); node --test "${files[@]}"'
else
    printf 'SKIP mobile-tests | keine mobile/test/*.mjs vorhanden\n'
fi
if [[ -f "$worktree/deploy/r2/test_scripts.py" ]]; then
    run r2-tests "$worktree" python3 deploy/r2/test_scripts.py
else
    printf 'SKIP r2-tests | deploy/r2/test_scripts.py nicht vorhanden\n'
fi
if ((${#hubs[@]})); then
    index=0
    for hub in "${hubs[@]}"; do
        index=$((index + 1))
        suffix=""; (( index == 1 )) || suffix="-$index"
        if [[ -f "$hub/test/smoke.mjs" && -x "$hub/test/fake-pi.mjs" ]]; then
            # smoke.mjs nutzt Mock/fake-pi, keine Modelle.
            run "hub-smoke$suffix" "$hub" node test/smoke.mjs
        else
            printf 'SKIP hub-smoke%s | smoke.mjs oder ausführbares fake-pi.mjs fehlt: %s\n' "$suffix" "$hub"
        fi
        if compgen -G "$hub/empfang/test/*.mjs" >/dev/null; then
            run "hub-empfang$suffix" "$hub" bash -c '
                failed=0
                for file in empfang/test/*.mjs; do echo "CHECK $file"; node "$file" || failed=1; done
                exit "$failed"'
        else
            printf 'SKIP hub-empfang%s | keine empfang/test/*.mjs: %s\n' "$suffix" "$hub"
        fi
        if [[ -f "$hub/test/sync.mjs" ]]; then
            run "hub-sync$suffix" "$hub" node test/sync.mjs
        else
            printf 'SKIP hub-sync%s | test/sync.mjs fehlt: %s\n' "$suffix" "$hub"
        fi
        if compgen -G "$worktree/test/i2/*.test.mjs" >/dev/null; then
            run "i2-szenarien$suffix" "$worktree" env "I2_HUB_DIR=$hub" bash -c 'node --test test/i2/*.test.mjs'
        else
            printf 'SKIP i2-szenarien%s | keine test/i2/*.test.mjs vorhanden\n' "$suffix"
        fi
    done
else
    printf 'SKIP hub-smoke | kein Hub-Worktree angegeben; Rust-Hubtest ebenfalls isoliert übersprungen\n'
    printf 'SKIP hub-empfang | kein Hub-Worktree angegeben\n'
    printf 'SKIP hub-sync | kein Hub-Worktree angegeben\n'
    printf 'SKIP i2-szenarien | kein Hub-Worktree angegeben\n'
fi
exit "$failed"
