// Echte Sync-Routen aus dem angegebenen Hub, echte v1-Geräteprüfung, nur Temp-Daten.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { once } from 'node:events';

export const hubDir = path.resolve(process.env.I2_HUB_DIR || '/home/medienhaus/Work/pi-hub-wt/s7');
export const missing = (...files) => files.filter(file => !fs.existsSync(path.join(hubDir, file)));
export const importHub = file => import(pathToFileURL(path.join(hubDir, file)).href);
export const hash = data => crypto.createHash('sha256').update(data).digest('hex');
export function identity() {
  const key = crypto.generateKeyPairSync('ed25519');
  const archive = crypto.generateKeyPairSync('x25519');
  return { key, archive, pubkey: key.publicKey.export({ format: 'jwk' }).x,
    archiv_pubkey: archive.publicKey.export({ format: 'jwk' }).x };
}
export async function request(base, route, { body, token, headers = {}, method = 'POST' } = {}) {
  const binary = Buffer.isBuffer(body);
  return fetch(base + route, { method, redirect: 'error', signal: AbortSignal.timeout(8000),
    headers: { ...(token ? { authorization: 'Bearer ' + token } : {}),
      ...(binary ? { 'content-type': 'application/gzip' } : { 'content-type': 'application/json' }), ...headers },
    ...(method === 'GET' ? {} : { body: binary ? body : JSON.stringify({ protokoll: 1, ...body }) }) });
}
export async function login(base, device, key) {
  const challenge = await (await request(base, '/v1/anmelden/challenge', { body: { geraet_id: device } })).json();
  const payload = `cocopi-v1-anmelden\n${new URL(base).origin}\naccount01\n${device}\n${challenge.challenge_id}\n${challenge.nonce}`;
  const r = await request(base, '/v1/anmelden', { body: { challenge_id: challenge.challenge_id,
    signatur: crypto.sign(null, Buffer.from(payload), key.privateKey).toString('base64url') } });
  if (r.status !== 200) throw new Error('Testgerät konnte sich nicht anmelden: HTTP ' + r.status);
  return (await r.json()).zugangstoken;
}
export async function syncFixture(t) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'cocopi-i2-sync-'));
  t.after(() => fs.rmSync(tmp, { recursive: true, force: true }));
  const { createSync, handleSync } = await importHub('lib/sync-v1.mjs');
  const { DeviceStore, msg } = await importHub('lib/devices-v1.mjs');
  const roots = Object.fromEntries(['Projekte', 'Ergebnisse', 'Skills', 'Verlaeufe'].map(area => [area, path.join(tmp, 'slot', area)]));
  const sync = createSync({ dir: path.join(tmp, 'state'), roots });
  let devices;
  const server = http.createServer(async (req, res) => {
    if (req.url.startsWith('/v1/sync/')) return handleSync(req, res, { sync, devices });
    try {
      const chunks = []; for await (const chunk of req) chunks.push(chunk);
      const b = JSON.parse(Buffer.concat(chunks)); let result;
      if (req.url === '/v1/anmelden/challenge') result = devices.challenge(b.geraet_id);
      else if (req.url === '/v1/anmelden') result = devices.login(b);
      else { res.writeHead(404); res.end(); return; }
      res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify(result));
    } catch (error) {
      res.writeHead(error.status || 500, { 'content-type': 'application/json' });
      res.end(JSON.stringify(error.packet || msg('fehler', { code: 'intern' })));
    }
  });
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(async () => { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  const base = `http://127.0.0.1:${server.address().port}`;
  devices = new DeviceStore({ slot: 'account01', hubUrl: base });
  const owner = identity();
  const invitation = devices.issue('e', { person: 'Testperson' });
  const registered = devices.redeem({ art: 'e', code: invitation.code, pubkey: owner.pubkey, geraeteart: 'desktop' }, 'test-local');
  const token = await login(base, registered.geraet_id, owner.key);
  return { tmp, roots, base, token, sync, devices };
}
