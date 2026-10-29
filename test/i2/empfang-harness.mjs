import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { hubDir, importHub, identity, login, request, syncFixture, hash } from './harness.mjs';

export async function empfangFixture(t, count = 10, {
  registerAfter, archiveBefore, detectEmpty = false, now = Date.now,
  isActive = async () => false, trustProxy = false,
} = {}) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cocopi-i2-empfang-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  const { createFakeOidc } = await importHub('empfang/test/fake-oidc.mjs');
  const { createEmpfang, challengePayload } = await importHub('empfang/empfang.mjs');
  const { Oidc } = await importHub('empfang/oidc.mjs');
  const { verifySigned } = await importHub('empfang/ausweis.mjs');
  const fake = await createFakeOidc(); t.after(() => fake.close());
  const oidc = new Oidc({ issuer: fake.issuer, clientId: fake.clientId, allowHttp: true, now });
  const slots = {}, events = [], hubFixtures = {}, processes = new Map(), delays = [];
  const epochenDir = path.join(tmp, 'epochen'); fs.mkdirSync(epochenDir);
  let failClear = false, empfang;
  for (let i = 1; i <= count; i++) {
    const slot = `account${String(i).padStart(2, '0')}`;
    fs.writeFileSync(path.join(epochenDir, slot + '.epoche'), '1\n');
    hubFixtures[slot] = await syncFixture(t, { empty: true, slot }); slots[slot] = hubFixtures[slot].base;
  }
  // Service-Adapter ist ausdrücklich ein Testdouble; echte Sync-/Geräte-Routen laufen auf Loopback.
  const adapter = {
    // N12: false ist ein ausdrücklicher Inaktivitätsnachweis des Testdoubles.
    // Tests können Hubaktivität oder fehlgeschlagene Prüfungen gezielt einspeisen.
    isActive,
    async isEmpty(slot) {
      return detectEmpty && Object.values(hubFixtures[slot].roots).every(root => fs.readdirSync(root).length === 0);
    },
    async register(slot, belegung) {
      assert.ok(verifySigned(belegung, empfang.ausweis.pubkey));
      const b = belegung, f = hubFixtures[slot];
      assert.equal(b.slot, slot);
      f.devices.transaction(state => { state.person = b.person; state.devices = b.geraete.map(g => ({
        ...g, slot, art: g.art, geraetename: g.name, widerrufen: false,
      })); });
      events.push(`belegung:${slot}`);
      await registerAfter?.(slot, belegung);
    },
    async archive(slot) {
      events.push(`archiv:${slot}`);
      await archiveBefore?.(slot);
      const f = hubFixtures[slot], a = await f.sync.archive();
      const data = fs.readFileSync(f.sync.getArchive(a.archiv_id).file);
      return { ...a, data };
    },
    async release(slot, freigabe) {
      assert.ok(verifySigned(freigabe, empfang.ausweis.pubkey));
      const f = hubFixtures[slot]; f.devices.tokens.clear();
      f.devices.transaction(state => { state.devices = []; state.person = ''; });
      const process = processes.get(slot);
      if (process && process.exitCode === null && process.signalCode === null) {
        const exited = once(process, 'exit'); process.kill('SIGTERM'); await exited;
        events.push(`prozess-beendet:${slot}`);
      }
      events.push(`freigabe:${slot}`);
    },
  };
  const server = http.createServer((req, res) => empfang.handle(req, res));
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(async () => { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  const base = `http://127.0.0.1:${server.address().port}`;
  const boxesDir = path.join(tmp, 'empfang/abholfach');
  const configuration = { dir: path.join(tmp, 'empfang'), slots, oidc, origin: base, adapter, epochenDir, now, trustProxy,
    sleep: async ms => { delays.push(ms); },
    clearSlot: async (slot, action) => {
      const f = hubFixtures[slot]; events.push(`${action}:${slot}`);
      assert.equal(action, 'leeren', 'N7: nach dem Hubstart darf kein Live-pruefen folgen');
      if (failClear) throw new Error('Absichtlich fehlender Leeren-Nachweis');
      for (const root of Object.values(f.roots)) {
        for (const entry of fs.readdirSync(root)) fs.rmSync(path.join(root, entry), { recursive: true, force: true });
      }
      assert.ok(Object.values(f.roots).every(root => fs.readdirSync(root).length === 0), 'Slot-Fake ist nicht leer');
      const state = path.join(f.tmp, 'state');
      for (const entry of fs.readdirSync(state)) fs.rmSync(path.join(state, entry), { recursive: true, force: true });
      const file = path.join(epochenDir, slot + '.epoche');
      const epoche = Number(fs.readFileSync(file, 'utf8')) + 1;
      fs.writeFileSync(file, `${epoche}\n`);
      const proof = { slot, zustand: 'leer', nachweis: 'vor_hubstart', epoche };
      events.push(`leer-bestaetigt:${slot}`);
      return proof;
    } };
  const restart = () => { empfang = createEmpfang(configuration); return empfang; };
  restart();
  let serial = 0;
  async function checkin({ sub = `person-${++serial}`, leader = false, owner = identity(), net_pubkey, ip } = {}) {
    const headers = ip === undefined ? {} : { 'x-forwarded-for': ip };
    const nonceRequestedAt = now();
    const nonceResponse = await request(base, '/v1/empfang/nonce', { headers, body: { geraet_pubkey: owner.pubkey } });
    assert.equal(nonceResponse.status, 200, 'Check-in braucht eine echte Server-Nonce');
    const noncePacket = await nonceResponse.json();
    assert.equal(noncePacket.typ, 'checkin_nonce');
    assert.equal(noncePacket.protokoll, 1);
    assert.ok(noncePacket.gueltig_bis >= nonceRequestedAt + 60000 && noncePacket.gueltig_bis <= now() + 60000);
    const { nonce } = noncePacket;
    const sec = Math.floor(now() / 1000);
    const claims = { sub, nonce, iat: sec, exp: sec + 3600, jti: crypto.randomUUID(),
      groups: [leader ? 'cocopi-seminarleitung' : 'cocopi-teilnehmende'] };
    const response = await request(base, '/v1/empfang/checkin', { headers, body: {
      nachweis: { art: 'oidc', id_token: fake.token(claims), nonce },
      geraet: { pubkey: owner.pubkey, name: 'I2-Testgerät', art: 'desktop' }, archiv_pubkey: owner.archiv_pubkey,
      ...(net_pubkey === undefined ? {} : { net_pubkey }),
    } });
    const p = await response.json();
    return { response, p, owner, sub };
  }
  async function signed(person, route, body, method = 'POST', key = person.owner.key) {
    const c = await (await request(base, '/v1/empfang/challenge', { body: { checkin_id: person.p.checkin_id } })).json();
    const payload = challengePayload(base, person.p.checkin_id, c.challenge_id, c.nonce);
    return request(base, route, { method, body, headers: { 'x-cocopi-checkin': person.p.checkin_id,
      'x-cocopi-challenge': c.challenge_id, 'x-cocopi-signatur': crypto.sign(null, Buffer.from(payload), key.privateKey).toString('base64url') } });
  }
  async function token(person) { const f = hubFixtures[person.p.slot]; return login(f.base, person.p.geraet_id, person.owner.key, person.p.slot); }
  return { base, boxesDir, checkin, signed, token, hubFixtures, get empfang() { return empfang; }, events, restart, delays,
    attachProcess: (slot, child) => processes.set(slot, child), failClear: value => { failClear = value; }, hash };
}

export async function fakePi(t) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cocopi-i2-pi-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  const child = spawn(process.execPath, [path.join(hubDir, 'test/fake-pi.mjs')], {
    cwd: tmp, env: { PATH: process.env.PATH, HOME: tmp, XDG_CONFIG_HOME: tmp, PI_CODING_AGENT_DIR: tmp }, stdio: ['pipe', 'pipe', 'pipe'],
  });
  child.stderr.resume();
  t.after(async () => {
    if (child.exitCode !== null || child.signalCode !== null) return;
    const exit = once(child, 'exit'); child.kill('SIGTERM');
    const timer = setTimeout(() => child.kill('SIGKILL'), 1000);
    try { await exit; } finally { clearTimeout(timer); }
  });
  let buffer = '';
  const state = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.stdout.on('data', bytes => {
      buffer += bytes;
      for (;;) {
        const end = buffer.indexOf('\n'); if (end < 0) break;
        const p = JSON.parse(buffer.slice(0, end)); buffer = buffer.slice(end + 1);
        if (p.command === 'get_state') resolve(p);
      }
    });
    child.once('exit', () => reject(new Error('fake-pi unerwartet beendet')));
  });
  child.stdin.write(JSON.stringify({ type: 'get_state', id: 'nur-test' }) + '\n');
  assert.equal((await state).success, true);
  return child;
}
