import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { missing, importHub, identity } from './harness.mjs';

const absent = missing('empfang/pool.mjs');
const options = { skip: absent.length ? `Fehlt im Hub: ${absent.join(', ')}` : false };
function temp(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cocopi-i2-pool-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true })); return dir;
}
function person(i) {
  return { pid: `nur-test-${i}`, person: `Test ${i}`, rolle: i === 19 ? 'seminarleitung' : 'teilnehmende',
    geraet: { pubkey: identity().pubkey, name: 'Testgerät', art: 'desktop' } };
}

test('Pool-Kern: 20 konkurrierende Reservierungen, zehn verschiedene Konten, zehn FIFO-Plätze', options, async t => {
  const { Pool } = await importHub('empfang/pool.mjs');
  const pool = new Pool({ dir: temp(t) });
  const results = await Promise.all(Array.from({ length: 20 }, (_, i) => Promise.resolve().then(() => pool.checkin(person(i)))));
  const occupied = results.filter(x => x.slot), queued = results.filter(x => x.warte_id);
  assert.equal(occupied.length, 10); assert.equal(queued.length, 10);
  assert.equal(new Set(occupied.map(x => x.slot)).size, 10);
  assert.equal(new Set(occupied.map(x => x.checkin_id)).size, 10);
  assert.deepEqual(queued.map(x => x.platz), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
  assert.equal(pool.poll(queued.at(-1).warte_id).platz, 10, 'Seminarleitung bekommt keinen Vorrang');
  assert.equal(pool.status().slots_frei, 0);
  pool.update(occupied[0].checkin_id, { zustand: 'leeren' }); pool.release(occupied[0].checkin_id);
  const first = pool.poll(queued[0].warte_id);
  assert.equal(first.slot, occupied[0].slot); assert.notEqual(first.checkin_id, occupied[0].checkin_id);
  assert.equal(pool.poll(queued[1].warte_id).platz, 1);
});

test('Pool-Kern: belegtes Konto nicht ohne Leeren freigeben; Zustand über Neustart erhalten', options, async t => {
  const { Pool } = await importHub('empfang/pool.mjs');
  const dir = temp(t), pool = new Pool({ dir }), owner = person(1);
  const first = pool.checkin(owner);
  assert.throws(() => pool.release(first.checkin_id));
  assert.equal(pool.checkin(owner).checkin_id, first.checkin_id, 'Wiederholung reserviert kein zweites Konto');
  const reopened = new Pool({ dir });
  assert.equal(reopened.status().slots_frei, 9);
  assert.equal(reopened.get(first.checkin_id).geraet.pubkey, owner.geraet.pubkey);
  assert.throws(() => reopened.checkin({ ...owner, geraet: person(2).geraet }), 'Fremdes Gerät darf Belegung nicht übernehmen');
});

test('Pool-Kern: unbeobachtete Warteschlange verfällt nach 90 Sekunden', options, async t => {
  const { Pool } = await importHub('empfang/pool.mjs');
  let now = 100000;
  const pool = new Pool({ dir: temp(t), slots: ['account01'], now: () => now });
  const occupied = pool.checkin(person(0)), waiting = pool.checkin(person(1));
  now += 90001;
  assert.throws(() => pool.poll(waiting.warte_id));
  pool.update(occupied.checkin_id, { zustand: 'leeren' }); pool.release(occupied.checkin_id);
  assert.equal(pool.status().slots_frei, 1, 'Abgelaufene Wartende reservieren kein Konto');
});
