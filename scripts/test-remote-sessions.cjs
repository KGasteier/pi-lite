// Remote-Sessions in der Sessionliste (ui/remote-sessions.js + Schlüssel in session-bookmarks.js).
const fs = require('node:fs'), path = require('node:path'), assert = require('node:assert/strict');
const R = require('../ui/remote-sessions.js');
const { createSessionBookmarks } = require('../ui/session-bookmarks.js');

// Schlüssel: lokal = Pfad, remote = remote:<backend>:<pfad> (identisch zu copi remote_key)
assert.equal(R.remoteSessionKey('', '/a.jsonl'), '/a.jsonl');
assert.equal(R.remoteSessionKey('srv', '/a.jsonl'), 'remote:srv:/a.jsonl');
const rust = fs.readFileSync(path.join(__dirname, '../copi/crates/pi-backend/src/sessions.rs'), 'utf8');
assert.match(rust, /format!\("remote:\{backend\}:\{path\}"\)/, 'Rust-Schlüsselformat gleich');

// Hub-Liste → Listenformat (ms → s), Backend-Kennung
const list = R.fromHub('srv', 'Server Büro', [
  { path: '/s/a.jsonl', modified: 1_700_000_000_500, cwd: '/srv/x', title: 'A' },
  { path: '/s/b.jsonl', modified: 1_600_000_000_000 }, null, { modified: 1 }]);
assert.equal(list.length, 2, 'ungültige Einträge fallen weg');
assert.deepEqual(list[0], { path: '/s/a.jsonl', cwd: '/srv/x', title: 'A', name: '', modified: 1_700_000_000, backend: 'srv', backendLabel: 'Server Büro', key: 'remote:srv:/s/a.jsonl' });
assert.equal(list[1].cwd, '');

// Herkunftsfilter
const local = [{ path: '/s/a.jsonl', modified: 1_650_000_000, cwd: '/h', title: 'L' }];
const all = local.concat(list);
assert.equal(R.filterOrigin(all, 'all').length, 3);
assert.deepEqual(R.filterOrigin(all, 'local'), local);
assert.equal(R.filterOrigin(all, 'remote').length, 2);
assert.equal(R.filterOrigin(all, 'kaputt').length, 3, 'unbekannt → alle');
assert.deepEqual(['all', 'local', 'remote'].map(R.nextOrigin), ['local', 'remote', 'all']);

// Bookmarks: gleicher Pfad lokal und remote bleibt getrennt
const data = new Map(), storage = { getItem: k => data.get(k) ?? null, setItem: (k, v) => data.set(k, v) };
const bm = createSessionBookmarks(storage);
bm.toggle(R.sessionKey(list[0]));
bm.setColor(R.sessionKey(list[0]), 9);
assert.equal(bm.has('/s/a.jsonl'), false, 'lokaler gleicher Pfad ist nicht markiert');
bm.setFilterMode('on');
assert.deepEqual(bm.visible(all).map(R.sessionKey), ['remote:srv:/s/a.jsonl']);
bm.toggle('/s/a.jsonl'); bm.setColor('/s/a.jsonl', 8);
bm.setFilterMode('colors');
assert.deepEqual(bm.sort(bm.visible(all)).map(R.sessionKey), ['/s/a.jsonl', 'remote:srv:/s/a.jsonl'], 'Limette (8) vor Braun (9)');

// Offene Instanz schützt vor Löschen
const insts = { srv: { i1: { id: 'i1', session: '/s/a.jsonl', state: 'idle' }, i2: { id: 'i2', session: '/s/b.jsonl', state: 'exited' } } };
assert.equal(R.openInstance(insts, list[0]).id, 'i1');
assert.equal(R.openInstance(insts, list[1]), null, 'beendete Instanz zählt nicht');
assert.equal(R.openInstance(undefined, list[0]), null);

// Einbindung: Skript geladen, Tauri-Kommandos registriert, Filterknopf vorhanden
const html = fs.readFileSync(path.join(__dirname, '../ui/index.html'), 'utf8');
assert.ok(html.indexOf('remote-sessions.js') > html.indexOf('session-bookmarks.js') && html.indexOf('remote-sessions.js') < html.indexOf('app.js'));
assert.match(html, /id="originfilter"/);
const main = fs.readFileSync(path.join(__dirname, '../src-tauri/src/main.rs'), 'utf8');
assert.match(main, /net_sessions, net_delete_session, net_rename_session, net_fetch/);
assert.match(fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8'), /invoke\("net_rename_session"/, 'Remote-Umbenennen ruft den Hub');
const app = fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8');
assert.doesNotMatch(app, /% 8\b/, 'Farbwähler-Tastatur nicht mehr auf 8 Farben fest');
// Regression: Metazeile (mit Server-Badge) darf nicht unter die Aktionsknöpfe laufen.
const pad = Number((html.match(/\.sess \.m \{ padding-right:(\d+)px; \}/) || [])[1]);
assert.ok(pad >= 56 + 2 * 23, `Metazeile reserviert ${pad}px, Aktionen brauchen ≥ 102px`);
console.log('Remote-Sessions: Schlüssel (Rust-gleich), Hub-Format, Herkunftsfilter, getrennte Bookmarks, Löschschutz geprüft.');
