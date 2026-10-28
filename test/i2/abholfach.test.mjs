import test from 'node:test';
import assert from 'node:assert/strict';
import { missing, importHub, identity } from './harness.mjs';
import { tarGz } from './tar-fixtures.mjs';
const absent = missing('empfang/ausweis.mjs');
const options = { skip: absent.length ? `Fehlt im Hub: ${absent.join(', ')}` : false };

test('Abholfach-Kryptografie: nur richtiger Geräteschlüssel; falscher Schlüssel und Manipulation scheitern', options, async () => {
  const { encryptArchive, decryptArchive } = await importHub('empfang/ausweis.mjs');
  const owner = identity(), stranger = identity(), now = 100000;
  const archive = tarGz([{ name: 'Ergebnisse/privat.md', data: 'Nur für das Testgerät' }]);
  const envelope = encryptArchive(archive, owner.archiv_pubkey, 'nur-test-checkin', now);
  assert.deepEqual(decryptArchive(envelope, owner.archive.privateKey, now), archive);
  assert.equal(envelope.ablauf - now, 72 * 3600e3);
  assert.throws(() => decryptArchive(envelope, stranger.archive.privateKey, now));
  assert.throws(() => decryptArchive({ ...envelope, checkin_id: 'fremder-checkin' }, owner.archive.privateKey, now));
  assert.throws(() => decryptArchive(envelope, owner.archive.privateKey, envelope.ablauf));
  const ciphertext = Buffer.from(envelope.chiffrat, 'base64url'); ciphertext[0] ^= 1;
  assert.throws(() => decryptArchive({ ...envelope, chiffrat: ciphertext.toString('base64url') }, owner.archive.privateKey, now));
  assert.ok(!JSON.stringify(envelope).includes('Nur für das Testgerät'));
});
