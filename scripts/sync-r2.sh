#!/usr/bin/env bash
# Kuratierter Wissenstransfer ARM → R2; niemals Repository-Quellcode oder Konfiguration.
set -euo pipefail
mode=--dry-run
case "${1:-}" in
  '') ;;
  --dry-run|--apply) mode=$1; shift ;;
  *) printf 'Aufruf: %s [--dry-run|--apply]\n' "$0" >&2; exit 2 ;;
esac
[[ $# == 0 ]] || { echo 'Keine weiteren Argumente erlaubt.' >&2; exit 2; }
remote=medienhaus@194.95.203.142
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
chmod 700 "$stage"
# Nur geprüfte Projekt-Skills und allgemeine Entwicklungsagenten. Keine Extensions:
# deren Code/Abhängigkeiten müssen separat geprüft und ausdrücklich freigegeben werden.
python3 - "$stage" <<'PY'
import pathlib, sys, shutil, re, hashlib
home = pathlib.Path.home()
base = home / '.pi/agent'
stage = pathlib.Path(sys.argv[1])
roots = ['skills/copidev', 'skills/pi-lite-dev', 'skills/mobicopidev']
files = [base / ('agents/' + x + '.md') for x in ['planner', 'reviewer', 'scout', 'worker']]
pairs = []
for rel in roots:
    root = (base / rel).resolve(strict=True)
    # rglob folgt keinen inneren Verzeichnissymlinks. Solche Ziele müssen eigens
    # in die Allowlist aufgenommen werden; die obersten Skill-Symlinks lösen wir auf.
    for p in sorted(root.rglob('*')):
        if not p.is_file():
            continue
        suffix = p.relative_to(root)
        if any(x.startswith('.') or x in ('node_modules', '__pycache__', 'target') for x in suffix.parts):
            continue
        if p.suffix not in ('.md', '.txt', '.sh', '.py'):
            continue
        resolved = p.resolve(strict=True)
        if not resolved.is_relative_to(root):
            raise SystemExit('Abbruch: interner Symlink verlässt Skill-Verzeichnis')
        pairs.append((resolved, pathlib.Path(rel) / suffix))
for p in files:
    pairs.append((p.resolve(strict=True), p.relative_to(base)))
for source, rel in pairs:
    if re.search(r'(?i)(auth|models|settings|secret|credential|token|\.env|\.bak|backup)', rel.name):
        raise SystemExit('Abbruch: gesperrter Dateiname im Transfer')
    data = source.read_bytes()
    if len(data) > 1024 * 1024 or b'\0' in data:
        raise SystemExit('Abbruch: keine kleine Textdatei')
    # Zusätzlicher Warnschutz, KEIN vollständiger Secret-Scanner. Manuelle Prüfung bleibt Pflicht.
    if re.search(rb'(?i)(-----BEGIN (?:OPENSSH|RSA|EC|PRIVATE).*PRIVATE KEY|\bsk-[a-zA-Z0-9_-]{20,}|\bgh[pousr]_[a-zA-Z0-9]{20,})', data):
        raise SystemExit('Abbruch: möglicher Schlüssel; Inhalt wird nicht ausgegeben')
    dest = stage / rel
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, dest)  # Inhalt statt Symlink übertragen
    dest.chmod(0o600)
    print(hashlib.sha256(data).hexdigest(), str(rel))
print('Nur Text-Allowlist; Auth/models/settings/Sessions/Extensions und Quellrepos ausgeschlossen.')
PY
# Nur lesender Zielcheck, auch im Dry-Run; keine mkdir-/Restart-Kommandos.
ssh -o BatchMode=yes -o ConnectTimeout=8 "$remote" 'python3 -' <<'PY'
from pathlib import Path
base = Path.home()/'.pi/agent'
for rel in ['', 'skills', 'agents', 'skills/copidev', 'skills/pi-lite-dev', 'skills/mobicopidev',
            'agents/planner.md', 'agents/reviewer.md', 'agents/scout.md', 'agents/worker.md']:
    p = base/rel
    if p.is_symlink():
        raise SystemExit('Abbruch: Symlink im R2-Zielpfad')
    if rel.startswith('skills/') and p.exists() and p.is_dir():
        if any(x.is_symlink() for x in p.rglob('*')):
            raise SystemExit('Abbruch: Symlink unter R2-Ziel; manuell prüfen')
if not base.is_dir():
    raise SystemExit('Abbruch: R2-Agentenverzeichnis fehlt')
print('R2-Zielprüfung bestanden; keine Änderungen.')
PY
args=(-rlt --checksum --itemize-changes --safe-links --chmod=D700,F600)
if [[ $mode == --dry-run ]]; then
  args+=(--dry-run)
  echo 'DRY-RUN: auf R2 wird nichts geschrieben.'
else
  # Backup außerhalb der automatisch entdeckten Skills/Agenten, keine Löschungen.
  args+=(--backup "--backup-dir=/home/medienhaus/.pi-sync-backups/$(date -u +%Y%m%dT%H%M%SZ)-$$")
  echo 'APPLY: nur Allowlist; vorhandene Dateien werden gesichert, keine Löschungen.'
fi
rsync "${args[@]}" -e 'ssh -o BatchMode=yes -o ConnectTimeout=8' "$stage/" "$remote:.pi/agent/"
echo 'Kein Reload, Neustart, Prompt oder Repository-Abgleich ausgeführt.'
