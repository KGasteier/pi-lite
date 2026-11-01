#!/usr/bin/env bash
# Containerpilot: im zugeteilten Container als UID 1000, NICHT als root.
# Legt nur neue Dateien an. Kein Modellaufruf und keine Netzwerkverbindung.
set -euo pipefail
set +x
umask 077
[[ $(id -u) == 1000 && $HOME == /home/node ]] || { echo 'Nur als UID 1000 im zugeteilten Pilotcontainer.' >&2; exit 1; }
[[ ${COCOPI_PILOT_KEY_FREIGABE:-} == ja ]] || { echo 'Pilot und personengebundene kurzlebige API-Berechtigung zuerst freigeben.' >&2; exit 1; }
python3 - <<'PY'
import getpass, json, os, pathlib
from urllib.parse import urlsplit
home=pathlib.Path.home()
a=home/'.pi/agent'
a.mkdir(parents=True,exist_ok=True,mode=0o700)
paths=[a/n for n in ('models.json','auth.json','settings.json')]
if any(p.exists() or p.is_symlink() for p in paths):
    raise SystemExit('Bestandskonfiguration vorhanden: keine Überschreibung, manuelle Prüfung.')
url=input('Vom API-Betreiber bestätigte HTTPS-baseUrl (inkl. API-Pfad): ').strip()
u=urlsplit(url)
if u.scheme!='https' or not u.hostname or u.username or u.password or u.query or u.fragment:
    raise SystemExit('Ungültige HTTPS-baseUrl')
api=input('Bestätigtes Protokoll (openai-completions/openai-responses/anthropic-messages): ').strip()
if api not in ('openai-completions','openai-responses','anthropic-messages'): raise SystemExit('Unbestätigtes Protokoll')
models=[x.strip() for x in input('Freigeschaltete Modell-IDs, kommasepariert: ').split(',') if x.strip()]
if not models: raise SystemExit('Modelle fehlen')
key=getpass.getpass('Persönlicher KURZLEBIGER Pilot-API-Key (unsichtbar): ')
if not key or any(c.isspace() for c in key): raise SystemExit('Key fehlt/ungültig')
data=[{'providers':{'udk-pilot':{'baseUrl':url,'api':api,'models':[{'id':m} for m in models]}}},
      {'udk-pilot':{'type':'api_key','key':key}},
      {'defaultProvider':'udk-pilot','defaultModel':models[0]}]
for p,d in zip(paths,data):
    fd=os.open(p,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'w') as f: json.dump(d,f,indent=2); f.write('\n')
print('Pilotkonfiguration angelegt. Kein API-Aufruf. Key serverseitig befristen und nach Test widerrufen.')
PY
