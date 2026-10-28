// Privater CryptoKey wird direkt strukturiert in IndexedDB gespeichert, nie exportiert.
const DB = 'cocopi-mobil-device-v1';
function openDB() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore('device');
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}
async function access(mode, operation) {
  const db = await openDB();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('device', mode), req = operation(tx.objectStore('device'));
      tx.oncomplete = () => resolve(req.result);
      tx.onerror = () => reject(tx.error); tx.onabort = () => reject(tx.error);
    });
  } finally { db.close(); }
}
export async function getDevice() {
  if (!globalThis.crypto?.subtle || !globalThis.indexedDB) throw new Error('Dieses Handy kann den Geräteschlüssel nicht sicher speichern. Nutze ein aktuelles Gerät oder später die cocopi-App.');
  const saved = await access('readonly', store => store.get('identity'));
  if (saved) return saved;
  let keys;
  try { keys = await crypto.subtle.generateKey('Ed25519', false, ['sign', 'verify']); }
  catch { throw new Error('Dieses Handy unterstützt den Geräteschlüssel noch nicht. Nutze ein aktuelles Gerät oder später die cocopi-App.'); }
  const raw = new Uint8Array(await crypto.subtle.exportKey('raw', keys.publicKey));
  const device = { id: crypto.randomUUID(), privateKey: keys.privateKey, publicKey: keys.publicKey, pubkey: base64url(raw), name: 'Dieses Handy' };
  // Kein exportierbarer oder rein flüchtiger Schlüssel als stiller Ersatz.
  try { await access('readwrite', store => store.add(device, 'identity')); }
  catch (error) {
    if (error?.name === 'ConstraintError') return access('readonly', store => store.get('identity'));
    throw new Error('Der Geräteschlüssel konnte nicht gespeichert werden. Prüfe den privaten Browsermodus oder nutze die cocopi-App.');
  }
  return device;
}
function base64url(bytes) { return btoa(String.fromCharCode(...bytes)).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, ''); }
export async function saveRegistration({ geraet_id, slot, hub_origin }) {
  if (!geraet_id || !/^account(?:0[1-9]|10)$/.test(slot) || new URL(hub_origin).origin !== hub_origin) {
    throw new Error('R2 hat die Anmeldung nicht vollständig bestätigt. Kopple dieses Handy erneut.');
  }
  const device = await getDevice();
  await access('readwrite', store => store.put({ ...device, geraet_id, slot, hub_origin }, 'identity'));
}
export async function forgetDevice() { await access('readwrite', store => store.clear()); }
export async function signLogin(device, payload) {
  const prefix = ['cocopi-v1-anmelden', device.hub_origin, device.slot, device.geraet_id, ''].join('\n');
  if (!device.hub_origin || !device.slot || !device.geraet_id || typeof payload !== 'string' || !payload.startsWith(prefix) || payload.split('\n').length !== 6) {
    throw new Error('Die Anmeldung passt nicht zum gespeicherten Konto. Kopple dieses Handy erneut.');
  }
  const signature = new Uint8Array(await crypto.subtle.sign('Ed25519', device.privateKey, new TextEncoder().encode(payload)));
  return base64url(signature);
}
