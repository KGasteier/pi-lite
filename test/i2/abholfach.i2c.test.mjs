import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { missing, identity, importHub } from './harness.mjs';
import { empfangFixture } from './empfang-harness.mjs';
import { readTarGz } from './tar-fixtures.mjs';

const absent = missing('empfang/empfang.mjs', 'empfang/test/fake-oidc.mjs', 'lib/sync-v1.mjs');
const options = { skip: absent.length ? `I2c-Dienste fehlen: ${absent.join(', ')}` : false, timeout: 30000 };

async function packet(response, status, typ) {
  assert.equal(response.status, status);
  const value = await response.json();
  assert.equal(value.protokoll, 1); assert.equal(value.typ, typ);
  return value;
}

test('I2c Zwangs-Check-out → Abholfach: gleiches pid, zweites Gerät ohne Schlüssel; physisch nach 72 h gelöscht', options, async t => {
  const started = Date.now(); let clock = started;
  const f = await empfangFixture(t, 2, { now: () => clock });
  const owner = identity(), otherDevice = identity();
  const first = await f.checkin({ sub: 'selbe-testperson', owner });
  const leader = await f.checkin({ sub: 'test-seminarleitung', leader: true });
  assert.equal(first.response.status, 201); assert.equal(leader.response.status, 201);
  const slot = f.hubFixtures[first.p.slot];
  fs.writeFileSync(path.join(slot.roots.Ergebnisse, 'privat.md'), 'Nur das erste Gerät kann dieses Ergebnis lesen.');
  const oldId = first.p.checkin_id;
  const boxFile = path.join(f.boxesDir, `${oldId}.json`);

  await packet(await f.signed(leader, '/v1/empfang/erzwingen', { checkin_id: oldId }), 200, 'ausgecheckt');
  assert.equal(f.empfang.pool.status().slots_frei, 1);
  assert.ok(f.events.includes('leer-bestaetigt:account01'));
  for (const root of Object.values(slot.roots)) assert.deepEqual(fs.readdirSync(root), []);
  assert.ok(fs.existsSync(boxFile), 'Archiv muss das Leeren überleben');
  const stored = fs.readFileSync(boxFile, 'utf8');
  assert.ok(!stored.includes('Nur das erste Gerät'));

  // Derselbe geprüfte OIDC-sub erzeugt dasselbe pid, aber ein neues Gerät hat
  // andere Ed25519- UND X25519-Schlüssel. Abrufrecht ersetzt nicht Entschlüsselung.
  const second = await f.checkin({ sub: 'selbe-testperson', owner: otherDevice });
  assert.equal(second.response.status, 201);
  assert.notEqual(first.p.geraet_id, second.p.geraet_id);
  assert.equal(first.p.geraeteausweis.pid, second.p.geraeteausweis.pid);
  assert.notEqual(owner.archiv_pubkey, otherDevice.archiv_pubkey);
  const list = await packet(await f.signed(second, '/v1/empfang/abholfach', undefined, 'GET'), 200, 'abholfaecher');
  assert.deepEqual(list.archive.map(a => a.checkin_id), [oldId]);
  const box = await packet(await f.signed(second, `/v1/empfang/abholfach/${oldId}`, undefined, 'GET'), 200, 'abholfach');
  const { decryptArchive } = await importHub('empfang/ausweis.mjs');
  // Node-Simulation des zweiten Clients, echte S7-Kryptofunktion, kein Krypto-Mock.
  assert.throws(() => decryptArchive(box.archiv, otherDevice.archive.privateKey, clock));
  const bytes = decryptArchive(box.archiv, owner.archive.privateKey, clock);
  assert.equal(f.hash(bytes), box.archiv.sha256);
  assert.equal(readTarGz(bytes).get('Ergebnisse/privat.md').toString(), 'Nur das erste Gerät kann dieses Ergebnis lesen.');
  assert.equal(box.archiv.ablauf, started + 72 * 3600e3);

  // Empfang-Neustart und echte Sweep-Löschung mit injizierter Uhr: kein Warten,
  // kein Date.now-Patch und keine Manipulation der produktiven Systemzeit.
  f.restart();
  clock = box.archiv.ablauf - 1;
  await f.empfang.sweep();
  assert.ok(fs.existsSync(boxFile), 'Eine Millisekunde vor Ablauf noch vorhanden');
  clock = box.archiv.ablauf;
  await f.empfang.sweep();
  assert.ok(!fs.existsSync(boxFile), 'Am 72-h-Limit muss auch die Datei verschwunden sein');
  assert.ok(!f.empfang.boxes({ pid: first.p.geraeteausweis.pid }).some(a => a.checkin_id === oldId));
  assert.throws(() => decryptArchive(box.archiv, owner.archive.privateKey, clock), 'Auch eine alte lokale Kopie ist abgelaufen');
});
