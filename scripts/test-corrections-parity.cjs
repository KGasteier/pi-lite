// Strukturtest: dieselben neuen Bedienfunktionen in beiden Frontends.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const html = read('ui/index.html'), js = read('ui/app.js'), rust = read('copi/app/src/view.rs');
assert(html.includes('id="commandmenu"'));
assert(js.includes('allCommands].sort'));
assert(rust.includes('ui.menu_button("/"'));
assert(!js.includes('95 * (1 - Math.exp'));
assert(!rust.includes('95.0 * (1.0 -'));
assert(html.indexOf('id="historyolder"') < html.indexOf('id="historynewer"'));
assert(html.indexOf('id="historynewer"') < html.indexOf('id="materialadd"'));
assert(rust.includes('ui.interact(row(2.0), Id::new("t-add")'));
assert(!rust.includes('pl.last = total;'));
assert(!html.includes('>☰</button>'));
console.log('Parität: Slash-Menü, Laufzeit, Material-Reihenfolge und Sidebar-Symbol vorhanden.');
