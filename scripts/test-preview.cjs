const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const source = fs.readFileSync('ui/app.js', 'utf8').split('// ---------- Cached sidebar previews ----------')[1];
const elements = new Map();
function $(id) {
  if (!elements.has(id)) elements.set(id, { hidden: true, disabled: false, style: {}, offsetWidth: 340, offsetHeight: 200, matches: () => false, contains: () => false, addEventListener() {} });
  return elements.get(id);
}
const calls = [];
const ctx = vm.createContext({ renderMd: (el, text) => { el.textContent = text; }, $, clearTimeout, setTimeout, innerWidth: 1000, innerHeight: 760, Date, document: { activeElement: null, addEventListener() {} }, window: { addEventListener() {} }, confirm: () => false,
  invoke: async (command, args) => { if (command === 'cached_purpose') return { text: 'Gecachte KI', method: 'Haiku' }; calls.push(args); return { text: args.purpose ? 'KI-Erklärung' : 'Lokaler Text', method: 'Test', updated: 0 }; }, toast() {} });
vm.runInContext(source, ctx);
const row = { isConnected: true, matches: () => false, contains: () => false, getBoundingClientRect: () => ({ right: 240, top: 100 }) };
ctx.row = row; ctx.entry = { path: '/example.md', name: 'example.md' };
(async () => {
  await vm.runInContext('showPreview(entry, row)', ctx);
  assert.equal($('previewtext').textContent, 'Lokaler Text');
  assert.equal($('previewaitext').textContent, 'Gecachte KI');
  assert.equal(calls.length, 1); // cache retrieval never calls a model
  $('preview').matches = () => true;
  $('previewpurpose').onclick();
  assert.equal($('previewconsent').hidden, false);
  assert.equal(calls.length, 1); // no model call before consent
  vm.runInContext('postponePreviewHide()', ctx);
  await new Promise(r => setTimeout(r, 300));
  assert.equal($('preview').hidden, false);
  $('previewallow').onclick();
  await new Promise(r => setTimeout(r, 0));
  assert.equal($('previewaitext').textContent, 'KI-Erklärung');
  assert.equal($('previewtext').textContent, 'Lokaler Text');
  assert.equal($('previewai').hidden, false);
  vm.runInContext('postponePreviewHide()', ctx);
  await new Promise(r => setTimeout(r, 300));
  assert.equal($('preview').hidden, false);
  $('preview').matches = () => false;
  vm.runInContext('postponePreviewHide()', ctx);
  await new Promise(r => setTimeout(r, 300));
  assert.equal($('preview').hidden, true);
  console.log('Preview inline consent, cache reuse and hover dismissal: passed');
})().catch(e => { console.error(e); process.exitCode = 1; });
