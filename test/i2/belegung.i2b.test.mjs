// Unabhängige Vertragstests N2/N3; keine echten Konten, Dienste oder Modellprozesse.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import crypto from 'node:crypto';
import { once } from 'node:events';
import { hubDir, missing, importHub, identity, login, request } from './harness.mjs';

const absent = missing('lib/belegung-v1.mjs', 'lib/hub-v1.mjs');
const source = absent.length ? '' : fs.readFileSync(path.join(hubDir, 'lib/belegung-v1.mjs'), 'utf8');
const router = absent.length ? '' : fs.readFileSync(path.join(hubDir, 'lib/hub-v1.mjs'), 'utf8');
const available = feature => ({ timeout: 30000, skip: absent.length ? `S3-Hub fehlt: ${absent.join(', ')}`
  : feature === 'N2' && !source.includes('belegung_aktiv') ? 'S3-I2b N2 fehlt: Hub kennt belegung_aktiv noch nicht.'
  : feature === 'N3' && !(source + router).includes('HUB_EPOCHE_DATEI') ? 'S3-I2b N3 fehlt: HUB_EPOCHE_DATEI noch nicht eingebunden.' : false });

async function fixture(t) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cocopi-i2b-belegung-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  const { createV1 } = await importHub('lib/hub-v1.mjs');
  const { canonicalJson } = await importHub('lib/belegung-v1.mjs');
  const issuer = identity(), owner = identity(), epochFile = path.join(tmp, 'account01.epoche');
  fs.writeFileSync(epochFile, '1\n');
  const dir = path.join(tmp, 'home', '.cocopi-hub'), root = path.join(tmp, 'arbeit');
  fs.mkdirSync(root);
  let hub;
  const server = http.createServer((req, res) => hub.handle(req, res, new URL(req.url, base)));
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  const base = `http://127.0.0.1:${server.address().port}`;
  t.after(async () => { hub?.close(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  function restart({ clearHome = false } = {}) {
    hub?.close();
    if (clearHome) fs.rmSync(path.join(tmp, 'home'), { recursive: true, force: true });
    // Nur für den synchronen Konstruktor gesetzt, danach ursprünglichen Wert wiederherstellen.
    const previous = process.env.HUB_EPOCHE_DATEI;
    process.env.HUB_EPOCHE_DATEI = epochFile;
    try {
      hub = createV1({ dir, slot: 'account01', hubUrl: base, roots: [root], inRoots: () => root,
        empfangPubkey: issuer.pubkey, createInstance() { assert.fail('Dieser Vertragstest darf keinen Agenten starten.'); } });
    } finally {
      if (previous === undefined) delete process.env.HUB_EPOCHE_DATEI;
      else process.env.HUB_EPOCHE_DATEI = previous;
    }
  }
  restart();
  const sign = value => ({ ...value, signatur: crypto.sign(null, Buffer.from(canonicalJson(value)), issuer.key.privateKey).toString('base64url') });
  const occupancy = { v: 1, typ: 'belegung', slot: 'account01', checkin_id: 's6-checkin-1', epoche: 1,
    person: 'Testperson', rolle: 'teilnehmende', gueltig_bis: Math.floor(Date.now() / 1000) + 3600,
    geraete: [{ geraet_id: 's6-desktop', pubkey: owner.pubkey, name: 'Testgerät', art: 'desktop' }] };
  const release = { v: 1, typ: 'freigabe', slot: 'account01', checkin_id: occupancy.checkin_id, epoche: 1 };
  const occupy = b => request(base, '/v1/belegung', { body: { belegung: b } });
  const free = b => request(base, '/v1/freigabe', { body: { freigabe: b } });
  return { base, owner, occupancy, release, sign, occupy, free, restart, epochFile };
}
async function ok(response, typ) {
  assert.equal(response.status, 200); const p = await response.json();
  assert.equal(p.protokoll, 1); if (typ) assert.equal(p.typ, typ); return p;
}
async function rejected(response, code) {
  assert.ok(response.status >= 400 && response.status < 500, `Replay muss abgewiesen werden: HTTP ${response.status}`);
  const p = await response.json(); assert.equal(p.typ, 'fehler'); if (code) assert.equal(p.code, code);
}

test('N2 HTTP: byte-gleiche Belegung und Freigabe wiederholbar, geänderte Nutzlast bleibt verbraucht', available('N2'), async t => {
  const f = await fixture(t), b = f.sign(f.occupancy), r = f.sign(f.release);
  await ok(await f.occupy(b));
  const token = await login(f.base, 's6-desktop', f.owner.key);
  const repeated = await ok(await f.occupy(b), 'belegung_aktiv');
  assert.equal(repeated.checkin_id, f.occupancy.checkin_id);
  assert.deepEqual(await ok(await f.occupy(b), 'belegung_aktiv'), repeated);
  // Ein Retry darf weder Tokens widerrufen noch die Geräte erneut registrieren.
  await ok(await request(f.base, '/v1/geraete', { method: 'GET', token }), 'geraete_liste');
  await rejected(await f.occupy(f.sign({ ...f.occupancy, person: 'Anderer Inhalt' })), 'verbraucht');
  f.restart();
  assert.deepEqual(await ok(await f.occupy(b), 'belegung_aktiv'), repeated, 'Idempotenz muss Hub-Neustart überleben');
  const firstFree = await ok(await f.free(r), 'freigegeben');
  assert.deepEqual(await ok(await f.free(r), 'freigegeben'), firstFree);
  assert.equal((await request(f.base, '/v1/geraete', { method: 'GET', token })).status, 401);
  await rejected(await f.occupy(b), 'verbraucht');
  f.restart();
  assert.deepEqual(await ok(await f.free(r), 'freigegeben'), firstFree, 'Freigabe-Retry auch nach Neustart');
});

test('N3 HTTP: alte Epoche nach Home-Leeren sperrt signierte Belegung und Freigabe', available('N3'), async t => {
  const f = await fixture(t), oldB = f.sign(f.occupancy), oldR = f.sign(f.release);
  await ok(await f.occupy(oldB)); await ok(await f.free(oldR), 'freigegeben');
  // Ausschließlich Temp-Home leeren; Epoche liegt bewusst außerhalb des Home.
  fs.writeFileSync(f.epochFile, '2\n'); f.restart({ clearHome: true });
  await rejected(await f.occupy(oldB));
  const fresh = { ...f.occupancy, checkin_id: 's6-checkin-2', epoche: 2 };
  await ok(await f.occupy(f.sign(fresh)));
  const token = await login(f.base, 's6-desktop', f.owner.key);
  await rejected(await f.free(oldR));
  await rejected(await f.free(f.sign({ ...f.release, checkin_id: fresh.checkin_id })), undefined);
  await ok(await request(f.base, '/v1/geraete', { method: 'GET', token }), 'geraete_liste');
  await ok(await f.free(f.sign({ ...f.release, checkin_id: fresh.checkin_id, epoche: 2 })), 'freigegeben');
});
