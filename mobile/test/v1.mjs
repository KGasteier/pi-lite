import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { once } from 'node:events';
import { createTransport } from '../../ui/transport.js';
async function fake(t, handler) {
  const server = http.createServer(handler);
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(() => { server.closeAllConnections(); server.close(); });
  return createTransport({ baseUrl: `http://127.0.0.1:${server.address().port}`, getToken: () => 'demo', reconnectMs: 5 });
}
const waitFor = async predicate => {
  for (let i = 0; i < 300; i++) { if (predicate()) return; await new Promise(r => setTimeout(r, 5)); }
  assert.fail('Bedingung wurde nicht erreicht');
};
function send(res, id, msg) { res.write(`${id ? `id: epoch:${id}\n` : ''}data: ${JSON.stringify(msg)}\n\n`); }

// v1-Vertrag: Geräteanmeldung und serverbestätigte Kopplung ohne persistiertes Token.
test('v1: Einladung, Challenge-Signatur, Widerruf und Fehler erhalten Hubtext', async t => {
  const calls = [];
  const transport = await fake(t, async (req, res) => {
    let raw = ''; for await (const chunk of req) raw += chunk;
    const body = raw ? JSON.parse(raw) : {};
    if (req.method === 'POST') assert.equal(body.protokoll, 1);
    calls.push(req.url);
    let reply;
    if (req.url === '/v1/hallo') reply = { protokoll: 1, typ: 'willkommen', slot: 'account03' };
    if (req.url === '/v1/einloesen') {
      assert.equal(req.headers.authorization, undefined); assert.equal(body.typ, 'einloesen'); assert.equal(body.art, 'e');
      assert.equal(body.code, 'K7QM-2XDA-9FHT-W3NB'); reply = { geraet_id: 'handy' };
    }
    if (req.url === '/v1/anmelden/challenge') { assert.equal(body.geraet_id, 'handy'); reply = { challenge_id: 'c1', nonce: 'test-nonce', slot: 'account03', hub_origin: 'https://untrusted.example' }; }
    if (req.url === '/v1/anmelden') {
      assert.equal(body.signatur, `signed:cocopi-v1-anmelden\nhttp://${req.headers.host}\naccount03\nhandy\nc1\ntest-nonce`);
      reply = { geraet_id: 'handy', zugangstoken: 'demo' };
    }
    if (req.url === '/v1/geraete/handy/widerrufen') { assert.equal(req.headers.authorization, 'Bearer demo'); reply = { typ: 'widerruf' }; }
    res.writeHead(200, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(reply));
  });
  const enrollment = await transport.einloesen('K7QM-2XDA-9FHT-W3NB');
  const signed = await transport.hallo({ geraet_id: 'handy', slot: 'account03', hub_origin: enrollment.hub_origin, sign: async payload => `signed:${payload}` });
  assert.equal(signed.zugangstoken, 'demo');
  await transport.abmelden();
  assert.deepEqual(calls, ['/v1/einloesen', '/v1/hallo', '/v1/anmelden/challenge', '/v1/anmelden', '/v1/geraete/handy/widerrufen']);
});

test('v1: Koppeln wartet auf Desktop, Poll liefert kein Token', async t => {
  let polled = 0, pending = false;
  const transport = await fake(t, async (req, res) => {
    let raw = ''; for await (const chunk of req) raw += chunk;
    assert.equal(req.headers.authorization, undefined);
    const reply = req.url === '/v1/einloesen' ? { typ: 'koppeln_wartet', anfrage_id: 'random-request' } : { typ: 'koppeln_bestaetigt', erlauben: true, geraet_id: 'handy' };
    if (req.method === 'GET') { polled++; assert.equal(req.url, '/v1/koppeln/random-request'); }
    else assert.equal(JSON.parse(raw).art, 'k');
    res.writeHead(200, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(reply));
  });
  const result = await transport.koppeln('K7QM-2XDA-9FHT-W3NB', { onPending: () => { pending = true; } });
  assert.ok(pending); assert.equal(polled, 1); assert.equal(result.geraet_id, 'handy'); assert.ok(!result.zugangstoken);
});

test('v1: neue Hubepoche mit explizitem Neuabgleich ersetzt Cursor', async t => {
  let connections = 0;
  const transport = await fake(t, (req, res) => {
    res.writeHead(200, { 'Content-Type': 'text/event-stream' });
    if (++connections === 1) { send(res, 1, { typ: 'session_zustand', n: 1 }); res.end(); }
    else {
      assert.equal(req.headers['last-event-id'], 'epoch:1');
      send(res, null, { typ: 'fehler', code: 'neu_abgleichen' });
      res.write('id: newepoch:8\ndata: {"typ":"sessions_liste","sessions":[]}\n\n');
      send(res, null, { typ: 'puls' });
    }
  });
  const messages = [], stream = transport.events(msg => messages.push(msg)); t.after(() => stream.close());
  await waitFor(() => messages.some(m => m.typ === 'sessions_liste')); stream.close(); await stream.done;
  assert.equal(stream.lastEventId, 'newepoch:8'); assert.ok(!messages.some(m => m.code === 'replay_luecke'));
});

test('v1: verbindlicher Hubfehler bleibt lesbar', async t => {
  const transport = await fake(t, (req, res) => {
    res.writeHead(410, { 'Content-Type': 'application/json' }); res.end(JSON.stringify({ protokoll: 1, typ: 'fehler', code: 'abgelaufen', was: 'Dieser Code ist abgelaufen.', tun: 'Bitte um einen neuen Code.' }));
  });
  await assert.rejects(transport.einloesen('K7QM-2XDA-9FHT-W3NB'), { code: 'abgelaufen', message: 'Dieser Code ist abgelaufen. Bitte um einen neuen Code.' });
});
