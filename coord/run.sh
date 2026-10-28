#!/usr/bin/env bash
# Startet einen v3-Subagenten losgelöst: run.sh <rolle> <auftrag-id> <modell> [timeout-s]
# Prompt = prompts/common.md + prompts/<auftrag-id>.md. Log/Status in runs/.
set -u
R=$1; T=$2; MODEL=$3; TO=${4:-7200}
C=/home/medienhaus/Work/pi-lite/coord
WT=/home/medienhaus/Work/pi-lite-wt/$R
export PATH=$HOME/.cargo/bin:$HOME/.local/share/mise/shims:$PATH
cd "$WT" || exit 1
P="$(cat "$C/prompts/common.md")

$( [ "${T#I2}" != "$T" ] || [ "${T#I3}" != "$T" ] && cat "$C/prompts/I2-common.md")

$(cat "$C/prompts/$T.md")"
st() { echo "$1" > "$C/runs/$T.status.json.new" && mv "$C/runs/$T.status.json.new" "$C/runs/$T.status.json"; }
st "{\"state\":\"running\",\"role\":\"$R\",\"model\":\"$MODEL\",\"start\":\"$(date -Is)\",\"pid\":$$}"
timeout --kill-after=30 "$TO" pi --mode json -p --no-extensions --model "$MODEL" --thinking high \
  --session-dir "$C/runs/sessions-$T" "$P" < /dev/null > "$C/runs/$T.jsonl" 2> "$C/runs/$T.err"
RC=$?
st "{\"state\":\"done\",\"rc\":$RC,\"role\":\"$R\",\"model\":\"$MODEL\",\"end\":\"$(date -Is)\"}"
