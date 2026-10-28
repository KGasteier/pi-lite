import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import { once } from 'node:events';
import { pathToFileURL } from 'node:url';
import { createTransport } from '../../ui/transport.js';
const modulePath = process.env.COCOPI_V1_TEST_MODULE || '/home/medienhaus/Work/pi-hub-wt/s3/lib/hub-v1.mjs';
let available = false;
try { await fs.access(modulePath); available = true; } catch {}

// I2-Poolintegration bleibt additiv: signierte Belegung statt Einladung fürs Erstgerät.
test('I2 S3-Pool: k-Kopplung, A1 Prompt/Rückfrage/Stopp, Freigabe → 401 und Logout', { skip: available ? false : 'S3-Modul fehlt' }, async t => {
  const { createV1 } = await import(pathToFileURL(modulePath));
  const belegungModule = pathToFileURL(path.join(path.dirname(modulePath), 'belegung-v1.mjs'));
  let canonicalJson;
  try { ({ canonicalJson } = await import(belegungModule)); } catch { t.skip('S3-I2-Belegung fehlt'); return; }
  const cryptoNode = await import('node:crypto');
  const reception = cryptoNode.generateKeyPairSync('ed25519');
  const receptionPubkey = reception.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32).toString('base64url');
  const signed = value => ({ ...value, signatur: cryptoNode.sign(null, Buffer.from(canonicalJson(value)), reception.privateKey).toString('base64url') });
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'cocopi-mobile-pool-')); t.after(() => fs.rm(root, { recursive: true, force: true }));
  let v1, instance, aborted = false, logout = 0;
  const server = http.createServer((req, res) => v1.handle(req, res, new URL(req.url, 'http://127.0.0.1')));
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  const baseUrl = `http://127.0.0.1:${server.address().port}`;
  v1 = createV1({ dir: path.join(root, 'state'), slot: 'account03', hubUrl: baseUrl, empfangPubkey: receptionPubkey,
    roots: [root], inRoots: value => value === root ? root : null,
    createInstance() {
      instance = { asks: new Map(), emit() {}, start() { this.emit({ type: 'response', command: 'get_state', success: true }); },
        stop() { this.emit({ type: 'hub_exit' }); },
        send(command) {
          if (command.type === 'prompt') {
            const question = { type: 'extension_ui_request', id: 'q1', method: 'confirm', message: 'Fortfahren?' };
            this.asks.set(question.id, question); this.emit(question);
          } else if (command.type === 'extension_ui_response') {
            assert.equal(command.id, 'q1'); assert.equal(command.confirmed, false); this.asks.delete('q1');
          } else if (command.type === 'abort') { aborted = true; this.emit({ type: 'agent_settled' }); }
        },
      }; return instance;
    },
  });
  t.after(() => { v1.close(); server.closeAllConnections(); server.close(); });
  async function identity() {
    const keys = await crypto.subtle.generateKey('Ed25519', false, ['sign', 'verify']);
    return { pubkey: Buffer.from(await crypto.subtle.exportKey('raw', keys.publicKey)).toString('base64url'),
      sign: async payload => Buffer.from(await crypto.subtle.sign('Ed25519', keys.privateKey, new TextEncoder().encode(payload))).toString('base64url') };
  }
  async function post(route, body, token) {
    const response = await fetch(baseUrl + route, { method: 'POST', headers: { 'Content-Type': 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) }, body: JSON.stringify({ protokoll: 1, ...body }) });
    const packet = await response.json(); assert.ok(response.ok, JSON.stringify(packet)); return packet;
  }
  const desktop = await identity(), mobile = await identity();
  await post('/v1/belegung', { belegung: signed({ v: 1, typ: 'belegung', slot: 'account03', checkin_id: 'checkin-mobil', person: 'Mira', rolle: 'teilnehmende', geraete: [{ geraet_id: 'desktop', pubkey: desktop.pubkey, name: 'Test-Rechner', art: 'desktop' }], gueltig_bis: Date.now() + 60000 }) });
  const desktopTransport = createTransport({ baseUrl, getToken: () => desktop.token });
  desktop.token = (await desktopTransport.hallo({ geraet_id: 'desktop', slot: 'account03', hub_origin: baseUrl, sign: desktop.sign })).zugangstoken;
  const opened = await post('/v1/sessions/oeffnen', { workspace: root, titel: 'I2-Test' }, desktop.token);
  const code = await post('/v1/koppeln/start', {}, desktop.token);
  const transport = createTransport({ baseUrl, getToken: () => mobile.token, onLogout: () => { logout++; } });
  let pending;
  const pair = transport.koppeln({ art: 'k', code: code.code, pubkey: mobile.pubkey, geraeteart: 'handy' }, { onPending: packet => { pending = packet; } });
  for (let i = 0; !pending && i < 100; i++) await new Promise(resolve => setTimeout(resolve, 5));
  assert.ok(pending); await post('/v1/koppeln/bestaetigen', { anfrage_id: pending.anfrage_id, erlauben: true }, desktop.token);
  const registration = await pair;
  mobile.token = (await transport.hallo({ ...registration, sign: mobile.sign })).zugangstoken;
  const answer = await transport.senden(opened.session_id, 'Nur Mock', crypto.randomUUID());
  assert.equal(answer.zustand, 'wartet'); assert.equal(answer.frage.id, 'q1');
  await transport.antwort(opened.session_id, 'q1', false);
  await transport.stopp(opened.session_id); assert.equal(aborted, true);
  const freed = await post('/v1/freigabe', { freigabe: signed({ v: 1, typ: 'freigabe', slot: 'account03', checkin_id: 'checkin-mobil' }) });
  assert.equal(freed.typ, 'freigegeben');
  await assert.rejects(transport.sessions(), { status: 401 }); assert.equal(logout, 1);
  await assert.rejects(transport.hallo({ ...registration, sign: mobile.sign }), { status: 401 });
});
