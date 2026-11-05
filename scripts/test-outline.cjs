// Gliederung (ui/outline.js): Zeilenmodell, Gruppen ohne Einordnung, Ablageebene.
// Änderungsoperationen selbst laufen in Rust (shared/outline.rs, cargo test).
const fs = require('node:fs'), path = require('node:path'), assert = require('node:assert/strict');
const O = require('../ui/outline.js');

const outline = {
  version: 1, rev: 3, open_unsorted: ['sessions'],
  items: [
    { t: 'cat', id: 'c1', name: 'Projekt', level: 0, collapsed: false },
    { t: 'session', key: '/s/a.jsonl', level: 1, collapsed: false },
    { t: 'cat', id: 'c2', name: 'Archiv', level: 1, collapsed: true },
    { t: 'md', path: '/m/alt.md', level: 2, collapsed: false },
    { t: 'session', key: 'remote:srv:/s/x.jsonl', level: 0, collapsed: false },
    { t: 'md', path: '/m/weg.md', level: 0, collapsed: false },
  ],
};
const sessions = [
  { path: '/s/a.jsonl', name: 'Alpha', title: '', modified: 2, cwd: '/h' },
  { path: '/s/b.jsonl', name: '', title: 'Beta', modified: 1, cwd: '/h' },
  { path: '/s/x.jsonl', name: '', title: 'Remote X', modified: 3, cwd: '', backend: 'srv', backendLabel: 'Server', key: 'remote:srv:/s/x.jsonl' },
];
const md = [{ name: 'alt.md', path: '/m/alt.md' }, { name: 'neu.md', path: '/m/neu.md' }];

assert.deepEqual(O.visible(outline.items), [0, 1, 2, 4, 5], 'Nachfahren zugeklappter Kategorien unsichtbar');
assert.equal(O.subtreeEnd(outline.items, 0), 4);
const rows = O.rows(outline, sessions, md);
assert.deepEqual(rows.map(r => r.label), ['Projekt', 'Alpha', 'Archiv', 'Remote X', 'weg.md', 'Sessions ohne Einordnung', 'Beta', 'md-Dateien ohne Einordnung']);
assert.equal(rows[0].hasChildren, true);
assert.equal(rows[0].count, 3);
assert.equal(rows[2].collapsed, true);
assert.equal(rows[3].remote, true, 'Remote-Session über Schlüssel aufgelöst');
assert.equal(rows[4].missing, true, 'nicht mehr vorhandene Datei bleibt als Hinweis sichtbar');
assert.equal(rows[5].count, 1);
assert.equal(rows[7].collapsed, true, 'md-Gruppe standardmäßig zu');
assert.equal(rows[7].count, 1, 'alt.md steckt in der zugeklappten Kategorie, zählt nicht als lose');
assert.equal(O.rowId(rows[0]), O.rowId({ kind: 'tree', ref: { t: 'cat', id: 'c1', name: 'anders' } }), 'Kategorie-Id unabhängig vom Namen');

// Ablageebene wie Rust drop_level
assert.equal(O.dropLevel(null, null, 5), 0);
assert.equal(O.dropLevel(1, 1, 9), 2);
assert.equal(O.dropLevel(2, 1, 0), 1);
assert.equal(O.dropLevel(0, null, 0), 0);
assert.equal(O.dropLevel(12, null, 99), 12, 'maximal 12 Ebenen wie Rust MAX_LEVEL');
const rust = fs.readFileSync(path.join(__dirname, '../shared/outline.rs'), 'utf8');
assert.match(rust, /pub fn drop_level\(above: Option<usize>, below: Option<usize>, want: usize\)/);
assert.match(rust, /rename_all = "snake_case"/, 'Op-Namen wie in ui/outline.js');
for (const op of ['toggle', 'set_collapsed', 'indent', 'outdent', 'up', 'down', 'move_to', 'insert', 'into_cat', 'remove', 'add_cat', 'rename', 'toggle_unsorted']) {
  const camel = op.split('_').map(w => w[0].toUpperCase() + w.slice(1)).join('');
  assert.match(rust, new RegExp(`\\n    ${camel} [{]`), `Rust-Op ${camel}`);
  assert.match(fs.readFileSync(path.join(__dirname, '../ui/outline.js'), 'utf8'), new RegExp(`op: "${op}"`), `JS nutzt ${op}`);
}
const html = fs.readFileSync(path.join(__dirname, '../ui/index.html'), 'utf8');
assert.match(html, /<option value="outline">Gliederung<\/option>/);
assert.ok(html.indexOf('outline.js') < html.indexOf('app.js'), 'outline.js vor app.js');
const main = fs.readFileSync(path.join(__dirname, '../src-tauri/src/main.rs'), 'utf8');
assert.match(main, /generate_handler!\[[^\]]*outline_read, outline_apply/);
console.log('outline ok');
