import test from 'node:test';
import assert from 'node:assert/strict';
import { identity, missing, request } from './harness.mjs';
import { empfangFixture } from './empfang-harness.mjs';

const absent = missing('empfang/empfang.mjs', 'empfang/test/fake-oidc.mjs', 'lib/sync-v1.mjs');
const options = { skip: absent.length ? `I4-Dienste fehlen: ${absent.join(', ')}` : false, timeout: 30000 };
const routes = [
  { name: 'nonce', accepted: 200, body: () => ({ geraet_pubkey: identity().pubkey }) },
  // Auch ungültige Check-ins müssen vor OIDC/Poolzugriff begrenzt werden.
  { name: 'checkin', accepted: 400, body: () => ({}) },
];
async function limited(response, seconds) {
  assert.equal(response.status, 429);
  assert.equal(response.headers.get('retry-after'), String(seconds));
  const p = await response.json();
  assert.equal(p.protokoll, 1);
  assert.equal(p.typ, 'fehler');
  assert.equal(p.code, 'zu_viele_anfragen');
  assert.ok(p.was.length > 0 && p.tun.length > 0, 'Fehler erklärt Ursache und nächsten Schritt');
}

test('N14 HTTP: elfte Anfrage je IP/Endpunkt ergibt 429 mit Retry-After; andere IP bleibt frei', options, async t => {
  let clock = new Date(2026, 9, 29, 8, 0).getTime();
  const f = await empfangFixture(t, 1, { trustProxy: true, now: () => clock });
  for (const route of routes) await t.test(route.name, async () => {
    const send = ip => request(f.base, `/v1/empfang/${route.name}`, {
      body: route.body(), headers: { 'x-forwarded-for': `198.51.100.99, ${ip}` },
    });
    // Derselbe letzte XFF-Eintrag = dieselbe IP. Beide Endpunkte haben eigene Buckets.
    for (let i = 0; i < 10; i++) assert.equal((await send('192.0.2.1')).status, route.accepted);
    await limited(await send('192.0.2.1'), 6);
    assert.equal((await send('192.0.2.2')).status, route.accepted, 'Andere IP ist nicht mitgesperrt');
    clock += 5999;
    await limited(await send('192.0.2.1'), 1);
    clock++;
    assert.equal((await send('192.0.2.1')).status, route.accepted, 'Ein Token nach sechs Sekunden');
    await limited(await send('192.0.2.1'), 6);
  });
  assert.equal(f.empfang.pool.status().slots_frei, 1, 'Ungültige Check-ins belegen keinen Slot');
  assert.deepEqual(f.events, []);
});

test('N14 HTTP: wechselnde X-Forwarded-For ohne Trust-Flag umgehen Socket-IP-Limit nicht', options, async t => {
  const clock = new Date(2026, 9, 29, 8, 0).getTime();
  const f = await empfangFixture(t, 1, { now: () => clock }); // trustProxy bleibt standardmäßig false.
  for (const route of routes) await t.test(route.name, async () => {
    const send = ip => request(f.base, `/v1/empfang/${route.name}`, {
      body: route.body(), headers: ip ? { 'x-forwarded-for': ip } : {},
    });
    for (let i = 1; i <= 10; i++) assert.equal((await send(`192.0.2.${i}`)).status, route.accepted);
    await limited(await send('198.51.100.1'), 6);
    await limited(await send('203.0.113.1'), 6);
    await limited(await send(), 6); // Mit und ohne Header derselbe Loopback-Socket-Absender.
  });
  assert.equal(f.empfang.pool.status().slots_frei, 1);
  assert.deepEqual(f.events, []);
});
