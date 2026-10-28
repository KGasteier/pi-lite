import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import vm from 'node:vm';

test('I2: Service-Worker-Allowlist bleibt unter dem Slotpräfix', async () => {
  const source = await fs.readFile(new URL('../sw.js', import.meta.url), 'utf8');
  const callbacks = {}, paths = [];
  vm.runInNewContext(source, {
    self: { location: { origin: 'https://r2.invalid', href: 'https://r2.invalid/slot/account03/m/sw.js' }, addEventListener: (type, fn) => { callbacks[type] = fn; } },
    URL, caches: { open: async () => ({ addAll: async values => paths.push(...values) }) },
  });
  let installed; callbacks.install({ waitUntil: promise => { installed = promise; } }); await installed;
  assert.ok(paths.every(path => path.startsWith('/slot/account03/')));
  assert.ok(paths.includes('/slot/account03/ui/transport.js'));
  assert.ok(paths.includes('/slot/account03/m/workflow.js'));
});
