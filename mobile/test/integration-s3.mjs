import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import { once } from 'node:events';
import { pathToFileURL } from 'node:url';
import { createTransport } from '../../ui/transport.js';

// Optional und ausschließlich importiert: kein produktiver Hubprozess, keine Echtdaten.
const modulePath = process.env.COCOPI_V1_TEST_MODULE || '/home/medienhaus/Work/pi-hub-wt/s3/lib/hub-v1.mjs';
let available = false;
try { await fs.access(modulePath); available = true; } catch {}

test('Integration S3-v1: Ed25519-Anmeldung, Kopplung, Sessions/SSE und eigener Widerruf', { skip: available ? false : 'S3-v1-Modul liegt noch nicht vor' }, async t => {
  const { createV1 } = await import(pathToFileURL(modulePath));
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'cocopi-mobile-v1-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  let v1;
  const server = http.createServer((req, res) => v1.handle(req, res, new URL(req.url, 'http://127.0.0.1')));
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  const baseUrl = `http://127.0.0.1:${server.address().port}`;
  v1 = createV1({
    dir: path.join(root, 'state'), slot: 'account03', hubUrl: baseUrl, roots: [root], inRoots: value => value === root ? root : null,
    createInstance: () => ({ asks: new Map(), emit() {}, start() {}, stop() {}, send(cmd) {
      assert.equal(cmd.type, 'prompt');
      this.emit({ type: 'message_end', message: { role: 'assistant', content: [{ type: 'text', text: 'Mock-Ergebnis' }] } });
      this.emit({ type: 'agent_settled' });
    } }),
  });
  t.after(() => { v1.close(); server.closeAllConnections(); server.close(); });
  async function post(route, body = {}, token) {
    const response = await fetch(baseUrl + route, { method: 'POST', headers: { 'Content-Type': 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) }, body: JSON.stringify({ protokoll: 1, ...body }) });
    const packet = await response.json(); assert.ok(response.ok, JSON.stringify(packet)); return packet;
  }
  async function identity(art) {
    const keys = await crypto.subtle.generateKey('Ed25519', false, ['sign', 'verify']);
    const pubkey = Buffer.from(await crypto.subtle.exportKey('raw', keys.publicKey)).toString('base64url');
    return { pubkey, geraetename: 'Mock-Gerät', geraeteart: art, sign: async nonce => Buffer.from(await crypto.subtle.sign('Ed25519', keys.privateKey, new TextEncoder().encode(nonce))).toString('base64url') };
  }
  const desktop = await identity('desktop');
  const invitation = v1.devices.issue('e', { person: 'Mira' });
  const enrollment = await post('/v1/einloesen', { art: 'e', code: invitation.code, pubkey: desktop.pubkey, geraeteart: 'desktop' });
  const desktopTransport = createTransport({ baseUrl, getToken: () => desktop.token });
  const authenticated = await desktopTransport.hallo({ geraet_id: enrollment.geraet_id, sign: desktop.sign }); desktop.token = authenticated.zugangstoken;
  await post('/v1/sessions/oeffnen', { workspace: root, titel: 'Test-Session' }, desktop.token);
  const mobile = await identity('handy');
  const transport = createTransport({ baseUrl, getToken: () => mobile.token, reconnectMs: 5 });
  const code = await post('/v1/koppeln/start', {}, desktop.token);
  let pending;
  const pairing = transport.koppeln({ art: 'k', code: code.code, pubkey: mobile.pubkey, geraetename: mobile.geraetename, geraeteart: 'handy' }, { onPending: packet => { pending = packet; } });
  for (let i = 0; !pending && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.ok(pending?.anfrage_id);
  await post('/v1/koppeln/bestaetigen', { anfrage_id: pending.anfrage_id, erlauben: true }, desktop.token);
  const paired = await pairing;
  const mobileLogin = await transport.hallo({ geraet_id: paired.geraet_id, sign: mobile.sign }); mobile.token = mobileLogin.zugangstoken;
  assert.equal(mobileLogin.slot, 'account03');
  const snapshot = await transport.sessions(); assert.equal(snapshot.sessions[0].titel, 'Test-Session');
  const messages = [], stream = transport.events(packet => messages.push(packet)); t.after(() => stream.close());
  for (let i = 0; !messages.some(p => p.typ === 'sessions_liste') && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  await post(`/v1/sessions/${snapshot.sessions[0].session_id}/senden`, { text: 'Nur Mock' }, desktop.token);
  for (let i = 0; !messages.some(p => p.typ === 'session_ereignis') && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.ok(messages.some(p => p.pi_event?.message?.content[0].text === 'Mock-Ergebnis'));
  assert.ok(messages.some(p => p.zustand === 'fertig'));
  // Widerruf eines anderen Geräts darf dieses Handy nicht abmelden.
  const otherCode = v1.devices.issue('e');
  const other = await post('/v1/einloesen', { art: 'e', code: otherCode.code, pubkey: mobile.pubkey, geraeteart: 'handy' });
  await post(`/v1/geraete/${other.geraet_id}/widerrufen`, {}, desktop.token);
  for (let i = 0; !messages.some(p => p.typ === 'widerruf') && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.notEqual(stream.state, 'abgemeldet');
  assert.equal((await transport.sessions()).sessions.length, 1);
  await transport.abmelden(); await stream.done;
  assert.equal(stream.state, 'abgemeldet');
  await assert.rejects(transport.sessions(), { status: 401 });
});
