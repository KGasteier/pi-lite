import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import vm from 'node:vm';
import { sessionLabel, connectionLabel, messageText, errorText } from '../state.js';

test('Zustandstexte sind genau die A1-Anzeigen, bereit bleibt ohne Markierung', () => {
  for (const [state, label] of Object.entries({ ruht: 'ruht', arbeitet: 'arbeitet', wartet: 'wartet auf dich', fertig: 'fertig', bereit: '', fehler: 'Fehler' })) assert.equal(sessionLabel(state), label);
  assert.equal(sessionLabel('busy'), 'Zustand unbekannt');
  assert.equal(connectionLabel('unterbrochen'), 'Verbindung unterbrochen');
  assert.equal(connectionLabel('abgemeldet'), 'abgemeldet');
  assert.equal(errorText({ code: 'abgelehnt' }), 'Der Rechner hat abgelehnt. Es wurde nichts gespeichert.');
});

test('Verlauf liest ausschließlich Text, keine Bild-/HTML-Ausführung', () => {
  assert.equal(messageText({ content: [{ type: 'text', text: '<script>nichts ausführen</script>' }, { type: 'image', url: 'https://example.invalid/secret' }] }), '<script>nichts ausführen</script>');
  assert.equal(messageText({ content: 'Text' }), 'Text');
});

test('Service Worker: statische Allowlist, keine API-/Laufzeit-Cachewrites', async () => {
  const source = await fs.readFile(new URL('../sw.js', import.meta.url), 'utf8');
  assert.ok(!source.includes('cache.put('));
  assert.ok(!source.includes('caches.match(request)'));
  assert.equal((source.match(/cache\.addAll/g) || []).length, 1);
  assert.ok(source.includes("request.headers.has('Authorization')"));
  assert.ok(source.includes('url.search'));
  const callbacks = {}, cached = [], replies = [];
  const cache = { addAll: async paths => { cached.push(...paths); }, match: async path => ({ shell: path }) };
  vm.runInNewContext(source, {
    self: { location: { origin: 'https://r2.invalid', href: 'https://r2.invalid/m/sw.js' }, addEventListener: (type, fn) => { callbacks[type] = fn; } },
    URL, caches: { open: async () => cache, keys: async () => [] },
    fetch: () => assert.fail('Inhalt sollte nicht im Service Worker abgefragt werden'),
  });
  let install;
  callbacks.install({ waitUntil: promise => { install = promise; } }); await install;
  assert.ok(cached.includes('/m/app.js')); assert.ok(cached.includes('/ui/transport.js'));
  assert.ok(!cached.some(path => path.includes('/v1/') || path.includes('/api/')));
  for (const path of ['/v1/sessions', '/v1/ereignisse', '/api/sessions', '/m/private.pdf', '/m/?c=SECRET', '/m/index.html?token=SECRET']) {
    callbacks.fetch({ request: new Request(`https://r2.invalid${path}`), respondWith: promise => replies.push(promise) });
  }
  callbacks.fetch({ request: new Request('https://r2.invalid/m/app.js', { headers: { Authorization: 'Bearer demo' } }), respondWith: promise => replies.push(promise) });
  callbacks.fetch({ request: new Request('https://evil.invalid/m/app.js'), respondWith: promise => replies.push(promise) });
  assert.equal(replies.length, 0);
  callbacks.fetch({ request: new Request('https://r2.invalid/m/style.css'), respondWith: promise => replies.push(promise) });
  assert.deepEqual(await replies[0], { shell: '/m/style.css' });
});

test('App-Hülle ist lokal vollständig und Manifest enthält keine Einladung', async () => {
  const manifest = JSON.parse(await fs.readFile(new URL('../manifest.webmanifest', import.meta.url), 'utf8'));
  assert.equal(manifest.start_url, './'); assert.equal(manifest.scope, './');
  for (const icon of manifest.icons) await fs.access(new URL(`../${icon.src}`, import.meta.url));
  const html = await fs.readFile(new URL('../index.html', import.meta.url), 'utf8');
  for (const id of ['list', 'session', 'settings', 'pair']) assert.ok(html.includes(`id="${id}"`));
  assert.ok(html.includes('id="prompt"')); assert.ok(html.includes('id="send"'));
  assert.ok(html.includes('id="results"')); assert.ok(html.includes('aria-live="polite"'));
  const app = await fs.readFile(new URL('../app.js', import.meta.url), 'utf8');
  assert.ok(!app.includes('.innerHTML')); assert.ok(!app.includes('localStorage'));
  assert.ok(!app.includes('EventSource'));
  assert.ok(app.includes('text.textContent = messageText(message)'));
});
