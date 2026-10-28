import { createTransport, parsePairingCode } from '../ui/transport.js';
import { getDevice, saveRegistration, forgetDevice, signLogin } from './device.js';
import { sessionLabel, connectionLabel, errorText, messageText } from './state.js';

const $ = id => document.getElementById(id);
const transport = createTransport({ baseUrl: location.origin, getToken: () => token });
let token = '', welcome, stream, rows = [], selected, lastSeen, connection = 'offline', registered = false;
let cameraStream, scanFrame, pairAbort, currentView = 'list';
const history = new Map();
function notice(text = '') { $('notice').textContent = text; $('notice').hidden = !text; }
function showConnection(state) {
  connection = state;
  $('connection').textContent = connectionLabel(state);
  document.body.classList.toggle('stale', state !== 'verbunden');
  if (state === 'abgemeldet') {
    token = ''; rows = []; history.clear(); welcome = null;
    $('account').textContent = 'nicht angemeldet';
    notice('Dieses Gerät wurde abgemeldet. Melde dich mit einer neuen Einladung oder durch Koppeln wieder an.');
    // Widerruf: keine privaten Inhalte im DOM zurücklassen.
    $('log').replaceChildren(); $('session-title').textContent = 'Session'; $('workspace').textContent = '';
    location.hash = 'list';
  }
  renderList(); renderSession();
}
function staleSuffix() {
  return connection !== 'verbunden' && lastSeen ? ` · Stand ${lastSeen.toLocaleTimeString('de', { hour: '2-digit', minute: '2-digit' })}` : '';
}
function renderList() {
  $('sessions').replaceChildren();
  $('list-hint').textContent = !token ? 'Melde dich an, um deine Sessions auf R2 zu sehen.' : rows.length ? 'Deine Sessions auf R2.' : 'Noch keine Session. Öffne eine Session am Rechner.';
  $('connect').hidden = Boolean(token) || registered;
  $('reconnect').hidden = !registered || (Boolean(token) && connection !== 'unterbrochen');
  $('settings-pair').hidden = Boolean(token);
  $('logout').hidden = !token;
  for (const row of rows) {
    const li = document.createElement('li'), link = document.createElement('a');
    link.href = `#session/${encodeURIComponent(row.session_id)}`;
    link.dataset.state = row.zustand;
    const title = document.createElement('span'), state = document.createElement('small');
    title.textContent = row.titel || 'Session';
    state.textContent = sessionLabel(row.zustand) + staleSuffix();
    link.append(title, state); li.append(link); $('sessions').append(li);
  }
}
function renderSession() {
  if (!selected) return;
  const row = rows.find(r => r.session_id === selected);
  if (!row) {
    $('session-title').textContent = 'Session'; $('workspace').textContent = ''; $('session-state').textContent = '';
    $('log').replaceChildren(); return;
  }
  $('session-title').textContent = row.titel || 'Session';
  $('workspace').textContent = row.workspace || '';
  $('session-state').textContent = sessionLabel(row.zustand) + staleSuffix();
  const log = $('log'); log.replaceChildren();
  const hint = document.createElement('p'); hint.className = 'muted';
  hint.textContent = 'Hier erscheinen empfangene Nachrichten. Ältere Nachrichten sind in dieser Version noch nicht vollständig abrufbar.';
  log.append(hint);
  for (const message of history.get(selected) || []) {
    const article = document.createElement('article'), who = document.createElement('small'), text = document.createElement('p');
    who.textContent = ({ user: 'Du', assistant: 'Agent', toolResult: 'Werkzeug' })[message.role] || 'Nachricht';
    text.textContent = messageText(message); // Untrusted Text: kein HTML, keine externen Bilder/Links.
    article.append(who, text); log.append(article);
  }
  if (row.frage) {
    const question = document.createElement('article'), label = document.createElement('small'), text = document.createElement('p');
    label.textContent = 'Rückfrage · am Rechner beantworten';
    text.textContent = row.frage.message || row.frage.titel || 'Der Agent wartet auf deine Antwort.';
    question.append(label, text); log.append(question);
  }
}
function receive(msg) {
  if (msg.typ === 'verbindung') { showConnection(msg.zustand); return; }
  if (msg.typ === 'puls') { lastSeen = new Date(msg.t || Date.now()); return; }
  lastSeen = new Date();
  if (msg.typ === 'sessions_liste') rows = msg.sessions || [];
  if (msg.typ === 'session_zustand') {
    const index = rows.findIndex(r => r.session_id === msg.session_id);
    if (index < 0) rows.push(msg); else rows[index] = msg;
  }
  if (msg.typ === 'session_ereignis') {
    const list = history.get(msg.session_id) || [], event = msg.pi_event;
    if (event?.type === 'message_end' && event.message) {
      const id = event.message?.id;
      if (!id || !list.some(m => m.id === id)) list.push(event.message);
    }
    // Vollständige message_end-Blöcke vermeiden Delta-Doppelzählung nach Replay.
    history.set(msg.session_id, list);
  }
  if (msg.typ === 'fehler') {
    notice(errorText(msg));
    if (msg.code === 'neu_abgleichen' || msg.code === 'replay_luecke') {
      history.clear();
      transport.sessions().then(receive).catch(error => notice(errorText(error)));
    }
  }
  renderList(); renderSession();
}
async function login(device) {
  registered = true;
  const response = await transport.hallo({ ...device, sign: payload => signLogin(device, payload) });
  token = response.zugangstoken;
  welcome = response;
  $('account').textContent = `${response.person || ''} · ${response.slot} @ R2`;
  $('device-name').textContent = device.name;
  $('device-hint').textContent = 'Der private Geräteschlüssel bleibt auf diesem Handy.';
  notice('');
  stream?.close();
  stream = transport.events(receive);
  receive(await transport.sessions());
}
function stopCamera() {
  cancelAnimationFrame(scanFrame);
  cameraStream?.getTracks().forEach(track => track.stop()); cameraStream = null;
  $('camera').srcObject = null; $('camera').hidden = true;
}
function route() {
  stopCamera();
  if (currentView === 'pair') pairAbort?.abort();
  const [view, id] = location.hash.slice(1).split('/');
  currentView = ['settings', 'pair', 'session'].includes(view) ? view : 'list';
  selected = view === 'session' ? decodeURIComponent(id || '') : null;
  for (const section of ['list', 'session', 'settings', 'pair']) $(section).hidden = section !== currentView;
  if (currentView === 'pair') {
    const supported = 'BarcodeDetector' in window && Boolean(navigator.mediaDevices?.getUserMedia);
    $('scan').hidden = !supported; $('camera-hint').hidden = !supported; $('type-code').hidden = !supported;
  }
  renderSession(); $('main').focus({ preventScroll: true });
}
function openPair() { notice(''); location.hash = 'pair'; }
$('connect').onclick = openPair;
$('reconnect').onclick = async () => {
  $('reconnect').disabled = true;
  try { await login(await getDevice()); } catch (error) { notice(errorText(error)); }
  finally { $('reconnect').disabled = false; renderList(); }
};
$('settings-pair').onclick = openPair;
$('type-code').onclick = event => { event.preventDefault(); stopCamera(); $('code').focus(); };
$('code').oninput = () => {
  try {
    const parsed = parsePairingCode($('code').value);
    $('redeem').textContent = parsed.art === 'e' ? 'Anmelden' : 'Mit Rechner koppeln';
  } catch { $('redeem').textContent = 'Mit Rechner koppeln'; }
};
$('pair-form').onsubmit = async event => {
  event.preventDefault(); notice(''); stopCamera(); pairAbort?.abort(); pairAbort = new AbortController();
  const signal = pairAbort.signal;
  $('redeem').disabled = true;
  try {
    const parsed = parsePairingCode($('code').value);
    if (parsed.art === 'b') throw new Error('Rechner bestätigen ist hier noch nicht verfügbar. Nutze in dieser Version die Bestätigung am Rechner.');
    const device = await getDevice();
    const body = { ...parsed, pubkey: device.pubkey, geraetename: device.name, geraeteart: 'handy' };
    $('pair-status').textContent = parsed.art === 'e' ? 'Einladung wird geprüft …' : 'Warte auf Bestätigung am Rechner …';
    const options = { signal, onPending: () => { $('pair-status').textContent = 'Warte auf Bestätigung am bestehenden Gerät …'; } };
    const result = parsed.art === 'e' ? await transport.einloesen(body, options) : await transport.koppeln(body, options);
    if (signal.aborted) return;
    if (!result.geraet_id) throw new Error('Die Anmeldung wurde noch nicht bestätigt. Prüfe den Rechner.');
    await saveRegistration(result);
    await login(await getDevice());
    $('code').value = '';
    $('pair-status').textContent = '';
    location.hash = 'list';
    notice(`Gekoppelt mit ${welcome.slot} @ R2.`);
  } catch (error) { if (!signal.aborted) $('pair-status').textContent = errorText(error); }
  finally { $('redeem').disabled = false; }
};
$('scan').onclick = async () => {
  stopCamera(); notice('');
  try {
    const detector = new BarcodeDetector({ formats: ['qr_code'] });
    cameraStream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'environment' }, audio: false });
    if (currentView !== 'pair') { stopCamera(); return; }
    $('camera').srcObject = cameraStream; $('camera').hidden = false; await $('camera').play();
    const scan = async () => {
      if (!cameraStream) return;
      try {
        const [result] = await detector.detect($('camera'));
        if (result) {
          parsePairingCode(result.rawValue); // QR startet niemals automatisch eine Anmeldung.
          $('code').value = result.rawValue; $('code').dispatchEvent(new Event('input'));
          stopCamera(); $('code').focus(); return;
        }
      } catch { /* kein Code im Bild; Eintippen bleibt erreichbar */ }
      scanFrame = requestAnimationFrame(scan);
    };
    scanFrame = requestAnimationFrame(scan);
  } catch { stopCamera(); notice('Die Kamera ist nicht verfügbar. Tippe den Code ein.'); $('code').focus(); }
};
$('logout').onclick = async () => {
  $('logout').disabled = true;
  try {
    await transport.abmelden(); await forgetDevice(); token = ''; history.clear(); registered = false;
    showConnection('abgemeldet'); $('device-hint').textContent = ''; notice('Dieses Handy ist abgemeldet.');
  } catch (error) {
    // Bei Funkloch nicht vortäuschen, dass der Hub widerrufen hat. Schlüssel für erneuten Versuch behalten.
    notice(`Die Abmeldung auf R2 wurde nicht bestätigt. ${errorText(error)} Der Geräteschlüssel bleibt für einen erneuten Versuch erhalten.`);
  } finally { $('logout').disabled = false; }
};
window.addEventListener('hashchange', route);
window.addEventListener('pagehide', () => { stopCamera(); pairAbort?.abort(); stream?.close(); token = ''; });
window.addEventListener('pageshow', event => { if (event.persisted) location.reload(); });
document.addEventListener('visibilitychange', () => { if (document.hidden) stopCamera(); });

// Eingehende Codes entfernen, bevor weitere Requests ausgelöst werden. Niemals Hosts aus QR übernehmen.
const initial = location.hash.startsWith('#1.') || new URLSearchParams(location.search).has('c');
if (initial) {
  const input = location.href;
  window.history.replaceState(null, '', `${location.pathname}#pair`);
  $('code').value = input; $('code').dispatchEvent(new Event('input'));
}
route();
try {
  const device = await getDevice();
  if (device.geraet_id) await login(device);
} catch (error) { renderList(); $('device-hint').textContent = errorText(error); notice(errorText(error)); }
if ('serviceWorker' in navigator) navigator.serviceWorker.register('./sw.js', { scope: './' }).catch(() => notice('Die App-Hülle konnte nicht gespeichert werden. Online kannst du cocopi weiter nutzen.'));
