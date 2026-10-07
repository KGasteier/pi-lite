#!/usr/bin/env python3
"""Opt-in RPC adaptation of the installed pi-custom-endpoint; never reads models/auth."""
import os
from pathlib import Path
from datetime import datetime
p = Path(os.environ.get('PI_CUSTOM_ENDPOINT_SOURCE', str(Path.home() / '.pi/agent/npm/node_modules/pi-custom-endpoint/index.ts')))
s = p.read_text()
marker = '// pi-lite RPC fallback'
if marker in s:
    print('Endpoint RPC adaptation already present')
    raise SystemExit(0)
one = '\tif (normalizedItems.length === 0) return null;'
many = '): Promise<string[] | null> {\n\treturn await ctx.ui.custom'
if s.count(one) != 1 or s.count(many) != 1:
    raise SystemExit('Unknown extension version; left unchanged')
backup = p.with_name(p.name + '.pi-lite-backup-' + datetime.now().strftime('%Y%m%d-%H%M%S'))
backup.write_text(s)
s = s.replace(one, one + '''
	// pi-lite RPC fallback: custom terminal components are unavailable.
	if (ctx.mode === "rpc") {
		const labels = normalizedItems.map((item, i) => `${i + 1}. ${item.label}${item.suffix ?? ""}${item.description ? " — " + item.description : ""}`);
		const selected = await ctx.ui.select(title, labels);
		const index = selected === undefined ? -1 : labels.indexOf(selected);
		return index < 0 ? null : normalizedItems[index].value;
	}
''')
s = s.replace(many, '''): Promise<string[] | null> {
	// pi-lite RPC fallback: collect a subset through standard select dialogs.
	if (ctx.mode === "rpc") {
		const selected = new Set<string>();
		while (true) {
			const labels = items.map((item, i) => `${selected.has(item.value) ? "[x]" : "[ ]"} ${i + 1}. ${item.label}`);
			const finish = `✓ Confirm selection (${selected.size})`;
			const choice = await ctx.ui.select(title, [finish, ...labels]);
			if (choice === undefined) return null;
			if (choice === finish) return [...selected];
			const index = labels.indexOf(choice);
			if (index >= 0) { const value = items[index].value; if (selected.has(value)) selected.delete(value); else selected.add(value); }
		}
	}
	return await ctx.ui.custom''')
p.write_text(s)
print('Endpoint RPC adaptation installed; backup:', backup)
