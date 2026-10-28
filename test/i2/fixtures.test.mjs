import test from 'node:test';
import assert from 'node:assert/strict';
import { gunzipSync } from 'node:zlib';
import { tarGz, readTarGz } from './tar-fixtures.mjs';

test('Testarchive: UTF-8-Inhalt, Rechte und gültige Prüfsumme', () => {
  const archive = tarGz([{ name: 'Ergebnisse/a.md', data: 'Grüße' }]);
  assert.equal(readTarGz(archive).get('Ergebnisse/a.md').toString(), 'Grüße');
  const raw = gunzipSync(archive);
  assert.equal(raw.subarray(100, 107).toString(), '0000644');
  assert.equal(raw.length % 512, 0);
});

test('Testarchive: 3-GiB-Größe steht nur im Header, nicht als große Allokation', () => {
  const archive = tarGz([{ name: 'riesig.bin', size: 3 * 1024 ** 3 }]);
  assert.ok(archive.length < 1024);
  const raw = gunzipSync(archive);
  assert.equal(raw.length, 1536);
  assert.equal(Number.parseInt(raw.subarray(124, 135).toString(), 8), 3 * 1024 ** 3);
  assert.throws(() => readTarGz(archive));
});

test('Testauswertung lehnt Elternpfade, Absolutpfade und Links ab', () => {
  for (const entry of [{ name: '../fremd' }, { name: '/absolut' }, { name: 'link', type: '2', link: '../fremd' }]) {
    assert.throws(() => readTarGz(tarGz([entry])));
  }
});
