#!/usr/bin/env python3
"""Enable the known local Vorlesen extension for explicitly marked pi-lite RPCs."""
import os
from pathlib import Path
from datetime import datetime
root = Path(os.environ.get('PI_CODING_AGENT_DIR', str(Path.home() / '.pi/agent')))
p = root / 'extensions/vorlesen/index.ts'
if not p.exists():
    print('Sprache: keine Vorlesen-Extension vorhanden; nichts installiert.')
    raise SystemExit(0)
s = p.read_text()
if 'PI_LITE_SPEECH' in s:
    print('Sprache: pi-lite-Unterstützung bereits eingerichtet.')
    raise SystemExit(0)
replacements = {
    'if (ctx.mode !== "tui") return;': 'if (ctx.mode !== "tui" && !(ctx.mode === "rpc" && process.env.PI_LITE_SPEECH === "1")) return;',
    'if (ctx.mode !== "tui" || !state.enabled) return;': 'if ((ctx.mode !== "tui" && !(ctx.mode === "rpc" && process.env.PI_LITE_SPEECH === "1")) || !state.enabled) return;',
}
if any(s.count(old) != 1 for old in replacements):
    print('Sprache: unbekannte Extension-Version; unverändert. Manuelle RPC-Anpassung erforderlich.')
    raise SystemExit(0)
backup = p.with_name('index.ts.bak.pi-lite.' + datetime.now().strftime('%Y%m%d-%H%M%S'))
backup.write_text(s)
for old, new in replacements.items():
    s = s.replace(old, new)
p.write_text(s)
print(f'Sprache: vorhandene Extension für pi-lite RPC aktiviert; Backup: {backup}')
print('Piper, Stimme und pw-play müssen vorhanden sein. Keine Downloads/Änderungen an Stimmen oder Regeln.')
