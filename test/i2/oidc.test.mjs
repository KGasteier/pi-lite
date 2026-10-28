import test from 'node:test';
import assert from 'node:assert/strict';
import { missing, importHub } from './harness.mjs';
const absent = missing('empfang/oidc.mjs', 'empfang/test/fake-oidc.mjs');
const options = { skip: absent.length ? `Fehlt im Hub: ${absent.join(', ')}` : false, timeout: 15000 };

test('OIDC: lokaler JWKS-Issuer prüft RS256 und ES256; Rollen nur aus signierten Gruppen', options, async t => {
  const { createFakeOidc } = await importHub('empfang/test/fake-oidc.mjs');
  const { Oidc } = await importHub('empfang/oidc.mjs');
  const fake = await createFakeOidc(); t.after(() => fake.close());
  const verifier = new Oidc({ issuer: fake.issuer, clientId: fake.clientId, allowHttp: true });
  for (const alg of ['RS256', 'ES256']) {
    const result = await verifier.verify(fake.token({}, { alg }), fake.nonce);
    assert.equal(result.rolle, 'teilnehmende');
    const leader = await verifier.verify(fake.token({ groups: ['cocopi-seminarleitung'] }, { alg }), fake.nonce);
    assert.equal(leader.rolle, 'seminarleitung');
  }
  const sec = Math.floor(Date.now() / 1000);
  for (const claims of [
    { iss: fake.issuer + '/falsch' }, { aud: 'fremder-client' }, { exp: sec - 1 },
    { iat: sec + 3600 }, { nonce: 'falsch-nonce-123456789' }, { groups: [] },
    { groups: ['unbekannt'], rolle: 'seminarleitung' },
  ]) await assert.rejects(verifier.verify(fake.token(claims), fake.nonce));
  const token = fake.token().split('.');
  token[1] = Buffer.from(JSON.stringify({ groups: ['cocopi-seminarleitung'] })).toString('base64url');
  await assert.rejects(verifier.verify(token.join('.'), fake.nonce), 'Manipuliertes Token wird abgewiesen');
});
