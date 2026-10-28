import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { once } from 'node:events';
import { SessionActions, personalResults, previewKind, attribution } from '../workflow.js';
import { createTransport, TransportError } from '../../ui/transport.js';

function deferred() { let resolve, reject; const promise = new Promise((a, b) => { resolve = a; reject = b; }); return { promise, resolve, reject }; }
test('I2: Doppelclick sendet genau einen Prompt mit eigener Client-ID, gesperrt bis Bestätigung', async () => {
  const gate = deferred(), calls = [], actions = new SessionActions({ senden(...args) { calls.push(args); return gate.promise; } });
  const first = actions.run('s1', 'senden', 'Handout');
  assert.equal(actions.state('s1').status, 'pending');
  await actions.run('s1', 'senden', 'Handout');
  assert.equal(calls.length, 1); assert.match(calls[0][2], /^[a-f0-9-]{36}$/);
  gate.resolve({ typ: 'session_zustand' }); await first;
  assert.equal(actions.state('s1').status, 'idle');
  await actions.run('s1', 'senden', 'Noch ein Auftrag'); assert.notEqual(calls[0][2], calls[1][2]);
});
test('I2: Funkloch/5xx bleibt gesperrt, bestätigte Ablehnung entsperrt, späte Antworten nach Leeren verworfen', async () => {
  const actions = new SessionActions({ senden: async () => { throw new Error('Funkloch'); } });
  await assert.rejects(actions.run('s', 'senden', 'Text'));
  assert.equal(actions.state('s').status, 'uncertain');
  let stopped = false; actions.transport.stopp = async () => { stopped = true; return {}; };
  await actions.run('s', 'stopp'); assert.equal(stopped, true); assert.equal(actions.state('s').status, 'idle');
  actions.clear(); actions.transport.senden = async () => { throw new TransportError('zu_viele_aktiv', '', '', 409); };
  await assert.rejects(actions.run('s', 'senden', 'Text')); assert.equal(actions.state('s').status, 'idle');
  const gate = deferred(); actions.transport.senden = () => gate.promise;
  const pending = actions.run('s', 'senden', 'Text'); actions.clear(); gate.resolve({ secret: 'nicht übernehmen' });
  assert.equal(await pending, undefined);
});
test('I2: Ergebnisse nur persönlich unter Ergebnisse/, Text bleibt Text, Seminarleitung markiert', () => {
  const packet = { persoenlich: ['Ergebnisse/Handout.pdf', 'Ergebnisse/Bild.png', 'Skills/foo.md', 'Ergebnisse/../secret', 'Ergebnisse/a\\b', 'Ergebnisse/a?token=x', 'Ergebnisse//x'].map(pfad => ({ pfad })), gross: [{ pfad: 'Ergebnisse/gross.pdf' }] };
  assert.deepEqual(personalResults(packet).map(r => r.pfad), ['Ergebnisse/Handout.pdf', 'Ergebnisse/Bild.png']);
  assert.equal(previewKind('Ergebnisse/x.pdf'), 'pdf'); assert.equal(previewKind('x.png'), 'image');
  assert.equal(previewKind('x.html'), 'text'); assert.equal(previewKind('x.svg'), 'text'); assert.equal(previewKind('x.exe'), null);
  assert.equal(attribution({ role: 'user', von: { rolle: 'seminarleitung', person: 'Aylin' } }), 'Aylin · Seminarleitung');
  assert.equal(attribution({ role: 'assistant', von: { rolle: 'gast', person: 'Aylin' } }), 'Agent');
});

test('I2 In-Memory-Hub: Slot-Pfad, A1-Aktionen, Vorschau/Datei read-only, 401 löscht auch ohne SSE', async t => {
  const requests = []; let released = false, logouts = 0;
  const server = http.createServer(async (req, res) => {
    const chunks = []; for await (const chunk of req) chunks.push(chunk);
    requests.push({ path: req.url, method: req.method, body: chunks.length ? JSON.parse(Buffer.concat(chunks)) : null, auth: req.headers.authorization });
    res.setHeader('Cache-Control', 'no-store');
    if (released) { res.writeHead(401); return res.end(JSON.stringify({ code: 'nicht_angemeldet' })); }
    if (req.url.includes('/sync/datei')) { res.writeHead(200, { 'Content-Type': 'text/plain' }); return res.end('Nur ansehen <script>'); }
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(req.url.endsWith('/vorschau') ? { persoenlich: [{ pfad: 'Ergebnisse/Test.txt' }] } : { typ: 'session_zustand', session_id: 'test', zustand: 'arbeitet' }));
  });
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(() => { server.closeAllConnections(); server.close(); });
  const transport = createTransport({ baseUrl: `http://127.0.0.1:${server.address().port}/slot/account03`, getToken: () => 'mock-only', onLogout: () => { logouts++; } });
  await transport.senden('test', 'Auftrag', 'prompt-uuid');
  await transport.antwort('test', 'q1', false); await transport.stopp('test');
  assert.equal((await transport.vorschau()).persoenlich[0].pfad, 'Ergebnisse/Test.txt');
  assert.equal(await (await transport.ergebnis('Ergebnisse/Test.txt')).text(), 'Nur ansehen <script>');
  for (const bad of ['../x', 'Ergebnisse/../x', 'Ergebnisse/a\\b', '/Ergebnisse/x']) assert.throws(() => transport.ergebnis(bad));
  assert.ok(requests.every(r => r.path.startsWith('/slot/account03/v1/') && r.auth === 'Bearer mock-only'));
  assert.equal(requests[0].body.client_id, 'prompt-uuid'); assert.deepEqual(requests[1].body, { protokoll: 1, frage_id: 'q1', wert: false });
  assert.equal(requests[2].path, '/slot/account03/v1/sessions/test/stopp');
  assert.equal(requests[4].method, 'GET');
  released = true;
  await assert.rejects(transport.sessions(), { status: 401 }); assert.equal(logouts, 1);
  await assert.rejects(transport.vorschau(), { status: 401 }); assert.equal(logouts, 1);
});
