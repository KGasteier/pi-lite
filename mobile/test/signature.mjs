import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { once } from 'node:events';
import { createTransport } from '../../ui/transport.js';

async function fixture(t, { welcomeSlot = 'account03', challengeSlot = 'account03' } = {}) {
  const calls = [];
  const server = http.createServer(async (req, res) => {
    for await (const chunk of req) { /* Request vollständig lesen. */ }
    calls.push(req.url);
    const reply = req.url === '/v1/hallo' ? { protokoll: 1, slot: welcomeSlot }
      : { challenge_id: 'test-challenge', nonce: 'test-nonce', slot: challengeSlot, hub_origin: 'https://angreifer.example' };
    res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify(reply));
  });
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(() => { server.closeAllConnections(); server.close(); });
  const hub_origin = `http://127.0.0.1:${server.address().port}`;
  return { calls, hub_origin, transport: createTransport({ baseUrl: hub_origin, getToken: () => '' }) };
}

for (const scenario of [
  { title: 'hallo wechselt den gespeicherten Slot', options: { welcomeSlot: 'account04' }, calls: ['/v1/hallo'] },
  { title: 'Challenge wechselt den gespeicherten Slot', options: { challengeSlot: 'account04' }, calls: ['/v1/hallo', '/v1/anmelden/challenge'] },
  { title: 'alte Registrierung ohne Kontokontext', legacy: true, calls: ['/v1/hallo'] },
  { title: 'gespeicherter Ursprung passt nicht zur Adresse', foreignOrigin: true, calls: ['/v1/hallo'] },
]) {
  test(`M3: ${scenario.title} blockiert vor Signatur`, async t => {
    const { transport, calls, hub_origin } = await fixture(t, scenario.options);
    const context = scenario.legacy ? {} : { slot: 'account03', hub_origin: scenario.foreignOrigin ? 'https://anderer-hub.example' : hub_origin };
    await assert.rejects(transport.hallo({ ...context, geraet_id: 'handy', sign: () => assert.fail('Darf nicht signieren') }), { code: 'fremder_slot' });
    assert.deepEqual(calls, scenario.calls);
  });
}
