import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { once } from 'node:events';
import { createTransport, createSSEParser, parsePairingCode } from '../../ui/transport.js';

async function fake(t, handler, options = {}) {
  const server = http.createServer(handler);
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(() => { server.closeAllConnections(); server.close(); });
  return createTransport({ baseUrl: `http://127.0.0.1:${server.address().port}`, getToken: () => 'demo', reconnectMs: 5, ...options });
}
const waitFor = async predicate => {
  for (let i = 0; i < 300; i++) { if (predicate()) return; await new Promise(r => setTimeout(r, 5)); }
  assert.fail('Bedingung wurde nicht erreicht');
};
function send(res, id, msg) { res.write(`${id ? `id: ${id}\n` : ''}data: ${JSON.stringify(msg)}\n\n`); }

test('QR: Fragment, Query und Rohcode, keine Hostübernahme', () => {
  const expected = { protokoll: 1, art: 'k', code: 'K7QM2XDA9FHTW3NB' };
  for (const input of ['https://r2.invalid/c#1.k.K7QM-2XDA-9FHT-W3NB', 'https://r2.invalid/c?c=1.k.K7QM2XDA9FHTW3NB', '?c=1.k.K7QM2XDA9FHTW3NB', 'K7QM-2XDA-9FHT-W3NB']) assert.deepEqual(parsePairingCode(input), expected);
  assert.equal(parsePairingCode('K7QM-2XDA-9FHT-W3NB', 'e').art, 'e');
  for (const input of ['1.x.K7QM2XDA9FHTW3NB', '2.k.K7QM2XDA9FHTW3NB', 'K7QM-2XDA-9FHT-W3NI', 'short', '<script>', '----K7QM2XDA9FHTW3NB']) assert.throws(() => parsePairingCode(input));
});

test('SSE-Parser verarbeitet CRLF und mehrzeilige Daten über Chunkgrenzen', () => {
  const found = [], parse = createSSEParser(e => found.push(e));
  parse(': kommentar\r'); parse('\nid: 1\r\ndata: {\r\ndata: "typ":"puls"}\r\n\r'); parse('\n');
  assert.deepEqual(found, [{ id: '1', event: 'message', data: '{\n"typ":"puls"}' }]);
});

test('Transport: Header statt URL, Handschlag und read-only Sessions', async t => {
  const calls = [];
  const transport = await fake(t, async (req, res) => {
    assert.equal(req.headers.authorization, 'Bearer demo'); assert.ok(!req.url.includes('demo'));
    assert.equal(req.headers['cache-control'], 'no-cache');
    let body = ''; for await (const chunk of req) body += chunk;
    calls.push([req.method, req.url, body && JSON.parse(body)]);
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(req.url.endsWith('sessions') ? { sessions: [] } : { protokoll: 1 }));
  });
  await transport.hallo(); assert.deepEqual(await transport.sessions(), { sessions: [] });
  assert.deepEqual(calls.map(c => c.slice(0, 2)), [['POST', '/v1/hallo'], ['GET', '/v1/sessions']]);
  assert.equal(calls[0][2].protokoll, 1);
});

test('SSE: Replay mit Last-Event-ID ohne Dubletten und ohne Lücken', async t => {
  let connections = 0;
  const transport = await fake(t, (req, res) => {
    assert.equal(req.headers.authorization, 'Bearer demo'); assert.equal(req.url, '/v1/events');
    res.writeHead(200, { 'Content-Type': 'text/event-stream' });
    if (++connections === 1) { send(res, null, { typ: 'puls' }); send(res, 1, { typ: 'session.zustand', n: 1 }); send(res, 2, { typ: 'session.zustand', n: 2 }); res.end(); }
    else { assert.equal(req.headers['last-event-id'], '2'); send(res, 2, { typ: 'session.zustand', n: 2 }); send(res, 3, { typ: 'session.zustand', n: 3 }); }
  });
  const found = [], stream = transport.events(msg => { if (msg.n) found.push(msg.n); });
  t.after(() => stream.close());
  await waitFor(() => found.length === 3); stream.close(); await stream.done;
  assert.deepEqual(found, [1, 2, 3]);
});

test('SSE: Sequenzlücke wird sichtbar statt unvollständiger Erfolg', async t => {
  const transport = await fake(t, (req, res) => {
    res.writeHead(200, { 'Content-Type': 'text/event-stream' });
    send(res, 1, { n: 1 }); send(res, 3, { n: 3 });
  });
  const found = [], stream = transport.events(msg => found.push(msg));
  t.after(() => stream.close()); await stream.done;
  assert.ok(found.some(m => m.code === 'replay_luecke'));
  assert.ok(!found.some(m => m.n === 3)); assert.equal(stream.lastEventId, 1);
});

test('SSE: 45-s-Pulsgrenze (im Test verkürzt), Inhalte ersetzen keinen Puls', async t => {
  const transport = await fake(t, (req, res) => {
    res.writeHead(200, { 'Content-Type': 'text/event-stream' });
    send(res, null, { typ: 'puls' });
    const interval = setInterval(() => send(res, null, { typ: 'session.zustand' }), 5);
    req.on('close', () => clearInterval(interval));
  }, { pulseTimeoutMs: 60, reconnectMs: 500 });
  const states = [], stream = transport.events(msg => { if (msg.typ === 'verbindung') states.push(msg.zustand); });
  t.after(() => stream.close());
  await waitFor(() => states.includes('unterbrochen')); stream.close(); await stream.done;
  assert.deepEqual(states, ['verbunden', 'unterbrochen']);
});

test('401 meldet alle Ereignisströme ab, kein endloser Neuversuch', async t => {
  let count = 0;
  const transport = await fake(t, (req, res) => { count++; res.writeHead(401); res.end('{}'); });
  const found = [], stream = transport.events(msg => found.push(msg)); await stream.done;
  assert.equal(stream.state, 'abgemeldet'); assert.equal(count, 1);
  await assert.rejects(transport.sessions(), { status: 401 });
});

test('Transport lehnt ungeschütztes Remote-HTTP und Zugangsdaten-URLs ab', () => {
  for (const baseUrl of ['http://r2.invalid', 'https://demo:secret@r2.invalid', 'https://r2.invalid/?token=secret']) assert.throws(() => createTransport({ baseUrl, getToken: () => '' }));
});
