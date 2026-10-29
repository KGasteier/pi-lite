import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { missing, importHub, request } from './harness.mjs';
import { empfangFixture } from './empfang-harness.mjs';
import { readTarGz } from './tar-fixtures.mjs';

const absent = missing('empfang/empfang.mjs', 'empfang/test/fake-oidc.mjs', 'lib/sync-v1.mjs');
const options = { skip: absent.length ? `I3-Dienste fehlen: ${absent.join(', ')}` : false, timeout: 30000 };
const hour = 3600e3;
async function packet(response, status, typ) {
  const body = await response.json();
  assert.equal(response.status, status, JSON.stringify(body));
  assert.equal(body.protokoll, 1);
  if (typ) assert.equal(body.typ, typ);
  return body;
}
function resultFile(f, person) {
  const file = path.join(f.hubFixtures[person.p.slot].roots.Ergebnisse, 'behalten.md');
  fs.writeFileSync(file, 'Das Ergebnis bleibt bis zur verschlüsselten Sicherung.');
  return file;
}
async function assertForced(f, person, clock) {
  assert.equal(f.empfang.pool.status().slots_frei, 1);
  assert.deepEqual(f.empfang.pool.status().belegungen, []);
  const stored = fs.readFileSync(path.join(f.boxesDir, `${person.p.checkin_id}.json`), 'utf8');
  assert.ok(!stored.includes('Das Ergebnis bleibt'));
  const { decryptArchive } = await importHub('empfang/ausweis.mjs');
  const bytes = decryptArchive(JSON.parse(stored).envelope, person.owner.archive.privateKey, clock);
  assert.equal(readTarGz(bytes).get('Ergebnisse/behalten.md').toString(),
    'Das Ergebnis bleibt bis zur verschlüsselten Sicherung.');
  assert.deepEqual(f.events.slice(-4), ['archiv:account01', 'freigabe:account01', 'leeren:account01', 'leer-bestaetigt:account01']);
  assert.ok(!f.events.includes('pruefen:account01'));
  for (const root of Object.values(f.hubFixtures.account01.roots)) assert.deepEqual(fs.readdirSync(root), []);
}

test('N8 HTTP: signierter Aktiv-Puls alle 60 s verhindert Acht-Stunden-Zwang', options, async t => {
  const started = new Date(2026, 9, 29, 8, 0).getTime(); let clock = started;
  const f = await empfangFixture(t, 1, { now: () => clock });
  const person = await f.checkin(); assert.equal(person.response.status, 201);
  const file = resultFile(f, person);
  // Mehr als acht Stunden, aber noch vor Tagesende. Jeder Puls läuft über echte
  // HTTP-Geräteauth; keine Poolmanipulation und keine Änderung der Systemuhr.
  for (let minute = 0; minute <= 9 * 60; minute++) {
    clock = started + minute * 60000;
    await packet(await f.signed(person, '/v1/empfang/aktiv', { verbunden: true }), 200, 'aktiv_bestaetigt');
    await f.empfang.sweep();
    assert.equal(f.empfang.pool.status().slots_frei, 0, `Minute ${minute}`);
  }
  const [active] = f.empfang.pool.status().belegungen;
  assert.equal(active.checkin_id, person.p.checkin_id);
  assert.equal(active.zustand, 'belegt');
  assert.ok(fs.existsSync(file));
  assert.deepEqual(f.events, ['belegung:account01'], 'Keine Archivierung oder Freigabe bei aktivem Gerät');
  assert.deepEqual(fs.readdirSync(f.boxesDir), []);
});

test('N8 HTTP: verstummtes true und sauberes false erzwingen erst nach mehr als acht Stunden', options, async t => {
  for (const verbunden of [true, false]) await t.test(`letztes Aktiv-Signal ${verbunden}`, async t => {
    const started = new Date(2026, 9, 29, 0, 0).getTime(); let clock = started;
    const f = await empfangFixture(t, 1, { now: () => clock });
    const person = await f.checkin(); assert.equal(person.response.status, 201);
    const file = resultFile(f, person);
    // Der letzte Puls kommt eine Stunde nach Check-in: die Frist muss von diesem
    // Signal laufen, nicht vom Check-in. Ein altes true darf nicht ewig schützen.
    clock += hour;
    await packet(await f.signed(person, '/v1/empfang/aktiv', { verbunden }), 200, 'aktiv_bestaetigt');
    const lastPulse = clock;
    f.restart(); // zuletzt/verbunden müssen auch aus Temp-Persistenz richtig wirken.
    for (const at of [started + 8 * hour + 1, lastPulse + 8 * hour]) {
      clock = at; await f.empfang.sweep();
      assert.equal(f.empfang.pool.status().slots_frei, 0, 'Noch nicht mehr als acht Stunden seit letztem Puls');
      assert.ok(fs.existsSync(file));
      assert.deepEqual(fs.readdirSync(f.boxesDir), []);
    }
    clock++;
    await f.empfang.sweep();
    await assertForced(f, person, clock);
  });
});

test('N8 Tagesende: Check-in nach 23:30 nutzt den Folgetag; Tagesende erzwingt trotz aktiver Verbindung', options, async t => {
  const started = new Date(2026, 9, 29, 23, 45).getTime(); let clock = started;
  const f = await empfangFixture(t, 1, { now: () => clock });
  const person = await f.checkin(); assert.equal(person.response.status, 201);
  const file = resultFile(f, person);
  const deadline = new Date(2026, 9, 30, 23, 30).getTime();
  // Drei Kontrollpunkte einschließlich Folgetag kurz vor Schluss. Pulse halten
  // die Acht-Stunden-Frist fern, damit hier nur die Tagesende-Regel geprüft wird.
  for (const at of [started, new Date(2026, 9, 30, 0, 0).getTime(), deadline - 1]) {
    clock = at;
    await packet(await f.signed(person, '/v1/empfang/aktiv', { verbunden: true }), 200, 'aktiv_bestaetigt');
    await f.empfang.sweep();
    assert.equal(f.empfang.pool.status().slots_frei, 0);
    assert.ok(fs.existsSync(file));
    assert.deepEqual(fs.readdirSync(f.boxesDir), []);
  }
  clock = deadline;
  await f.empfang.sweep();
  await assertForced(f, person, clock);
});

test('N12 Handy-only: Hubaktivität verhindert Leerlauf-Zwang; nach Inaktivität wird sicher ausgecheckt', options, async t => {
  const started = new Date(2026, 9, 29, 0, 0).getTime(); let clock = started;
  // Das Handy ist nur am Hub aktiv, nicht über Empfangs-Aktivpulse des Desktops.
  const devices = { desktop: 0, mobil: 1 }, checks = [];
  const f = await empfangFixture(t, 1, { now: () => clock, isActive: async slot => {
    checks.push({ slot, at: clock, ...devices });
    return devices.desktop + devices.mobil > 0;
  } });
  const person = await f.checkin(); assert.equal(person.response.status, 201);
  const file = resultFile(f, person);
  clock = started + 8 * hour + 1;
  await f.empfang.sweep();
  assert.deepEqual(checks, [{ slot: 'account01', at: clock, desktop: 0, mobil: 1 }]);
  const [active] = f.empfang.pool.status().belegungen;
  assert.equal(active.checkin_id, person.p.checkin_id);
  assert.equal(active.zustand, 'belegt');
  assert.equal(active.zuletzt, clock, 'N12: aktive Hubantwort erneuert zuletzt');
  assert.equal(f.empfang.pool.status().slots_frei, 0);
  assert.ok(fs.existsSync(file));
  assert.deepEqual(f.events, ['belegung:account01']);
  assert.deepEqual(fs.readdirSync(f.boxesDir), []);
  const lastActive = clock;
  devices.mobil = 0;
  f.restart();
  clock = lastActive + 8 * hour;
  await f.empfang.sweep();
  assert.equal(checks.length, 1, 'Exakt acht Stunden reichen auch nach Neustart nicht');
  assert.equal(f.empfang.pool.status().slots_frei, 0);
  assert.ok(fs.existsSync(file));
  clock++;
  await f.empfang.sweep();
  assert.deepEqual(checks[1], { slot: 'account01', at: clock, desktop: 0, mobil: 0 });
  await assertForced(f, person, clock);
});

test('N12 Hubfehler: Zwang erst nach zweiter Fehlprüfung mit mindestens fünf Minuten Abstand', options, async t => {
  for (const failure of ['Transportfehler', 'ungültige Antwort']) await t.test(failure, async t => {
    const started = new Date(2026, 9, 29, 0, 0).getTime(); let clock = started;
    const checks = [];
    const f = await empfangFixture(t, 1, { now: () => clock, isActive: async slot => {
      checks.push({ slot, at: clock });
      if (failure === 'Transportfehler') throw new Error('Simulierter Hub-Timeout');
      return undefined; // Kein boolesches false: darf keinen sofortigen Zwang erlauben.
    } });
    const person = await f.checkin(); assert.equal(person.response.status, 201);
    const file = resultFile(f, person);
    const firstFailure = started + 8 * hour + 1;
    clock = firstFailure;
    await f.empfang.sweep();
    assert.deepEqual(checks, [{ slot: 'account01', at: firstFailure }]);
    assert.equal(f.empfang.pool.get(person.p.checkin_id).aktiv_fehler_seit, firstFailure);
    f.restart(); // Der erste Fehlschlag muss persistent sein, nicht nur im RAM.
    for (const at of [firstFailure, firstFailure + 30000, firstFailure + 5 * 60000 - 1]) {
      clock = at;
      await f.empfang.sweep();
      assert.equal(checks.length, 1, 'Kein erneuter Hubaufruf innerhalb des Fünf-Minuten-Abstands');
      assert.equal(f.empfang.pool.status().slots_frei, 0);
      assert.ok(fs.existsSync(file));
      assert.deepEqual(f.events, ['belegung:account01'], 'Keine Sicherung/Freigabe nach nur einem Fehlschlag');
      assert.deepEqual(fs.readdirSync(f.boxesDir), []);
    }
    clock = firstFailure + 5 * 60000;
    await f.empfang.sweep();
    assert.deepEqual(checks, [{ slot: 'account01', at: firstFailure }, { slot: 'account01', at: clock }]);
    await assertForced(f, person, clock);
  });
});

async function empfangToken(f, person) {
  const c = await packet(await request(f.base, '/v1/empfang/anmelden/challenge', {
    body: { geraet_id: person.p.geraet_id },
  }), 200, 'challenge');
  const text = `cocopi-v1-empfang\n${f.base}\nempfang\n${person.p.geraet_id}\n${c.challenge_id}\n${c.nonce}`;
  const p = await packet(await request(f.base, '/v1/empfang/anmelden', { body: {
    challenge_id: c.challenge_id,
    signatur: crypto.sign(null, Buffer.from(text), person.owner.key.privateKey).toString('base64url'),
  } }), 200, 'angemeldet');
  return p.zugangstoken;
}

test('N9 HTTP: Seminarverzeichnis ist Empfang-signiert und nur für aktuell eingecheckte Geräte', options, async t => {
  const f = await empfangFixture(t, 3);
  const person = await f.checkin({ net_pubkey: 'a'.repeat(64) });
  const leader = await f.checkin({ leader: true, net_pubkey: 'b'.repeat(64) });
  const withoutNet = await f.checkin();
  for (const p of [person, leader, withoutNet]) assert.equal(p.response.status, 201);
  const queued = await f.checkin({ net_pubkey: 'c'.repeat(64) });
  assert.equal(queued.response.status, 202);
  const hallo = await packet(await request(f.base, '/v1/empfang/hallo'), 200, 'empfang_willkommen');
  const { verifySigned } = await importHub('empfang/ausweis.mjs');
  await packet(await request(f.base, '/v1/empfang/seminar', { method: 'GET' }), 401, 'fehler');
  await packet(await request(f.base, '/v1/empfang/seminar', { method: 'GET', token: 'x'.repeat(43) }), 401, 'fehler');
  await packet(await f.signed(person, '/v1/empfang/seminar', undefined, 'GET', leader.owner.key), 401, 'fehler');
  const before = Date.now();
  const directory = await packet(await f.signed(person, '/v1/empfang/seminar', undefined, 'GET'), 200);
  assert.ok(verifySigned(directory, hallo.empfang_pubkey), 'Signatur über die vollständige Antwort');
  assert.ok(Number.isInteger(directory.verzeichnis.ausgestellt));
  assert.ok(directory.verzeichnis.ausgestellt >= Math.floor(before / 1000));
  assert.ok(directory.verzeichnis.ausgestellt <= Math.floor(Date.now() / 1000));
  const expected = [person, leader].map(p => ({ person: p.p.geraeteausweis.pid, anzeige: p.p.person,
    rolle: p.p.rolle, net_pubkey: p.p.geraeteausweis.net_pubkey }));
  // N11 darf Adressen additiv ergänzen. Die N9-Identitätsfelder bleiben exakt;
  // weder Slots noch Geräte-/Archivschlüssel dürfen im Verzeichnis auftauchen.
  assert.deepEqual(directory.verzeichnis.personen.map(({ person, anzeige, rolle, net_pubkey }) =>
    ({ person, anzeige, rolle, net_pubkey })), expected);
  for (const entry of directory.verzeichnis.personen) {
    assert.deepEqual(Object.keys(entry).filter(k => k !== 'adressen').sort(), ['anzeige', 'net_pubkey', 'person', 'rolle']);
    if ('adressen' in entry) assert.ok(Array.isArray(entry.adressen));
  }
  const manipulated = structuredClone(directory);
  manipulated.verzeichnis.personen[0].rolle = 'seminarleitung';
  assert.equal(verifySigned(manipulated, hallo.empfang_pubkey), false);
  const leaderDirectory = await packet(await f.signed(leader, '/v1/empfang/seminar', undefined, 'GET'), 200);
  assert.ok(verifySigned(leaderDirectory, hallo.empfang_pubkey));
  assert.deepEqual(leaderDirectory.verzeichnis.personen, directory.verzeichnis.personen);
  const token = await empfangToken(f, person);
  const checkout = await packet(await f.signed(person, '/v1/empfang/checkout', { checkin_id: person.p.checkin_id }), 200, 'checkout_bereit');
  // Ein noch gültig authentisiertes, aber nicht mehr belegtes Gerät ist gesperrt.
  await packet(await request(f.base, '/v1/empfang/seminar', { method: 'GET', token }), 401, 'fehler');
  const after = await packet(await f.signed(leader, '/v1/empfang/seminar', undefined, 'GET'), 200);
  assert.ok(verifySigned(after, hallo.empfang_pubkey));
  assert.deepEqual(after.verzeichnis.personen.map(p => p.person), [leader.p.geraeteausweis.pid]);
  await packet(await f.signed(person, `/v1/empfang/checkout/${checkout.checkout_id}/erhalten`, { sha256: checkout.sha256 }), 200, 'ausgecheckt');
  await packet(await request(f.base, '/v1/empfang/seminar', { method: 'GET', token }), 401, 'fehler');
  const reserved = f.empfang.pool.status().belegungen.find(p => p.warte_id === queued.p.warte_id);
  assert.equal(reserved?.zustand, 'reserviert', 'Nachrücken reserviert ohne Poll, aktiviert aber noch nicht');
  const afterRelease = await packet(await f.signed(leader, '/v1/empfang/seminar', undefined, 'GET'), 200);
  assert.ok(verifySigned(afterRelease, hallo.empfang_pubkey));
  assert.deepEqual(afterRelease.verzeichnis.personen.map(p => p.person), [leader.p.geraeteausweis.pid],
    'Eine bloß reservierte Belegung ist noch nicht im Seminarverzeichnis');
});
