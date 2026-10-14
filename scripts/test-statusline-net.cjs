// Statuszeile (Phase, Zähler, letzter Lauf) und Netz-Seitenleiste – pi-lite-Seite der
// Parität mit copi (copi/app/src/run_activity.rs, view.rs status_line/net_panel).
// Struktur- und Logiktest; keine Live-Layoutprüfung.
const fs = require('node:fs'), assert = require('node:assert/strict');
const { createRunActivity, runClock, runToolDetail } = require('../ui/run-activity.js');

const a = createRunActivity();
assert.equal(a.phaseLabel('bereit'), 'bereit'); assert.equal(a.countsLabel(), '');
a.event({type:'agent_start'}); a.event({type:'turn_start'});
assert.equal(a.phaseLabel('x'), 'wartet auf Modell');
a.event({type:'message_update', assistantMessageEvent:{type:'thinking_delta'}}); assert.equal(a.phaseLabel('x'), 'denkt nach');
a.event({type:'message_update', assistantMessageEvent:{type:'text_delta'}}); assert.equal(a.phaseLabel('x'), 'schreibt Antwort');
a.event({type:'tool_execution_start', toolCallId:'1', toolName:'bash', args:{command:'cargo test\nmehr'}});
a.event({type:'tool_execution_start', toolCallId:'2', toolName:'read', args:{path:'src/a.rs'}});
assert.equal(a.phaseLabel('x'), 'Werkzeug: read +1'); assert.equal(a.detailLabel(), 'src/a.rs');
a.event({type:'tool_execution_end', toolCallId:'2', isError:true});
assert.equal(a.phaseLabel('x'), 'Werkzeug: bash'); assert.equal(a.detailLabel(), 'cargo test…');
a.event({type:'tool_execution_end', toolCallId:'1'}); assert.equal(a.phaseLabel('x'), 'wartet auf Modell');
a.event({type:'queue_update', steering:['a'], followUp:['b','c']});
assert.equal(a.countsLabel(), 'Runde 1 · 2 Werkzeuge · 1 Fehler · Warteschlange 3');
a.event({type:'auto_retry_start', attempt:1, maxAttempts:3}); assert.equal(a.phaseLabel('x'), 'Wiederholung 1/3');
a.event({type:'auto_retry_end'}); a.event({type:'agent_start'}); assert.equal(a.state.toolsDone, 2);
a.event({type:'queue_update', steering:[], followUp:[]});
a.event({type:'agent_settled', aborted:true}, 133);
assert.equal(a.phaseLabel('bereit'), 'bereit');
assert.equal(a.countsLabel(), 'Letzter Lauf 2:13 · 1 Runde · 2 Werkzeuge · 1 Fehler · abgebrochen');
a.event({type:'agent_start'}); assert.equal(a.state.turns, 0); assert(a.state.last);
a.reset(); assert(!a.state.last && !a.state.active);
const b = createRunActivity();
b.event({type:'compaction_start', reason:'manual'}); assert.equal(b.phaseLabel('bereit'), 'komprimiert Kontext');
b.event({type:'compaction_end'}); assert.equal(b.phaseLabel('bereit'), 'bereit');
assert.equal(runClock(133.9), '2:13'); assert.equal(runToolDetail({}), ''); assert.equal(runToolDetail({x:'y'}), 'y');

// Subagenten (Extension subagent: parallel/chain/single) – gleiche Fälle wie run_activity.rs
const c = createRunActivity();
c.event({type:'agent_start'}); c.event({type:'turn_start'});
c.event({type:'tool_execution_start', toolCallId:'p', toolName:'subagent', args:{tasks:[{agent:'scout',task:'a'},{agent:'worker',task:'b'},{agent:'scout',task:'c'}]}});
assert.equal(c.detailLabel(), 'parallel: scout, worker, scout');
assert.equal(c.countsLabel(), 'Runde 1 · 0 Werkzeuge · Subagenten 0/3 fertig');
c.event({type:'tool_execution_update', toolCallId:'p', toolName:'subagent', partialResult:{details:{mode:'parallel', results:[{exitCode:0},{exitCode:-1},{exitCode:1}]}}});
assert.equal(c.countsLabel(), 'Runde 1 · 0 Werkzeuge · Subagenten 2/3 fertig');
c.event({type:'tool_execution_start', toolCallId:'c', toolName:'subagent', args:{chain:[{agent:'a',task:'x'},{agent:'b',task:'{previous}'}]}});
c.event({type:'tool_execution_update', toolCallId:'c', partialResult:{details:{mode:'chain', results:[{exitCode:0},{exitCode:0}]}}});
assert.equal(c.detailLabel(), 'Kette: a → b');
assert.equal(c.countsLabel(), 'Runde 1 · 0 Werkzeuge · Subagenten 3/5 fertig');
c.event({type:'tool_execution_end', toolCallId:'p'}); c.event({type:'tool_execution_end', toolCallId:'c'});
assert.equal(c.countsLabel(), 'Runde 1 · 2 Werkzeuge · 5 Subagenten');
c.event({type:'tool_execution_start', toolCallId:'s', toolName:'subagent', args:{agent:'scout', task:'Finde X\nmehr'}});
assert.equal(c.detailLabel(), 'scout: Finde X…');
c.event({type:'agent_settled'}, 61);
assert.equal(c.countsLabel(), 'Letzter Lauf 1:01 · 1 Runde · 2 Werkzeuge · 6 Subagenten');
// copi: kein zusätzlicher Pixel-Spinner mehr am Verlaufsende (Statuszeile reicht)
assert(!fs.readFileSync('copi/app/src/view.rs', 'utf8').includes('ui.horizontal(|ui| spinner(ui))'));

// Gleiche Texte wie copi (Parität)
const rs = fs.readFileSync('copi/app/src/run_activity.rs', 'utf8'), js = fs.readFileSync('ui/run-activity.js', 'utf8');
for (const t of ['wartet auf Modell','denkt nach','schreibt Antwort','plant Werkzeugaufruf','komprimiert Kontext','Letzter Lauf','Warteschlange','abgebrochen','fertig','Subagenten','parallel: ','Kette: ']) {
  assert(rs.includes(t) && js.includes(t), 'Text in beiden Frontends: ' + t);
}
const html = fs.readFileSync('ui/index.html', 'utf8'), view = fs.readFileSync('copi/app/src/view.rs', 'utf8');
const net = html.slice(html.indexOf('<aside id="netpanel"'), html.indexOf('</aside>', html.indexOf('<aside id="netpanel"')));
for (const m of view.match(/pub const NET_PLACEHOLDER[^;]+;/s)[0].matchAll(/"([^"]+)"/g)) assert(net.includes(m[1]), 'Netz-Platzhaltertext identisch: ' + m[1]);
assert(net.includes(' hidden'), 'Netz-Seitenleiste standardmäßig aus');
// Reihenfolge: Netz liegt neben dem Chatverlauf (unter Kopfzeile), Statuszeile direkt über der Fußzeile.
assert(html.indexOf('</header>') < html.indexOf('<div id="workrow">') && html.indexOf('<aside id="netpanel"') < html.indexOf('<div id="materialbar">'));
assert(html.indexOf('<div id="statusline">') < html.indexOf('<footer>') && html.indexOf('<div id="materialbar">') < html.indexOf('<div id="statusline">'));
assert(html.indexOf('run-activity.js') < html.indexOf('src="app.js"'));
const app = fs.readFileSync('ui/app.js', 'utf8');
assert(app.includes('"pi-lite-net-visible"') && app.includes('e.shiftKey && !e.altKey && e.key.toLowerCase() === "b"'));
assert(view.includes('k(ctx, Key::B, false, true, true)') && view.includes('Panel::right("net")') && view.includes('Panel::bottom("statusline")'));
assert(fs.readFileSync('copi/app/src/store.rs', 'utf8').includes('net_visible: false'));
console.log('Statuszeile (Phasen, Zähler, letzter Lauf, Werkzeugdetail) und Netz-Seitenleiste: Logik und Struktur in beiden Frontends geprüft; keine Live-Layoutprüfung.');
