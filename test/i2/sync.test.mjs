import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { tarGz, readTarGz } from './tar-fixtures.mjs';
import { missing, syncFixture, request, hash } from './harness.mjs';

const absent = missing('lib/sync-v1.mjs', 'lib/devices-v1.mjs');
const options = { skip: absent.length ? `Fehlt im Hub: ${absent.join(', ')}` : false, timeout: 20000 };
const upload = (f, body, name = 'Buch', extra = {}) => request(f.base, '/v1/sync/hoch', {
  token: f.token, body, headers: { 'x-cocopi-bereich': 'Projekte', 'x-cocopi-name': name, ...extra } });
async function packet(response, status, typ) {
  assert.equal(response.status, status); const p = await response.json();
  assert.equal(p.protokoll, 1); assert.equal(p.typ, typ); return p;
}

test('Sync: Projekt hochladen → Vorschau → geprüftes Archiv (echte HTTP-Routen)', options, async t => {
  const f = await syncFixture(t);
  const result = await packet(await upload(f, tarGz([{ name: 'quelle.md', data: 'Entwurf' }])), 201, 'sync_hoch_ok');
  assert.equal(result.dateien, 1); assert.equal(result.bytes, Buffer.byteLength('Entwurf'));
  assert.match(result.manifest_sha256, /^[a-f0-9]{64}$/);
  const source = path.join(f.roots.Projekte, 'Buch', 'quelle.md');
  assert.equal(fs.readFileSync(source, 'utf8'), 'Entwurf');
  const unchanged = await packet(await request(f.base, '/v1/sync/vorschau', { token: f.token }), 200, 'sync_vorschau');
  assert.equal(unchanged.persoenlich.find(x => x.pfad === 'Projekte/Buch/quelle.md').status, 'unveraendert');
  fs.writeFileSync(source, 'Überarbeitet');
  fs.writeFileSync(path.join(f.roots.Ergebnisse, 'handout.md'), 'Fertiges Handout');
  fs.mkdirSync(path.join(f.roots.Projekte, 'Buch', 'target'));
  fs.writeFileSync(path.join(f.roots.Projekte, 'Buch', 'target', 'cache.tmp'), 'flüchtig');
  const preview = await packet(await request(f.base, '/v1/sync/vorschau', { token: f.token }), 200, 'sync_vorschau');
  assert.equal(preview.persoenlich.find(x => x.pfad === 'Projekte/Buch/quelle.md').status, 'geaendert');
  assert.equal(preview.persoenlich.find(x => x.pfad === 'Ergebnisse/handout.md').status, 'neu');
  assert.equal(preview.fluechtig.dateien, 1);
  const archive = await packet(await request(f.base, '/v1/sync/archiv', { token: f.token,
    body: { auswahl: { zusaetzlich: [], ausschliessen: [] } } }), 201, 'archiv_bereit');
  const r = await request(f.base, `/v1/sync/archiv/${archive.archiv_id}`, { token: f.token, method: 'GET' });
  assert.equal(r.status, 200); const bytes = Buffer.from(await r.arrayBuffer());
  assert.equal(bytes.length, archive.bytes); assert.equal(hash(bytes), archive.sha256);
  assert.equal(r.headers.get('x-cocopi-sha256'), archive.sha256);
  const files = readTarGz(bytes);
  assert.deepEqual([...files.keys()].sort(), ['Ergebnisse/handout.md', 'Projekte/Buch/quelle.md']);
  assert.equal(files.get('Projekte/Buch/quelle.md').toString(), 'Überarbeitet');
  assert.equal(files.get('Ergebnisse/handout.md').toString(), 'Fertiges Handout');
});

for (const [name, entry] of [
  ['Elternpfad ../', { name: '../ausbruch.md', data: 'nicht schreiben' }],
  ['absoluter Pfad', { name: '/ABSOLUT', data: 'nicht schreiben' }],
  ['Windows-Laufwerk', { name: 'C:/ausbruch.md', data: 'nicht schreiben' }],
  ['Symlink', { name: 'link', type: '2', link: '../ausbruch.md' }],
  ['Hardlink', { name: 'link', type: '1', link: 'quelle.md' }],
  ['3-GiB-Tar-Header ohne Nutzlast', { name: 'riesig.bin', size: 3 * 1024 ** 3 }],
]) {
  test(`Sync-Angriff: ${name} wird atomar abgewiesen`, options, async t => {
    const f = await syncFixture(t);
    // Absolutpfad zeigt ausschließlich auf das eigene Temp-Verzeichnis.
    const malicious = entry.name === '/ABSOLUT' ? { ...entry, name: path.join(f.tmp, 'ausbruch.md') } : entry;
    const r = await upload(f, tarGz([malicious]), 'Angriff');
    assert.ok([400, 403, 413].includes(r.status), `Sicherheitsablehnung erwartet, HTTP ${r.status}`);
    const p = await r.json(); assert.equal(p.typ, 'fehler');
    assert.equal(p.protokoll, 1); assert.ok(p.code && p.was && p.tun);
    assert.deepEqual(fs.readdirSync(f.roots.Projekte), [], 'Kein Teilupload und kein Temp-Rest');
    assert.ok(!fs.existsSync(path.join(f.tmp, 'ausbruch.md')));
    assert.ok(!fs.existsSync(path.join(f.roots.Projekte, 'ausbruch.md')));
    const preview = await packet(await request(f.base, '/v1/sync/vorschau', { token: f.token }), 200, 'sync_vorschau');
    assert.deepEqual(preview.persoenlich, []);
  });
}

test('Sync-Angriff: bestehender Ziel-Symlink bleibt unverändert, kein Schreiben nach außen', options, async t => {
  const f = await syncFixture(t), outside = path.join(f.tmp, 'ausserhalb');
  fs.mkdirSync(outside); fs.writeFileSync(path.join(outside, 'bewahren.md'), 'unverändert');
  fs.symlinkSync(outside, path.join(f.roots.Projekte, 'Fremd'));
  const r = await upload(f, tarGz([{ name: 'bewahren.md', data: 'Angriff' }]), 'Fremd');
  assert.ok([400, 403, 409].includes(r.status));
  assert.equal(fs.readFileSync(path.join(outside, 'bewahren.md'), 'utf8'), 'unverändert');
  assert.deepEqual(fs.readdirSync(outside), ['bewahren.md']);
  assert.ok(fs.lstatSync(path.join(f.roots.Projekte, 'Fremd')).isSymbolicLink());
});

test('Sync: ohne Geräteschlüssel/Token weder Upload noch Archivabruf', options, async t => {
  const f = await syncFixture(t);
  for (const token of [undefined, 'falscher-schluessel']) {
    const r = await request(f.base, '/v1/sync/hoch', { token, body: tarGz([{ name: 'a.md', data: 'x' }]),
      headers: { 'x-cocopi-bereich': 'Projekte', 'x-cocopi-name': 'Unerlaubt' } });
    assert.equal(r.status, 401);
  }
  const archive = await packet(await request(f.base, '/v1/sync/archiv', { token: f.token }), 201, 'archiv_bereit');
  assert.equal((await request(f.base, `/v1/sync/archiv/${archive.archiv_id}`, { method: 'GET' })).status, 401);
  assert.deepEqual(fs.readdirSync(f.roots.Projekte), []);
});
