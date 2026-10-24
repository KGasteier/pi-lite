// Multi-Backend-Rasteransicht (ui/net.js): reine Ereignis→Zustand-Funktionen und Einbindung.
// Ereignisform wie src-tauri/src/net.rs (tagged_json). Optional: NET_EVENTS=<Datei> spielt einen
// Mitschnitt aus dem Rust-Live-Test (NET_TEST_DUMP) ab.
const fs = require('node:fs'), path = require('node:path'), assert = require('node:assert/strict');
const N = require('../ui/net.js');

// ---------- Leiste ----------
const m = N.createNetModel();
N.applyBackends(m, [{ id: 'lokal', label: 'Dieser Rechner', kind: 'local', remote: false, enabled: true, connected: true, link: null },
  { id: 'srv', label: 'Server', kind: 'hub', remote: true, enabled: true, connected: true, link: null, token_set: true }]);
assert.deepEqual(Object.keys(m.insts), ['lokal', 'srv']);
N.applyList(m, 'srv', [{ id: 'i2', name: 'B', state: 'idle', asks: 0, last: '' }, { id: 'i10', name: 'C', state: 'busy', asks: 0, last: '' }]);
assert.deepEqual(N.instList(m, 'srv').map(i => i.id), ['i2', 'i10'], 'numerische Sortierung');
const w = (kind, extra) => ({ backend: 'srv', inst: 'i2', src: 'watch', kind, ...extra });
assert.equal(N.applyWatch(m, w('info', { info: { id: 'i2', name: 'B', state: 'waiting', asks: 1, last: 'x' } })), true);
assert.equal(N.applyWatch(m, w('info', { info: { id: 'i2', name: 'B', state: 'waiting', asks: 1, last: 'x' } })), false, 'unverändert');
assert.equal(m.insts.srv.i2.asks, 1);
assert.equal(N.applyWatch(m, w('info', { inst: 'neu', info: { id: 'neu', state: 'starting', asks: 0 } })), true);
assert.equal(N.applyWatch(m, w('gone', { inst: 'neu' })), true);
assert.equal(N.applyWatch(m, w('gone', { inst: 'neu' })), false);
assert.equal(N.applyWatch(m, { backend: 'srv', inst: '', src: 'watch', kind: 'link', up: false }), true);
assert.equal(N.linkLabel(m.backends[1]).cls, 'down');
assert.match(N.linkLabel(m.backends[1]).text, /getrennt/);
assert.equal(N.applyWatch(m, { backend: 'srv', inst: '', src: 'watch', kind: 'link', up: true }), true);
assert.equal(N.linkLabel(m.backends[1]).cls, 'up');
assert.equal(N.applyWatch(m, { ...w('info', { info: {} }), src: 'sub' }), false, 'Abo-Ereignisse gehören nicht in die Leiste');
assert.equal(N.linkLabel({ enabled: false }).cls, 'off');
assert.equal(N.linkLabel({ enabled: true, error: 'kaputt' }).cls, 'err');
assert.equal(N.linkLabel({ enabled: true }).cls, 'wait');
N.applyBackends(m, [m.backends[0]]);
assert.deepEqual(Object.keys(m.insts), ['lokal'], 'entferntes Backend verschwindet');

assert.equal(N.stateLabel('waiting'), 'Rückfrage');
assert.equal(N.stateLabel('exited'), 'beendet');
for (const s of ['starting', 'idle', 'busy', 'waiting', 'exited']) assert(N.STATE_LABELS[s]);

// ---------- Rasterauswahl ----------
let sel = [];
for (const k of ['a', 'b', 'c', 'd']) sel = N.toggleTile(sel, k);
assert.deepEqual(sel, ['a', 'b', 'c', 'd']);
sel = N.toggleTile(sel, 'e'); assert.deepEqual(sel, ['b', 'c', 'd', 'e'], 'höchstens vier, älteste fällt');
sel = N.toggleTile(sel, 'c'); assert.deepEqual(sel, ['b', 'd', 'e']);
assert.deepEqual(N.splitKey(N.tileKey('srv', 'i:1')), ['srv', 'i:1']);
assert.equal(N.gridLayout(1).columns, '1fr');
assert.equal(N.gridLayout(2).columns, '1fr 1fr');
assert.match(N.gridLayout(3).areas, /t0 t1.*t0 t2/);
assert.equal(N.gridLayout(4).rows, '1fr 1fr');

// ---------- Kachel ----------
const t = N.createTile('srv', 'i2', '7');
const sub = (ev, extra) => ({ backend: 'srv', inst: 'i2', src: 'sub', tag: '7', kind: 'line', ev, ...extra });
assert.equal(N.tileEvent(t, { ...sub({ type: 'agent_start' }), tag: '6' }), false, 'altes Abo ignoriert');
assert.equal(N.tileEvent(t, { ...sub({ type: 'agent_start' }), inst: 'x' }), false, 'andere Instanz ignoriert');
assert.equal(N.tileEvent(t, sub(null, { kind: 'live', ev: undefined })), true); assert(t.live);
N.tileEvent(t, sub({ type: 'agent_start' })); assert(t.busy);
N.tileEvent(t, sub({ type: 'message_start', message: { role: 'user', content: [{ type: 'text', text: 'Hallo' }] } }));
N.tileEvent(t, sub({ type: 'message_end', message: { role: 'user', content: [{ type: 'text', text: 'Hallo' }] } }));
N.tileEvent(t, sub({ type: 'message_start', message: { role: 'assistant', content: [] } }));
N.tileEvent(t, sub({ type: 'message_update', assistantMessageEvent: { type: 'thinking_delta', delta: 'hm' } }));
N.tileEvent(t, sub({ type: 'tool_execution_start', toolCallId: 't1', toolName: 'bash', args: { command: 'ls' } }));
N.tileEvent(t, sub({ type: 'tool_execution_end', toolCallId: 't1', result: { content: [{ type: 'text', text: 'ok' }] }, isError: true }));
N.tileEvent(t, sub({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'Du ' } }));
N.tileEvent(t, sub({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'hast' } }));
N.tileEvent(t, sub({ type: 'message_end', message: { role: 'assistant', content: [{ type: 'text', text: 'Du hast **gesagt**' }] } }));
assert.deepEqual(t.items.map(i => i.kind), ['user', 'thinking', 'tool', 'assistant']);
assert.equal(t.items[0].text, 'Hallo');
assert.equal(t.items[2].text, 'ok'); assert(t.items[2].err && t.items[2].done);
assert.equal(t.items[3].text, 'Du hast **gesagt**', 'Endtext ersetzt Deltas');
assert(t.items.every(i => i.dirty), 'neue Einträge sind zu zeichnen');
// Nachricht ohne Deltas (Verlaufsrest) wird übernommen.
N.tileEvent(t, sub({ type: 'message_start', message: { role: 'assistant', content: [] } }));
N.tileEvent(t, sub({ type: 'message_end', message: { role: 'assistant', content: [{ type: 'text', text: 'fertig' }] } }));
assert.equal(t.items.at(-1).text, 'fertig');
// Rückfragen
const req = { type: 'extension_ui_request', id: 'q1', method: 'confirm', title: 'Löschen?' };
assert.equal(N.tileEvent(t, sub(req)), true);
assert.equal(N.tileEvent(t, sub(req)), false, 'doppelte Rückfrage');
assert.equal(N.tileEvent(t, sub({ type: 'extension_ui_request', id: 'n', method: 'notify', message: 'x' })), false, 'notify ist keine Rückfrage');
assert.equal(t.asks.length, 1);
assert.deepEqual(N.answerAsk(t, 'q1', { confirmed: true }), { type: 'extension_ui_response', id: 'q1', confirmed: true });
assert.equal(t.asks.length, 0);
N.tileEvent(t, sub({ ...req, id: 'q2' }));
N.tileEvent(t, sub({ type: 'agent_settled', aborted: true })); assert(!t.busy); assert.equal(t.asks.length, 0, 'Lauf zu Ende → Rückfragen weg');
N.tileEvent(t, sub({ type: 'response', command: 'prompt', success: false, error: 'kaputt' }));
assert.equal(t.items.at(-1).kind, 'note'); assert(t.items.at(-1).err);
N.tileEvent(t, sub(null, { kind: 'exit', ev: undefined })); assert(t.exited); assert.match(t.items.at(-1).text, /beendet/);
assert.equal(N.tileEvent(t, sub(null, { kind: 'stderr', ev: undefined, text: 'x' })), false);

// ---------- Befehle (Protokoll aus B.md) ----------
assert.deepEqual(N.promptCmd('Hi', false), { type: 'prompt', message: 'Hi' });
assert.deepEqual(N.promptCmd('Hi', true), { type: 'prompt', message: 'Hi', streamingBehavior: 'steer' });

// ---------- Formular ----------
assert.equal(N.validateBackendForm({ label: '' }, false), 'Name fehlt');
assert.match(N.validateBackendForm({ label: 'S', url: 'ftp://x' }, false), /http/);
assert.equal(N.validateBackendForm({ label: 'S', url: 'http://h:8787' }, false), null);
assert.match(N.validateBackendForm({ label: 'S', url: 'http://h', token_env: 'A B' }, false), /Umgebungsvariable/);
assert.equal(N.validateBackendForm({ label: 'Lokal' }, true), null);

// ---------- Einbindung und Token-Hygiene ----------
const ui = (f) => fs.readFileSync(path.join(__dirname, '..', 'ui', f), 'utf8');
const html = ui('index.html'), js = ui('net.js'), app = ui('app.js');
assert(html.indexOf('src="app.js"') < html.indexOf('src="net.js"'), 'net.js nach app.js (renderMd)');
assert(html.includes('href="net.css"') && html.includes('id="gridtoggle"') && html.includes('id="netview" hidden'));
assert(/getItem\("pi-lite-net-grid"\) === "1"/.test(js), 'Einzel-Chat ist Standard');
assert(!/console\.(log|info|debug)\([^)]*token/i.test(js), 'kein Token-Logging');
assert(/type="password"|"password"/.test(js), 'Token-Feld verdeckt');
assert(!/\.token\b(?!_)/.test(js.replace(/form\.token|token: token/g, '')), 'UI liest keine Tokenwerte aus Backends');
assert(app.includes('classList.contains("netmode")'), 'Einzel-Chat-Kürzel ruhen im Raster');

// ---------- Optional: Live-Mitschnitt abspielen ----------
if (process.env.NET_EVENTS) {
  const evs = fs.readFileSync(process.env.NET_EVENTS, 'utf8').trim().split('\n').map(JSON.parse);
  const lm = N.createNetModel();
  N.applyBackends(lm, [{ id: 'hub', enabled: true, connected: true }]);
  for (const e of evs) N.applyWatch(lm, e);
  const tags = [...new Set(evs.filter(e => e.tag).map(e => e.tag))];
  const tiles = Object.fromEntries(tags.map(tag => { const e = evs.find(x => x.tag === tag); return [tag, N.createTile(e.backend, e.inst, tag)]; }));
  for (const e of evs) for (const x of Object.values(tiles)) N.tileEvent(x, e);
  assert.equal(lm.backends[0].link, true);
  const a = tiles.A1; assert(a.live && a.items.some(i => i.kind === 'assistant' && /Hallo Raster/.test(i.text)) && a.items.some(i => i.kind === 'tool'));
  assert(tiles.B1.asks.length === 1 && tiles.B1.asks[0].method === 'confirm', 'B1 hat offene Rückfrage');
  assert(tiles.B2.items.some(i => /simuliert/.test(i.text || '')) && !tiles.B2.asks.length, 'B2 nach Antwort fertig');
  console.log(`net live replay ok (${evs.length} Ereignisse)`);
}
console.log('net ok');
