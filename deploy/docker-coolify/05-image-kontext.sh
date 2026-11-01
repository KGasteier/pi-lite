#!/usr/bin/env bash
# Exportiert nur die für das Pilotimage geprüften getrackten Laufzeitdateien.
set -euo pipefail
repo=${1:?Aufruf: 05-image-kontext.sh /pfad/pi-hub /neues/ziel}
out=${2:?Neues Zielverzeichnis fehlt}
[[ ! -e $out && ! -L $out ]] || { echo 'Ziel vorhanden; keine Überschreibung.' >&2; exit 1; }
python3 - "$repo" "$out" "$(dirname "$0")/image" <<'PY'
import pathlib,subprocess,sys,shutil
repo=pathlib.Path(sys.argv[1]).resolve(); out=pathlib.Path(sys.argv[2]); templates=pathlib.Path(sys.argv[3])
if subprocess.check_output(['git','-C',str(repo),'status','--porcelain','--untracked-files=no'],text=True).strip():
    raise SystemExit('Hub hat Änderungen an getrackten Dateien: zuerst prüfen/committen.')
files=subprocess.check_output(['git','-C',str(repo),'ls-files','-z'],text=True).split('\0')
selected=[f for f in files if f=='server.mjs' or (f.startswith(('lib/','empfang/')) and f.endswith('.mjs')) or f.startswith('client/')]
if 'server.mjs' not in selected: raise SystemExit('Hubquellen fehlen')
for f in selected:
    src=repo/f
    if src.is_symlink() or not src.is_file() or not src.resolve().is_relative_to(repo):
        raise SystemExit('Symlink/abweichende Quelldatei: '+f)
    if any(part.startswith('.') for part in pathlib.Path(f).parts): raise SystemExit('Versteckte Datei: '+f)
out.mkdir(parents=True)
for f in selected:
    dst=out/'hub'/f;dst.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(repo/f,dst)
for f in ('Dockerfile','entrypoint.sh'): shutil.copyfile(templates/f,out/f)
sha=subprocess.check_output(['git','-C',str(repo),'rev-parse','HEAD'],text=True).strip()
(out/'QUELLSTAND.txt').write_text('pi-hub '+sha+'\nExport nur Laufzeitdateien, keine Credentials/Sessions.\n')
print('Neuer Buildkontext:',out,'Hub:',sha)
PY
