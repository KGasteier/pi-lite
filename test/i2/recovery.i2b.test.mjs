import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { hubDir, missing, request, identity } from './harness.mjs';
import { empfangFixture } from './empfang-harness.mjs';

const absent = missing('empfang/empfang.mjs', 'empfang/test/fake-oidc.mjs', 'lib/sync-v1.mjs');
const source = absent.length ? '' : fs.readFileSync(path.join(hubDir, 'empfang/empfang.mjs'), 'utf8');
const options = feature => ({ timeout: 30000, skip: absent.length ? `S7-Empfang fehlt: ${absent.join(', ')}`
  : feature === 'N6' && !/abbruch|ohne_archiv|ohneArchiv/.test(source) ? 'S7-I2b N6 fehlt: Check-out ohne Upload baut bisher immer ein Archiv.'
  : feature === 'restart' && !/recover|wiederherstell|recovery/i.test(source) ? 'S7-I2b Reservierungs-Recovery nach Neustart noch nicht vorhanden.' : false });
async function packet(r, status, typ) {
  assert.equal(r.status, status); const p = await r.json(); assert.equal(p.protokoll, 1); assert.equal(p.typ, typ); return p;
}

test('N6 HTTP: Abbruch nach Vergabe ohne Upload gibt Platz ohne Archiv an erste wartende Person frei', options('N6'), async t => {
  const f = await empfangFixture(t, 1, { detectEmpty: true, archiveBefore() { assert.fail('Ein leerer Check-in darf kein Archiv bauen.'); } });
  const person = await f.checkin(); assert.equal(person.response.status, 201);
  const oldToken = await f.token(person), slot = f.hubFixtures[person.p.slot];
  const queued = await f.checkin(); assert.equal(queued.response.status, 202); assert.equal(queued.p.platz, 1);
  await packet(await f.signed(person, '/v1/empfang/checkout', { checkin_id: person.p.checkin_id }), 200, 'ausgecheckt');
  assert.ok(!f.events.includes('archiv:account01'));
  assert.deepEqual(f.events.slice(-4), ['freigabe:account01', 'leeren:account01', 'pruefen:account01', 'leer-bestaetigt:account01']);
  for (const root of Object.values(slot.roots)) assert.deepEqual(fs.readdirSync(root), []);
  assert.equal((await request(slot.base, '/v1/sync/vorschau', { token: oldToken })).status, 401);
  const promoted = await packet(await request(f.base, '/v1/empfang/warten/' + queued.p.warte_id, { method: 'GET' }), 200, 'eingecheckt');
  assert.equal(promoted.slot, person.p.slot); assert.notEqual(promoted.checkin_id, person.p.checkin_id);
  assert.equal(f.empfang.pool.status().warteschlange, 0);
});

test('N6 HTTP: auch Abbruch eines leeren Check-ins gibt bei fehlendem Leeren-Nachweis keinen Platz frei', options('N6'), async t => {
  const f = await empfangFixture(t, 1, { detectEmpty: true, archiveBefore() { assert.fail('Kein Archiv für leeren Check-in'); } });
  const person = await f.checkin(); assert.equal(person.response.status, 201); f.failClear(true);
  const response = await f.signed(person, '/v1/empfang/checkout', { checkin_id: person.p.checkin_id });
  assert.ok(response.status >= 400); assert.equal((await response.json()).typ, 'fehler');
  assert.equal(f.empfang.pool.status().slots_frei, 0);
  assert.ok(!f.events.includes('leer-bestaetigt:account01'));
  assert.equal((await f.checkin()).response.status, 202);
});

test('Empfang-Neustart nach Hub-Vergabe vor Aktivierungsbestätigung verliert weder Reservierung noch Platz', options('restart'), async t => {
  let loseReply = true;
  const signedRegistrations = [];
  const f = await empfangFixture(t, 1, { registerAfter(slot, signed) {
    signedRegistrations.push(JSON.stringify(signed));
    if (loseReply) throw new Error('Test: Hub hat angenommen, Empfang-Antwort ging vor Persistenz verloren.');
  } });
  const owner = identity(), first = await f.checkin({ sub: 'restart-person', owner });
  assert.ok(first.response.status >= 400);
  assert.equal(signedRegistrations.length, 4, 'Erster Versuch plus höchstens drei Wiederholungen');
  assert.deepEqual(f.delays, [1000, 3000, 9000], 'N2-Backoff exakt 1/3/9 Sekunden');
  const reserved = f.empfang.pool.status().belegungen;
  assert.equal(reserved.length, 1); assert.equal(reserved[0].zustand, 'reserviert');
  const id = reserved[0].checkin_id;
  assert.equal(f.empfang.pool.status().slots_frei, 0);
  // Kein alter asynchroner Auftrag bleibt offen: registerAfter war bereits mit Fehler beendet.
  const key = f.empfang.ausweis.pubkey;
  loseReply = false; f.restart();
  assert.equal(f.empfang.ausweis.pubkey, key, 'Signierschlüssel muss Neustart überleben');
  await f.empfang.sweep();
  const recovered = f.empfang.pool.status().belegungen;
  assert.equal(recovered.length, 1); assert.equal(recovered[0].zustand, 'belegt');
  assert.equal(recovered[0].checkin_id, id);
  assert.equal(signedRegistrations.length, 5, 'Nach Neustart genügt genau eine erneute Aktivierung');
  assert.ok(signedRegistrations.every(b => b === signedRegistrations[0]), 'Wiederholung muss byte-gleich signiert sein');
  const again = await f.checkin({ sub: 'restart-person', owner });
  assert.equal(again.response.status, 201); assert.equal(again.p.checkin_id, id);
  const token = await f.token(again);
  assert.equal((await request(f.hubFixtures.account01.base, '/v1/sync/vorschau', { token })).status, 200);
  assert.equal((await f.checkin()).response.status, 202, 'Kein zweiter Platz während Recovery');
  t.diagnostic('Empfang neu konstruiert aus Temp-Persistenz; signierter Hub-/Leeren-Adapter ist Testdouble, kein systemd-/Prozess-Crash-Test.');
});
