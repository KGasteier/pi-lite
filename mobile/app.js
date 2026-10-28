import { createTransport, parsePairingCode } from '../ui/transport.js';
import { getDevice, saveRegistration, clearRegistration, forgetDevice, signLogin } from './device.js';
import { SessionActions, personalResults, previewKind, attribution } from './workflow.js';
import { sessionLabel, connectionLabel, errorText, messageText } from './state.js';

const $ = id => document.getElementById(id);
// Bei /slot/accountNN/m/ bleibt jede Anfrage im expliziten Slot-Pfad.
const baseUrl = new URL('../', location.href).href;
const transport = createTransport({ baseUrl, getToken: () => token, onLogout: () => signedOut() });
let token = '', welcome, stream, rows = [], selected, lastSeen, connection = 'offline', registered = false;
let generation = 0, invalidated = false, registrationClear = Promise.resolve(), resultFile, resultUrl, resultLoad = 0;
const actions = new SessionActions(transport, () => renderControls());
let cameraStream, scanFrame, pairAbort, currentView = 'list';
const history = new Map();
function notice(text = '') { $('notice').textContent = text; $('notice').hidden = !text; }
function clearPreview() {
  resultLoad++; resultFile = null;
  if (resultUrl) URL.revokeObjectURL(resultUrl);
  resultUrl = null; $('preview').replaceChildren(); $('preview-title').textContent = '';
  $('preview-title').hidden = true; $('share').hidden = true;
}
function signedOut() {
  if (invalidated) return;
  invalidated = true; generation++; token = ''; rows = []; history.clear(); actions.clear(); registered = false;
  welcome = null; lastSeen = null; selected = null; questionKey = ''; pairAbort?.abort(); stopCamera(); stream?.close();
  $('code').value = ''; $('prompt').value = ''; $('pair-status').textContent = '';
  $('device-name').textContent = 'Dieses Handy'; $('device-hint').textContent = '';
  $('answer-fields').replaceChildren(); $('question-text').textContent = ''; $('answer-form').hidden = true;
  $('log').replaceChildren(); $('session-title').textContent = 'Session'; $('workspace').textContent = '';
  $('session-state').textContent = ''; $('action-status').textContent = '';
  $('results-list').replaceChildren(); $('results-status').textContent = ''; clearPreview();
  $('account').textContent = 'nicht angemeldet';
  // Sofortige IDB-Transaktion; Kopplung wartet auf deren Abschluss.
  registrationClear = clearRegistration().catch(() => {
    notice('Der Handy-Speicher konnte nicht geleert werden. Schließe cocopi und lösche die Websitedaten im Browser.');
    throw new Error('Der Handy-Speicher konnte nicht geleert werden. Lösche zuerst die Websitedaten.');
  });
  registrationClear.catch(() => {});
  connection = 'abgemeldet'; $('connection').textContent = connectionLabel(connection);
  document.body.classList.add('stale'); location.hash = 'list';
  notice('Dein Rechner hat ausgecheckt. Dieses Handy ist abgemeldet.'); renderList(); renderControls();
}
function showConnection(state) {
  if (state === 'abgemeldet') { signedOut(); return; }
  connection = state;
  $('connection').textContent = connectionLabel(state);
  document.body.classList.toggle('stale', state !== 'verbunden');
  renderList(); renderSession();
}
function staleSuffix() {
  return connection !== 'verbunden' && lastSeen ? ` · Stand ${lastSeen.toLocaleTimeString('de', { hour: '2-digit', minute: '2-digit' })}` : '';
}
function renderList() {
  $('sessions').replaceChildren();
  $('list-hint').textContent = !token ? 'Melde dich an, um deine Sessions auf R2 zu sehen.' : rows.length ? 'Deine Sessions auf R2.' : 'Noch keine Session. Öffne eine Session am Rechner.';
  $('connect').hidden = Boolean(token) || registered || invalidated;
  $('repair').hidden = !invalidated;
  $('reconnect').hidden = !registered || (Boolean(token) && connection !== 'unterbrochen');
  $('settings-pair').hidden = Boolean(token) || invalidated;
  $('logout').hidden = !token;
  $('key-renew').hidden = Boolean(token) || registered;
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
  renderControls();
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
    who.textContent = attribution(message);
    if (message.von?.rolle === 'seminarleitung') article.className = 'seminarleitung';
    text.textContent = messageText(message); // Untrusted Text: kein HTML, keine externen Bilder/Links.
    article.append(who, text); log.append(article);
  }
}
let questionKey = '';
function renderControls() {
  const row = rows.find(r => r.session_id === selected), state = actions.state(selected);
  const ready = Boolean(token) && connection === 'verbunden' && Boolean(row);
  $('send').disabled = !ready || !['bereit', 'fertig'].includes(row?.zustand) || state.status !== 'idle';
  $('prompt').disabled = !ready || state.status !== 'idle';
  $('stop').hidden = !['arbeitet', 'wartet'].includes(row?.zustand);
  $('stop').disabled = !ready || state.status === 'pending';
  $('send').setAttribute('aria-busy', String(state.status === 'pending' && state.kind === 'senden'));
  $('answer-send').setAttribute('aria-busy', String(state.status === 'pending' && state.kind === 'antwort'));
  $('action-status').textContent = state.status === 'pending' ? 'Warte auf Bestätigung von R2 …'
    : state.status === 'uncertain' ? 'Versand unklar. Nicht erneut senden. Prüfe den Auftrag am Rechner.' : '';
  const q = row?.frage, key = q ? `${selected}/${q.id}` : '';
  $('answer-form').hidden = !q;
  $('answer-send').disabled = !ready || state.status !== 'idle';
  if (key === questionKey) return;
  questionKey = key; $('answer-fields').replaceChildren(); $('question-text').textContent = '';
  if (!q) return;
  $('question-text').textContent = q.message || q.title || q.titel || 'Der Agent wartet auf deine Antwort.';
  let input;
  if (q.method === 'confirm' || q.method === 'select') {
    input = document.createElement('select');
    const placeholder = document.createElement('option'); placeholder.value = ''; placeholder.textContent = 'Bitte wählen'; placeholder.disabled = true; placeholder.selected = true; input.append(placeholder);
    const options = q.method === 'confirm' ? [['false', 'Nein'], ['true', 'Ja']] : (q.options || []).map(value => [value, value]);
    for (const [value, text] of options) { const option = document.createElement('option'); option.value = value; option.textContent = text; input.append(option); }
  } else if (['input', 'editor'].includes(q.method)) {
    input = document.createElement('textarea'); input.rows = q.method === 'editor' ? 5 : 2; input.maxLength = 16000;
    input.value = q.prefill || q.value || '';
  } else { $('question-text').textContent += ' Bitte am Rechner beantworten.'; $('answer-send').disabled = true; return; }
  input.id = 'answer-value'; input.required = true;
  const label = document.createElement('label'); label.htmlFor = input.id; label.textContent = 'Deine Antwort';
  $('answer-fields').append(label, input);
}
function receive(msg) {
  if (invalidated && msg.typ !== 'verbindung') return;
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
      if (!id || !list.some(m => m.id === id)) list.push({ ...event.message, ...(msg.von ? { von: msg.von } : {}) });
    }
    if (event?.type === 'cocopi_absender' && msg.von?.rolle === 'seminarleitung') {
      list.push({ role: 'user', content: event.text, von: msg.von });
    }
    // Vollständige message_end-Blöcke vermeiden Delta-Doppelzählung nach Replay.
    history.set(msg.session_id, list);
  }
  if (msg.typ === 'fehler') {
    notice(errorText(msg));
    if (msg.code === 'neu_abgleichen' || msg.code === 'replay_luecke') {
      history.clear();
      const epoch = generation;
      transport.sessions().then(packet => { if (epoch === generation) receive(packet); }).catch(error => { if (!invalidated) notice(errorText(error)); });
    }
  }
  renderList(); renderSession();
}
async function login(device) {
  const epoch = generation;
  registered = true;
  const response = await transport.hallo({ ...device, sign: payload => signLogin(device, payload) });
  if (epoch !== generation) return;
  invalidated = false;
  token = response.zugangstoken;
  welcome = response;
  $('account').textContent = `${response.person || ''} · ${response.slot} @ R2`;
  $('device-name').textContent = device.name;
  $('device-hint').textContent = 'Der private Geräteschlüssel bleibt auf diesem Handy.';
  notice('');
  stream?.close();
  stream = transport.events(receive);
  const snapshot = await transport.sessions();
  if (epoch === generation) receive(snapshot);
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
  currentView = ['settings', 'pair', 'session', 'results'].includes(view) ? view : 'list';
  selected = view === 'session' ? decodeURIComponent(id || '') : null;
  $('prompt').value = ''; questionKey = '';
  if (currentView !== 'results') clearPreview();
  for (const section of ['list', 'session', 'settings', 'pair', 'results']) $(section).hidden = section !== currentView;
  if (currentView === 'results') loadResults();
  if (currentView === 'pair') {
    const supported = 'BarcodeDetector' in window && Boolean(navigator.mediaDevices?.getUserMedia);
    $('scan').hidden = !supported; $('camera-hint').hidden = !supported; $('type-code').hidden = !supported;
  }
  renderSession(); $('main').focus({ preventScroll: true });
}
function openPair() { notice(''); location.hash = 'pair'; }
$('connect').onclick = openPair;
$('repair').onclick = openPair;
$('reconnect').onclick = async () => {
  $('reconnect').disabled = true;
  try { await login(await getDevice()); } catch (error) { if (!invalidated) notice(errorText(error)); }
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
    if (parsed.art !== 'k') throw new Error('Nutze den Kopplungscode vom eingecheckten Rechner. Dieses Handy kann nicht selbst einchecken.');
    await registrationClear;
    const device = await getDevice();
    const body = { ...parsed, pubkey: device.pubkey, geraetename: device.name, geraeteart: 'handy' };
    $('pair-status').textContent = parsed.art === 'e' ? 'Einladung wird geprüft …' : 'Warte auf Bestätigung am Rechner …';
    const options = { signal, onPending: () => { $('pair-status').textContent = 'Warte auf Bestätigung am bestehenden Gerät …'; } };
    const result = parsed.art === 'e' ? await transport.einloesen(body, options) : await transport.koppeln(body, options);
    if (signal.aborted) return;
    if (!result.geraet_id) throw new Error('Die Anmeldung wurde noch nicht bestätigt. Prüfe den Rechner.');
    await saveRegistration(result);
    if (signal.aborted) { await clearRegistration(); return; }
    invalidated = false;
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
$('key-renew').onclick = async () => {
  if (token || registered) return;
  $('key-renew').disabled = true;
  try {
    await registrationClear; await forgetDevice(); await getDevice();
    $('device-hint').textContent = 'Ein neuer Geräteschlüssel wurde auf diesem Handy erzeugt. Du kannst erneut koppeln.';
  } catch (error) { notice(errorText(error)); }
  finally { $('key-renew').disabled = false; }
};
$('logout').onclick = async () => {
  $('logout').disabled = true;
  try {
    await transport.abmelden(); await registrationClear; await forgetDevice();
    $('device-hint').textContent = ''; notice('Dieses Handy ist abgemeldet.');
  } catch (error) {
    // Bei Funkloch nicht vortäuschen, dass der Hub widerrufen hat. Schlüssel für erneuten Versuch behalten.
    if (!invalidated) notice(`Die Abmeldung auf R2 wurde nicht bestätigt. ${errorText(error)} Der Geräteschlüssel bleibt für einen erneuten Versuch erhalten.`);
  } finally { $('logout').disabled = false; }
};
$('prompt-form').onsubmit = async event => {
  event.preventDefault();
  if ($('send').disabled || !$('prompt').value.trim()) return;
  const id = selected, epoch = generation, text = $('prompt').value.trim();
  notice('');
  try {
    const result = await actions.run(id, 'senden', text);
    if (result && epoch === generation) { if (selected === id) $('prompt').value = ''; receive(result); }
  } catch (error) { if (epoch === generation && !invalidated) notice(errorText(error)); }
};
$('answer-form').onsubmit = async event => {
  event.preventDefault();
  if ($('answer-send').disabled) return;
  const id = selected, epoch = generation, q = rows.find(row => row.session_id === id)?.frage;
  if (!q || !$('answer-value')) return;
  const value = $('answer-value').value;
  try {
    const result = await actions.run(id, 'antwort', q.id, q.method === 'confirm' ? value === 'true' : value);
    if (result && epoch === generation) receive(result);
  } catch (error) { if (epoch === generation && !invalidated) notice(errorText(error)); }
};
$('stop').onclick = async () => {
  if ($('stop').disabled) return;
  const id = selected, epoch = generation;
  try {
    const result = await actions.run(id, 'stopp');
    if (result && epoch === generation) { receive(result); notice('Abbruch angefragt. Warte auf den bestätigten Zustand von R2.'); }
  } catch (error) { if (epoch === generation && !invalidated) notice(errorText(error)); }
};
async function loadResults() {
  clearPreview(); const load = resultLoad, epoch = generation;
  $('results-list').replaceChildren();
  if (!token) { $('results-status').textContent = 'Kopple dieses Handy zuerst mit deinem eingecheckten Rechner.'; return; }
  $('results-status').textContent = 'Ergebnisse werden geladen …'; $('results-reload').disabled = true;
  try {
    const packet = await transport.vorschau();
    if (epoch !== generation || load !== resultLoad || currentView !== 'results') return;
    const files = personalResults(packet);
    $('results-status').textContent = files.length ? `${files.length} Ergebnisse · nur ansehen` : 'Noch keine Ergebnisse unter Ergebnisse/.';
    for (const row of files) {
      const li = document.createElement('li'), button = document.createElement('button');
      button.textContent = row.pfad.slice('Ergebnisse/'.length);
      button.onclick = () => openResult(row.pfad); li.append(button); $('results-list').append(li);
    }
  } catch (error) { if (epoch === generation && load === resultLoad) $('results-status').textContent = errorText(error); }
  finally { $('results-reload').disabled = false; }
}
async function openResult(path) {
  clearPreview(); const load = resultLoad, epoch = generation, kind = previewKind(path);
  if (!kind) { $('results-status').textContent = 'Dieses Format lässt sich hier nicht anzeigen. Öffne es am Rechner.'; return; }
  $('results-status').textContent = 'Vorschau wird geladen …';
  try {
    const blob = await transport.ergebnis(path);
    if (epoch !== generation || load !== resultLoad || currentView !== 'results') return;
    const name = path.split('/').at(-1);
    let safeBlob;
    if (kind === 'text') {
      const text = await blob.text();
      if (epoch !== generation || load !== resultLoad) return;
      const pre = document.createElement('pre'); pre.textContent = text; $('preview').append(pre);
      safeBlob = new Blob([text], { type: 'text/plain' });
    } else {
      if (kind === 'image' && !/^image\/(png|jpeg|gif|webp|avif)$/.test(blob.type)) throw new Error('Das Ergebnis ist kein unterstütztes Bild. Öffne es am Rechner.');
      if (kind === 'pdf' && (blob.type !== 'application/pdf' || await blob.slice(0, 5).text() !== '%PDF-')) throw new Error('Das Ergebnis ist kein lesbares PDF. Öffne es am Rechner.');
      if (epoch !== generation || load !== resultLoad) return;
      safeBlob = blob;
      resultUrl = URL.createObjectURL(blob);
      if (kind === 'image') {
        const image = document.createElement('img'); image.src = resultUrl; image.alt = name; $('preview').append(image);
      } else {
        // PDF nur im Browser-Viewer, niemals als HTML oder in einer eigenen Engine.
        const frame = document.createElement('iframe'); frame.title = `PDF: ${name}`; frame.src = resultUrl;
        const link = document.createElement('a'); link.href = resultUrl; link.target = '_blank'; link.rel = 'noopener noreferrer'; link.textContent = 'PDF im Browser öffnen';
        $('preview').append(link, frame);
      }
    }
    resultFile = new File([safeBlob], kind === 'text' ? `${name}.txt` : name, { type: safeBlob.type });
    $('preview-title').textContent = name; $('preview-title').hidden = false;
    $('results-status').textContent = 'Vorschau · nur ansehen';
    $('share').hidden = !(navigator.share && navigator.canShare?.({ files: [resultFile] }));
    $('preview-title').tabIndex = -1; $('preview-title').focus();
  } catch (error) { if (epoch === generation && load === resultLoad) $('results-status').textContent = errorText(error); }
}
$('results-reload').onclick = loadResults;
$('share').onclick = async () => {
  if (!resultFile) return;
  const epoch = generation;
  try { await navigator.share({ files: [resultFile] }); }
  catch (error) { if (error.name !== 'AbortError' && epoch === generation) $('results-status').textContent = 'Teilen war nicht möglich. Versuche es erneut oder öffne das Ergebnis am Rechner.'; }
};
window.addEventListener('hashchange', route);
window.addEventListener('pagehide', () => {
  generation++; stopCamera(); pairAbort?.abort(); stream?.close(); token = ''; rows = []; history.clear(); actions.clear();
  $('log').replaceChildren(); $('sessions').replaceChildren(); $('results-list').replaceChildren(); $('prompt').value = '';
  $('answer-fields').replaceChildren(); $('question-text').textContent = ''; clearPreview();
});
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
} catch (error) { renderList(); if (!invalidated) { $('device-hint').textContent = errorText(error); notice(errorText(error)); } }
if ('serviceWorker' in navigator) navigator.serviceWorker.register('./sw.js', { scope: './' }).catch(() => notice('Die App-Hülle konnte nicht gespeichert werden. Online kannst du cocopi weiter nutzen.'));
