import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import vm from 'node:vm';
import { SessionActions, personalResults, previewKind, attribution } from '../workflow.js';
import { sessionLabel, connectionLabel, errorText, messageText } from '../state.js';

// DOM-Vertragsdummy: prüft Zustände/Bedienaktionen, ersetzt keinen Browser-Screenshot.
class Element {
  constructor(tag = 'div') { this.tag = tag; this.children = []; this.hidden = false; this.disabled = false; this.value = ''; this.dataset = {}; this.textContent = ''; this.className = ''; }
  append(...children) { this.children.push(...children); }
  replaceChildren(...children) { this.children = children; }
  focus() { this.focused = true; }
  get classList() { return { toggle() {}, add() {} }; }
  get firstChild() { return this.children[0]; }
}
async function appFixture(t) {
  const html = await fs.readFile(new URL('../index.html', import.meta.url), 'utf8');
  const elements = new Map([...html.matchAll(/id="([^"]+)"/g)].map(m => [m[1], new Element()]));
  const document = { body: new Element(), hidden: false, addEventListener() {}, getElementById(id) {
    if (elements.has(id)) return elements.get(id);
    const find = node => node.id === id ? node : node.children.map(find).find(Boolean);
    for (const element of elements.values()) { const result = find(element); if (result) return result; }
  }, createElement: tag => new Element(tag) };
  let receive, logout, clearCount = 0, resolveSend;
  const calls = [], listeners = {}, row = { session_id: 's1', titel: 'Handout', workspace: 'Projekte/Seminar', zustand: 'bereit' };
  const transport = {
    hallo: async () => ({ zugangstoken: 'mock', person: 'Mira', slot: 'account03' }),
    sessions: async () => ({ typ: 'sessions_liste', sessions: [row] }),
    events(fn) { receive = fn; fn({ typ: 'verbindung', zustand: 'verbunden' }); return { close() {} }; },
    senden(...args) { calls.push(['senden', ...args]); return new Promise(resolve => { resolveSend = resolve; }); },
    antwort: async (...args) => { calls.push(['antwort', ...args]); return { ...row, typ: 'session_zustand', zustand: 'arbeitet' }; },
    stopp: async (...args) => { calls.push(['stopp', ...args]); return { ...row, typ: 'session_zustand', zustand: 'fertig' }; },
    vorschau: async () => ({ persoenlich: [{ pfad: 'Ergebnisse/Handout.md' }, { pfad: 'Skills/privat.md' }] }),
    ergebnis: async () => new Blob(['<script>Text, nicht ausführen</script>'], { type: 'text/plain' }),
  };
  const location = { origin: 'https://r2.invalid', href: 'https://r2.invalid/slot/account03/m/', pathname: '/slot/account03/m/', hash: '#list' };
  const window = { addEventListener: (name, fn) => { listeners[name] = fn; }, history: { replaceState() {} } };
  const context = vm.createContext({ document, location, window, navigator: {}, URL, URLSearchParams, Blob, File, crypto, console,
    cancelAnimationFrame() {}, requestAnimationFrame() {}, AbortController, Event,
    createTransport(options) { logout = options.onLogout; assert.equal(options.baseUrl, 'https://r2.invalid/slot/account03/'); return transport; },
    parsePairingCode() {}, SessionActions, personalResults, previewKind, attribution, sessionLabel, connectionLabel, errorText, messageText,
    getDevice: async () => ({ geraet_id: 'mock-device', name: 'Miras Handy' }), saveRegistration: async () => {},
    clearRegistration: async () => { clearCount++; }, forgetDevice: async () => {}, signLogin() {},
  });
  const source = (await fs.readFile(new URL('../app.js', import.meta.url), 'utf8')).replace(/^import .*;\n/gm, '');
  await vm.runInContext(`(async () => {${source}\n})()`, context);
  const route = hash => { location.hash = hash; listeners.hashchange(); };
  const tick = () => new Promise(resolve => setImmediate(resolve));
  return { $, receive: msg => receive(msg), logout: () => logout(), clearCount: () => clearCount, route, calls, tick, resolveSend: result => resolveSend(result), location };
  function $(id) { return document.getElementById(id); }
}
const submit = { preventDefault() {} };
test('I2 DOM: Promptknopf bis Bestätigung gesperrt, Rückfrage Nein/Ja explizit, Abbruch ehrlich', async t => {
  const a = await appFixture(t); a.route('#session/s1');
  a.$('prompt').value = 'Kurzes Handout';
  const sent = a.$('prompt-form').onsubmit(submit);
  assert.equal(a.$('send').disabled, true);
  await a.$('prompt-form').onsubmit(submit); assert.equal(a.calls.length, 1);
  assert.match(a.calls[0][3], /^[a-f0-9-]{36}$/);
  a.resolveSend({ typ: 'session_zustand', session_id: 's1', zustand: 'arbeitet' }); await sent;
  assert.equal(a.$('prompt').value, ''); assert.equal(a.$('send').disabled, true);
  a.receive({ typ: 'session_zustand', session_id: 's1', zustand: 'wartet', frage: { id: 'q1', method: 'confirm', message: 'Freigeben?' } });
  assert.equal(a.$('answer-form').hidden, false); assert.equal(a.calls.length, 1, 'keine automatische Antwort');
  a.$('answer-value').value = 'false'; await a.$('answer-form').onsubmit(submit);
  assert.deepEqual(a.calls[1], ['antwort', 's1', 'q1', false]);
  await a.$('stop').onclick(); assert.equal(a.calls[2][0], 'stopp');
  assert.match(a.$('notice').textContent, /Abbruch angefragt/);
});
test('I2 DOM: Seminarleitung markiert, Ergebnisse read-only, 401 leert DOM und Registrierung sofort', async t => {
  const a = await appFixture(t); a.route('#session/s1');
  a.receive({ typ: 'session_ereignis', session_id: 's1', von: { person: 'Aylin', rolle: 'seminarleitung' }, pi_event: { type: 'cocopi_absender', text: 'Bitte kürzer' } });
  const article = a.$('log').children.find(node => node.className === 'seminarleitung');
  assert.equal(article.children[0].textContent, 'Aylin · Seminarleitung'); assert.equal(article.children[1].textContent, 'Bitte kürzer');
  a.route('#results'); await a.tick();
  assert.equal(a.$('results-list').children.length, 1);
  await a.$('results-list').children[0].children[0].onclick();
  assert.equal(a.$('preview').children[0].tag, 'pre'); assert.match(a.$('preview').children[0].textContent, /<script>/);
  a.logout(); await a.tick();
  assert.equal(a.clearCount(), 1);
  assert.equal(a.$('preview').children.length, 0); assert.equal(a.$('log').children.length, 0);
  assert.equal(a.$('results-list').children.length, 0); assert.equal(a.$('sessions').children.length, 0);
  assert.equal(a.$('account').textContent, 'nicht angemeldet');
  assert.equal(a.$('device-name').textContent, 'Dieses Handy');
  assert.equal(a.$('repair').hidden, false);
  assert.equal(a.$('notice').textContent, 'Dein Rechner hat ausgecheckt. Dieses Handy ist abgemeldet.');
  a.receive({ typ: 'sessions_liste', sessions: [{ session_id: 'late', titel: 'Nicht zurückholen' }] });
  assert.equal(a.$('sessions').children.length, 0);
});
