import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import { once } from 'node:events';
import { pathToFileURL } from 'node:url';
import { signLogin } from '../device.js';
import { createTransport, loginSignaturePayload } from '../../ui/transport.js';

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
    createInstance: () => ({ asks: new Map(), emit() {}, start() { this.emit({ type: 'response', command: 'get_state', success: true, data: {} }); }, stop() {}, send(cmd) {
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
  const authenticated = await desktopTransport.hallo({ ...enrollment, hub_origin: baseUrl, sign: desktop.sign }); desktop.token = authenticated.zugangstoken;
  await post('/v1/sessions/oeffnen', { workspace: root, titel: 'Test-Session' }, desktop.token);
  const mobile = await identity('handy');
  const transport = createTransport({ baseUrl, getToken: () => mobile.token, reconnectMs: 5 });
  // H1 ohne vorhandenes Handy: auch e + geraeteart handy braucht Desktopzustimmung.
  const deniedCode = v1.devices.issue('e');
  let desktopPending;
  const denied = transport.einloesen({ art: 'e', code: deniedCode.code, pubkey: mobile.pubkey, geraeteart: 'handy' }, { onPending: packet => { desktopPending = packet; } });
  const deniedCheck = assert.rejects(denied, { code: 'abgelehnt' });
  for (let i = 0; !desktopPending && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.ok(desktopPending?.anfrage_id);
  assert.equal(desktopPending.bestaetigung, undefined);
  assert.equal(v1.devices.list().geraete.length, 1, 'H1: e-Einladung ohne Handy wartet auf Desktop');
  await post('/v1/koppeln/bestaetigen', { anfrage_id: desktopPending.anfrage_id, erlauben: false }, desktop.token);
  await deniedCheck;
  assert.equal(v1.devices.list().geraete.length, 1, 'H1: Ablehnung registriert nichts');
  const code = await post('/v1/koppeln/start', {}, desktop.token);
  let pending;
  const pairing = transport.koppeln({ art: 'k', code: code.code, pubkey: mobile.pubkey, geraetename: mobile.geraetename, geraeteart: 'handy' }, { onPending: packet => { pending = packet; } });
  for (let i = 0; !pending && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.ok(pending?.anfrage_id);
  assert.equal(v1.devices.list().geraete.length, 1, 'H1: vor Bestätigung kein zweites Gerät');
  await post('/v1/koppeln/bestaetigen', { anfrage_id: pending.anfrage_id, erlauben: true }, desktop.token);
  const paired = await pairing;
  const mobileLogin = await transport.hallo({ ...paired, sign: mobile.sign }); mobile.token = mobileLogin.zugangstoken;
  assert.equal(mobileLogin.slot, 'account03');
  const snapshot = await transport.sessions(); assert.equal(snapshot.sessions[0].titel, 'Test-Session');
  const messages = [], stream = transport.events(packet => messages.push(packet)); t.after(() => stream.close());
  for (let i = 0; !messages.some(p => p.typ === 'sessions_liste') && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  await post(`/v1/sessions/${snapshot.sessions[0].session_id}/senden`, { text: 'Nur Mock' }, desktop.token);
  for (let i = 0; !messages.some(p => p.typ === 'session_ereignis') && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.ok(messages.some(p => p.pi_event?.message?.content[0].text === 'Mock-Ergebnis'));
  assert.ok(messages.some(p => p.zustand === 'fertig'));
  // H1/H2: auch eine e-Einladung für ein Handy wartet; b-Code nur in HTTP.
  const otherCode = v1.devices.issue('e');
  const otherIdentity = await identity('handy');
  let invitationPending, settled = false;
  const invitationPairing = transport.einloesen({ art: 'e', code: otherCode.code, pubkey: otherIdentity.pubkey, geraeteart: 'handy' }, { onPending: packet => { invitationPending = packet; } });
  invitationPairing.then(() => { settled = true; });
  for (let i = 0; !invitationPending && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.ok(invitationPending?.bestaetigung?.code);
  assert.equal(settled, false);
  assert.equal(v1.devices.list().geraete.length, 2, 'H1: e-Einladung darf mit geraeteart handy nicht umgehen');
  for (let i = 0; !messages.some(p => p.anfrage_id === invitationPending.anfrage_id) && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.ok(messages.some(p => p.anfrage_id === invitationPending.anfrage_id));
  const assertNoConfirmation = packets => {
    const text = JSON.stringify(packets);
    assert.ok(!text.includes(invitationPending.bestaetigung.code), 'H2: kein b-Code im SSE');
    assert.ok(!text.includes(invitationPending.bestaetigung.url), 'H2: keine b-URL im SSE');
    assert.ok(!text.includes('bestaetigung'), 'H2: kein Bestätigungsobjekt im SSE');
  };
  assertNoConfirmation(messages);
  const replay = await fetch(baseUrl + '/v1/ereignisse', { headers: { Authorization: `Bearer ${mobile.token}`, 'Last-Event-ID': messages.length ? `${stream.lastEventId.split(':')[0]}:1` : '' } });
  const replayReader = replay.body.getReader();
  const replayText = new TextDecoder().decode((await replayReader.read()).value);
  assert.ok(replayText.includes(invitationPending.anfrage_id), 'H2: Replay enthält die wartende Anfrage');
  assertNoConfirmation(replayText);
  await replayReader.cancel();
  await transport.einloesen({ art: 'b', code: invitationPending.bestaetigung.code });
  const other = await invitationPairing;
  assert.equal(v1.devices.list().geraete.length, 3);
  // Widerruf eines anderen Geräts darf dieses Handy nicht abmelden.
  await post(`/v1/geraete/${other.geraet_id}/widerrufen`, {}, desktop.token);
  for (let i = 0; !messages.some(p => p.typ === 'widerruf') && i < 100; i++) await new Promise(r => setTimeout(r, 5));
  assert.notEqual(stream.state, 'abgemeldet');
  assert.equal((await transport.sessions()).sessions.length, 1);
  await transport.abmelden(); await stream.done;
  assert.equal(stream.state, 'abgemeldet');
  await assert.rejects(transport.sessions(), { status: 401 });
});

test('Integration M3: Signatur für Hub A wird von Hub B abgewiesen', { skip: available ? false : 'S3-v1-Modul liegt noch nicht vor' }, async t => {
  const { createV1 } = await import(pathToFileURL(modulePath));
  let hubB;
  // A reicht B-Challenges weiter: selbst gleiche Geräte-ID/Nonce darf nicht genügen.
  const handler = (req, res) => hubB.handle(req, res, new URL(req.url, 'http://127.0.0.1'));
  const serverA = http.createServer(handler), serverB = http.createServer(handler);
  for (const server of [serverA, serverB]) { server.listen(0, '127.0.0.1'); await once(server, 'listening'); }
  const originA = `http://127.0.0.1:${serverA.address().port}`, originB = `http://127.0.0.1:${serverB.address().port}`;
  hubB = createV1({ dir: null, slot: 'account03', hubUrl: originB, roots: [], inRoots: () => null, createInstance: () => assert.fail('kein Spawn') });
  t.after(() => { hubB.close(); for (const server of [serverA, serverB]) { server.closeAllConnections(); server.close(); } });
  const keys = await crypto.subtle.generateKey('Ed25519', false, ['sign', 'verify']);
  const pubkey = Buffer.from(await crypto.subtle.exportKey('raw', keys.publicKey)).toString('base64url');
  const sign = async payload => Buffer.from(await crypto.subtle.sign('Ed25519', keys.privateKey, new TextEncoder().encode(payload))).toString('base64url');
  const clientA = createTransport({ baseUrl: originA, getToken: () => '' });
  const savedA = await clientA.einloesen({ art: 'e', code: hubB.devices.issue('e').code, pubkey, geraeteart: 'handy' });
  assert.equal(savedA.hub_origin, originA);
  await assert.rejects(clientA.hallo({ ...savedA, sign }), { code: 'nicht_angemeldet', status: 401 });
  const clientB = createTransport({ baseUrl: originB, getToken: () => '' });
  const loginB = await clientB.hallo({ ...savedA, hub_origin: originB, sign });
  assert.ok(loginB.zugangstoken, 'Kontrolle: dieselben Schlüssel mit richtigem Ursprung funktionieren');
});

test('M3: Signaturbytes entsprechen dem deterministischen Protokollvektor', async () => {
  const challenge = JSON.parse(await fs.readFile(new URL('../../shared/protocol/v1/examples/anmelden_challenge.json', import.meta.url)));
  const expected = JSON.parse(await fs.readFile(new URL('../../shared/protocol/v1/examples/anmelden_req.json', import.meta.url)));
  const payload = loginSignaturePayload({ hub_origin: new URL('https://R2.example:443/path').origin, slot: 'account01', geraet_id: 'test-geraet', ...challenge });
  assert.equal(payload, 'cocopi-v1-anmelden\nhttps://r2.example\naccount01\ntest-geraet\ntest-challenge\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA');
  const privateKey = await crypto.subtle.importKey('pkcs8', Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), Buffer.alloc(32)]), 'Ed25519', false, ['sign']);
  assert.equal(await signLogin({ privateKey, hub_origin: 'https://r2.example', slot: 'account01', geraet_id: 'test-geraet' }, payload), expected.signatur);
});
