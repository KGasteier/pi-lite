import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { missing, request, identity, importHub } from './harness.mjs';
import { empfangFixture, fakePi } from './empfang-harness.mjs';
import { tarGz, readTarGz } from './tar-fixtures.mjs';

const absent = missing('empfang/empfang.mjs', 'empfang/test/fake-oidc.mjs', 'lib/sync-v1.mjs', 'test/fake-pi.mjs');
const options = { skip: absent.length ? `I2-Dienste fehlen: ${absent.join(', ')}` : false, timeout: 30000 };
async function packet(r, status, typ) {
  assert.equal(r.status, status); const p = await r.json(); assert.equal(p.protokoll, 1); assert.equal(p.typ, typ); return p;
}

test('Empfang E2E: Check-in → Projekt → Vorschau → Archiv → Check-out → Slot-Fake leer', options, async t => {
  const f = await empfangFixture(t, 1);
  const person = await f.checkin(); assert.equal(person.response.status, 201); assert.equal(person.p.typ, 'eingecheckt');
  const process = await fakePi(t); // Modellfreier RPC-Kindprozess wird beim Checkout bestätigt beendet.
  f.attachProcess(person.p.slot, process);
  const slot = f.hubFixtures[person.p.slot], token = await f.token(person);
  await packet(await request(slot.base, '/v1/sync/hoch', { token,
    body: tarGz([{ name: 'start.md', data: 'Bleibt lokal vorhanden' }]),
    headers: { 'x-cocopi-bereich': 'Projekte', 'x-cocopi-name': 'Buch' } }), 201, 'sync_hoch_ok');
  fs.writeFileSync(path.join(slot.roots.Ergebnisse, 'handout.md'), 'Das neue Ergebnis');
  const preview = await packet(await request(slot.base, '/v1/sync/vorschau', { token }), 200, 'sync_vorschau');
  assert.equal(preview.persoenlich.find(p => p.pfad === 'Ergebnisse/handout.md').status, 'neu');
  const checkout = await packet(await f.signed(person, '/v1/empfang/checkout', { checkin_id: person.p.checkin_id }), 200, 'checkout_bereit');
  assert.ok(fs.existsSync(path.join(slot.roots.Ergebnisse, 'handout.md')), 'Vor Bestätigung keine Löschung');
  assert.equal(f.empfang.pool.status().slots_frei, 0);
  const download = await request(slot.base, '/v1/sync/archiv/' + checkout.archiv_id, { method: 'GET', token });
  assert.equal(download.status, 200);
  const bytes = Buffer.from(await download.arrayBuffer()); assert.equal(f.hash(bytes), checkout.sha256);
  const files = readTarGz(bytes);
  assert.deepEqual([...files.keys()], ['Ergebnisse/handout.md']);
  assert.equal(files.get('Ergebnisse/handout.md').toString(), 'Das neue Ergebnis');
  // Client-Rückläufer nur ins eigene Temp-Verzeichnis sichern und dort nachweisen.
  const local = path.join(slot.tmp, 'client'); fs.mkdirSync(local);
  fs.writeFileSync(path.join(local, 'archiv.tar.gz'), bytes);
  for (const [name, data] of files) { const file = path.join(local, name); fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, data, { flag: 'wx' }); }
  assert.equal(fs.readFileSync(path.join(local, 'Ergebnisse/handout.md'), 'utf8'), 'Das neue Ergebnis');
  const route = `/v1/empfang/checkout/${checkout.checkout_id}/erhalten`;
  assert.equal((await f.signed(person, route, { sha256: '0'.repeat(64) })).status, 400);
  assert.equal(f.empfang.pool.status().slots_frei, 0, 'Falscher Hash darf nicht freigeben');
  await packet(await f.signed(person, route, { sha256: checkout.sha256 }), 200, 'ausgecheckt');
  assert.equal(f.empfang.pool.status().slots_frei, 1);
  assert.ok(process.exitCode !== null || process.signalCode !== null, 'Fake-pi muss vor Slotfreigabe beendet sein');
  assert.ok(f.events.indexOf('prozess-beendet:account01') < f.events.indexOf('leeren:account01'));
  assert.deepEqual(f.events.slice(-4), ['freigabe:account01', 'leeren:account01', 'pruefen:account01', 'leer-bestaetigt:account01']);
  for (const root of Object.values(slot.roots)) assert.deepEqual(fs.readdirSync(root), []);
  assert.deepEqual(fs.readdirSync(path.join(slot.tmp, 'state')), []);
  assert.equal((await request(slot.base, '/v1/sync/vorschau', { token })).status, 401, 'Alter Token widerrufen');
});

test('Empfang HTTP: 20 parallele OIDC-Check-ins → 10 eindeutige Slots, 10 FIFO-Wartende', options, async t => {
  const f = await empfangFixture(t);
  const people = await Promise.all(Array.from({ length: 20 }, (_, i) => f.checkin({ sub: `parallel-${i}`, leader: i === 19 })));
  const occupied = people.filter(x => x.response.status === 201), waiting = people.filter(x => x.response.status === 202);
  assert.equal(occupied.length, 10, JSON.stringify(people.map(x => x.p)));
  assert.equal(waiting.length, 10);
  assert.equal(new Set(occupied.map(x => x.p.slot)).size, 10);
  assert.equal(new Set(occupied.map(x => x.p.checkin_id)).size, 10);
  assert.deepEqual(waiting.map(x => x.p.platz).sort((a, b) => a - b), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
  const queued = waiting.sort((a, b) => a.p.platz - b.p.platz);
  const first = occupied[0];
  const checkout = await packet(await f.signed(first, '/v1/empfang/checkout', { checkin_id: first.p.checkin_id }), 200, 'checkout_bereit');
  await packet(await f.signed(first, `/v1/empfang/checkout/${checkout.checkout_id}/erhalten`, { sha256: checkout.sha256 }), 200, 'ausgecheckt');
  const promoted = await packet(await request(f.base, '/v1/empfang/warten/' + queued[0].p.warte_id, { method: 'GET' }), 200, 'eingecheckt');
  assert.equal(promoted.slot, first.p.slot);
  assert.notEqual(promoted.checkin_id, first.p.checkin_id);
  const next = await packet(await request(f.base, '/v1/empfang/warten/' + queued[1].p.warte_id, { method: 'GET' }), 202, 'warteschlange');
  assert.equal(next.platz, 1);
});

test('Empfang: fehlgeschlagenes Leeren lässt Konto gesperrt und gibt keinen Erfolg zurück', options, async t => {
  const f = await empfangFixture(t, 1), person = await f.checkin();
  const slot = f.hubFixtures[person.p.slot]; fs.writeFileSync(path.join(slot.roots.Ergebnisse, 'behalten.md'), 'Nicht verlieren');
  const checkout = await packet(await f.signed(person, '/v1/empfang/checkout', { checkin_id: person.p.checkin_id }), 200, 'checkout_bereit');
  f.failClear(true);
  const r = await f.signed(person, `/v1/empfang/checkout/${checkout.checkout_id}/erhalten`, { sha256: checkout.sha256 });
  assert.ok(r.status >= 400); assert.equal((await r.json()).typ, 'fehler');
  assert.equal(f.empfang.pool.status().slots_frei, 0);
  assert.equal(fs.readFileSync(path.join(slot.roots.Ergebnisse, 'behalten.md'), 'utf8'), 'Nicht verlieren');
  const waiting = await f.checkin(); assert.equal(waiting.response.status, 202);
  assert.ok(!f.events.some(e => e.startsWith('leer-bestaetigt')));
});

test('Empfang Abholfach: anderer pid/falscher Schlüssel blockiert; richtiger Schlüssel holt Archiv', options, async t => {
  const f = await empfangFixture(t, 2), owner = identity();
  const person = await f.checkin({ sub: 'archiv-eigentuemer', owner });
  const stranger = await f.checkin({ sub: 'fremd', leader: true });
  const slot = f.hubFixtures[person.p.slot];
  fs.writeFileSync(path.join(slot.roots.Ergebnisse, 'privat.md'), 'Persönliches Testarchiv');
  await packet(await f.signed(stranger, '/v1/empfang/erzwingen', { checkin_id: person.p.checkin_id }), 200, 'ausgecheckt');
  const oldId = person.p.checkin_id;
  assert.deepEqual(fs.readdirSync(slot.roots.Ergebnisse), []);
  const strangerList = await packet(await f.signed(stranger, '/v1/empfang/abholfach', undefined, 'GET'), 200, 'abholfaecher');
  assert.deepEqual(strangerList.archive, []);
  assert.equal((await f.signed(stranger, '/v1/empfang/abholfach/' + oldId, undefined, 'GET')).status, 404);
  const back = await f.checkin({ sub: 'archiv-eigentuemer', owner }); assert.equal(back.response.status, 201);
  assert.equal((await f.signed(back, '/v1/empfang/abholfach/' + oldId, undefined, 'GET', stranger.owner.key)).status, 401);
  const box = await packet(await f.signed(back, '/v1/empfang/abholfach/' + oldId, undefined, 'GET'), 200, 'abholfach');
  const { decryptArchive } = await importHub('empfang/ausweis.mjs');
  assert.throws(() => decryptArchive(box.archiv, stranger.owner.archive.privateKey));
  const bytes = decryptArchive(box.archiv, owner.archive.privateKey);
  assert.equal(readTarGz(bytes).get('Ergebnisse/privat.md').toString(), 'Persönliches Testarchiv');
  await packet(await f.signed(back, `/v1/empfang/abholfach/${oldId}/erhalten`, { sha256: f.hash(bytes) }), 200, 'abholfach_erhalten');
  assert.equal((await f.signed(back, '/v1/empfang/abholfach/' + oldId, undefined, 'GET')).status, 404);
});
