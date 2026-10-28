import test from 'node:test';
import assert from 'node:assert/strict';
import { getDevice, saveRegistration, forgetDevice, signNonce } from '../device.js';

// Kleiner IndexedDB-Vertrags-Dummy, keine Browser-/Keychainprüfung.
function memoryIDB() {
  const data = new Map();
  return { data, open() {
    const req = {};
    const db = { createObjectStore() {}, close() {}, transaction() {
      const tx = {};
      const operation = (kind, key, value) => {
        const request = {};
        queueMicrotask(() => {
          try {
            if (kind === 'get') request.result = structuredClone(data.get(key));
            else if (kind === 'add' && data.has(key)) throw new DOMException('Vorhanden', 'ConstraintError');
            else if (kind === 'clear') data.clear();
            else data.set(key, structuredClone(value));
            tx.oncomplete?.();
          } catch (error) { tx.error = error; tx.onerror?.(); }
        });
        return request;
      };
      tx.objectStore = () => ({ get: key => operation('get', key), add: (value, key) => operation('add', key, value), put: (value, key) => operation('put', key, value), clear: () => operation('clear') });
      return tx;
    } };
    queueMicrotask(() => { req.result = db; req.onupgradeneeded?.(); req.onsuccess?.(); });
    return req;
  } };
}

test('Geräteschlüssel: Ed25519 nicht exportierbar, IDB-Clone, Base64url und lokale Abmeldung', async t => {
  const prior = globalThis.indexedDB;
  globalThis.indexedDB = memoryIDB();
  t.after(() => { if (prior) globalThis.indexedDB = prior; else delete globalThis.indexedDB; });
  const device = await getDevice();
  assert.equal(device.privateKey.extractable, false);
  assert.match(device.pubkey, /^[A-Za-z0-9_-]{43}$/);
  await assert.rejects(crypto.subtle.exportKey('pkcs8', device.privateKey));
  await saveRegistration('test-handy');
  const saved = await getDevice(); assert.equal(saved.geraet_id, 'test-handy');
  assert.equal(saved.id, device.id);
  const signature = await signNonce(saved, 'nonce-vom-hub');
  assert.match(signature, /^[A-Za-z0-9_-]{86}$/);
  assert.ok(await crypto.subtle.verify('Ed25519', saved.publicKey, Buffer.from(signature, 'base64url'), new TextEncoder().encode('nonce-vom-hub')));
  await forgetDevice(); assert.equal(globalThis.indexedDB.data.size, 0);
});

test('Geräteschlüssel: fehlendes IndexedDB blockiert statt unsicherem Fallback', async t => {
  const prior = globalThis.indexedDB; delete globalThis.indexedDB;
  t.after(() => { if (prior) globalThis.indexedDB = prior; });
  await assert.rejects(getDevice(), /nicht sicher speichern/);
});
