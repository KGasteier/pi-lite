// Gemeinsamer, Tauri-freier HTTP/SSE-Transport. Keine Zugangsdaten in URLs.
export class TransportError extends Error {
  constructor(code, was, tun = '', status = 0) {
    super([was, tun].filter(Boolean).join(' '));
    Object.assign(this, { name: 'TransportError', code, was, tun, status });
  }
}

export function parsePairingCode(input, art = 'k') {
  let value = String(input ?? '').trim();
  if (/^https?:\/\//i.test(value)) {
    const url = new URL(value);
    if (url.username || url.password) throw new Error('Dieser Link enthält Zugangsdaten. Nutze nur den Code.');
    value = url.hash ? url.hash.slice(1) : url.searchParams.get('c') || '';
  } else if (value.startsWith('#')) value = value.slice(1);
  else if (value.startsWith('?')) value = new URLSearchParams(value).get('c') || '';
  const parts = /^1\.([ekb])\.(.+)$/i.exec(value);
  if (parts) { art = parts[1].toLowerCase(); value = parts[2]; }
  else if (value.includes('.')) throw new Error('Dieser Code hat ein unbekanntes Format. Prüfe die Zeichen.');
  if (!['e', 'k', 'b'].includes(art)) throw new Error('Dieser Code hat eine unbekannte Art.');
  if (!/^(?:[0-9A-HJKMNP-TV-Z]{16}|[0-9A-HJKMNP-TV-Z]{4}(?:-[0-9A-HJKMNP-TV-Z]{4}){3})$/i.test(value)) {
    throw new Error('Dieser Code passt zu keiner Einladung. Prüfe die Zeichen oder öffne den Link direkt.');
  }
  return { art, code: value.replaceAll('-', '').toUpperCase(), protokoll: 1 };
}

// Inkrementeller SSE-Parser: CRLF, geteilte UTF-8-Zeichen, mehrzeilige Daten.
export function createSSEParser(onEvent) {
  let buffer = '', data = [], id, event = '';
  function line(text) {
    if (!text) {
      if (data.length) onEvent({ id, event: event || 'message', data: data.join('\n') });
      data = []; id = undefined; event = ''; return;
    }
    if (text.startsWith(':')) return;
    const colon = text.indexOf(':');
    const field = colon < 0 ? text : text.slice(0, colon);
    let value = colon < 0 ? '' : text.slice(colon + 1);
    if (value.startsWith(' ')) value = value.slice(1);
    if (field === 'data') data.push(value);
    if (field === 'event') event = value;
    if (field === 'id' && !value.includes('\0')) id = value;
  }
  return chunk => {
    buffer += chunk;
    if (buffer.length > 2 * 1024 * 1024) throw new Error('Ereignis zu groß.');
    let match;
    while ((match = /\r\n|\r|\n/.exec(buffer))) {
      if (match[0] === '\r' && match.index === buffer.length - 1) break;
      line(buffer.slice(0, match.index));
      buffer = buffer.slice(match.index + match[0].length);
    }
  };
}

export function createTransport({ baseUrl, getToken, pulseTimeoutMs = 45_000, reconnectMs = 1_000 }) {
  const base = new URL(baseUrl, globalThis.location?.href);
  if (!['https:', 'http:'].includes(base.protocol) || base.username || base.password || base.search || base.hash) {
    throw new Error('Die R2-Adresse ist ungültig.');
  }
  if (base.protocol !== 'https:' && !['localhost', '127.0.0.1', '[::1]'].includes(base.hostname)) {
    throw new Error('R2 braucht eine geschützte HTTPS-Verbindung.');
  }
  const streams = new Set();
  let signedOut = false;
  const url = path => new URL(base.pathname.replace(/\/$/, '') + path, base.origin);
  async function headers(auth = true) {
    const token = auth ? await getToken() : null;
    return { Accept: 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) };
  }
  function logout() {
    signedOut = true;
    for (const stream of streams) stream.close('abgemeldet');
  }
  async function request(path, { method = 'GET', body, auth = true, signal } = {}) {
    const response = await fetch(url(path), {
      method, headers: { ...await headers(auth), ...(body ? { 'Content-Type': 'application/json' } : {}) },
      body: body ? JSON.stringify(body) : undefined, signal, cache: 'no-store',
      credentials: 'omit', redirect: 'error', referrerPolicy: 'no-referrer',
    });
    if (response.status === 401) logout();
    let payload;
    try { payload = await response.json(); } catch { payload = {}; }
    if (!response.ok) {
      const error = payload.fehler || payload;
      throw new TransportError(error.code || `http_${response.status}`, error.was || 'R2 konnte die Anfrage nicht bearbeiten.', error.tun || 'Versuche es noch einmal.', response.status);
    }
    return payload;
  }
  function events(onMsg) {
    let stopped = signedOut, controller, timer, retryTimer, wakeRetry, lastId = 0, state = '';
    const notify = zustand => {
      if (state !== zustand) { state = zustand; onMsg({ typ: 'verbindung', zustand }); }
    };
    const handle = {
      close(reason) {
        stopped = true; clearTimeout(timer); clearTimeout(retryTimer); wakeRetry?.(); controller?.abort();
        if (reason) notify(reason);
        streams.delete(handle);
      },
      get state() { return state; }, get lastEventId() { return lastId; }, done: null,
    };
    streams.add(handle);
    function armPulse() {
      clearTimeout(timer);
      timer = setTimeout(() => { notify('unterbrochen'); controller?.abort(); }, pulseTimeoutMs);
    }
    async function run() {
      if (stopped) { notify('abgemeldet'); streams.delete(handle); return; }
      while (!stopped) {
        controller = new AbortController();
        armPulse();
        try {
          const response = await fetch(url('/v1/events'), {
            headers: { ...await headers(), Accept: 'text/event-stream', ...(lastId ? { 'Last-Event-ID': String(lastId) } : {}) },
            signal: controller.signal, credentials: 'omit', cache: 'no-store', redirect: 'error', referrerPolicy: 'no-referrer',
          });
          if (response.status === 401) { logout(); break; }
          if (!response.ok || !response.headers.get('content-type')?.startsWith('text/event-stream')) throw new Error('Kein Ereignisstrom.');
          const parse = createSSEParser(frame => {
            const msg = JSON.parse(frame.data);
            const type = msg.typ || msg.type || frame.event;
            if (type === 'puls') { armPulse(); notify('verbunden'); }
            if (frame.id !== undefined && frame.id !== '') {
              const next = Number(frame.id);
              if (!Number.isSafeInteger(next) || next < 1) throw new Error('Ungültige Ereignisnummer.');
              if (next <= lastId) return; // Replay-Dubletten nicht ausliefern.
              if (lastId && next !== lastId + 1) {
                onMsg({ typ: 'fehler', code: 'replay_luecke', was: 'Ein Teil des Verlaufs fehlt.', tun: 'Lade die Sessions erneut.' });
                handle.close('unterbrochen'); return; // Keine stillschweigend übersprungene Lücke.
              }
              if (stopped) return;
              onMsg(msg); lastId = next;
            } else if (!stopped) onMsg(msg);
          });
          const reader = response.body.getReader(), decoder = new TextDecoder();
          try {
            while (!stopped) {
              const { value, done } = await reader.read();
              if (done) break;
              parse(decoder.decode(value, { stream: true }));
            }
          } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
        } catch { /* Funkloch: gleicher Cursor, niemals Befehle wiederholen. */ }
        clearTimeout(timer);
        if (!stopped) {
          notify('unterbrochen');
          await new Promise(resolve => { wakeRetry = resolve; retryTimer = setTimeout(resolve, reconnectMs); });
          wakeRetry = null;
        }
      }
    }
    handle.done = run();
    return handle;
  }
  return {
    hallo: () => request('/v1/hallo', { method: 'POST', body: { protokoll: 1, app: 'cocopi-mobil 3.0.0' } }),
    sessions: () => request('/v1/sessions'), events,
    einloesen: code => request('/v1/einloesen', { method: 'POST', body: typeof code === 'object' ? code : parsePairingCode(code, 'e'), auth: false }),
    koppeln: code => request('/v1/koppeln', { method: 'POST', body: typeof code === 'object' ? code : parsePairingCode(code), auth: false }),
    async abmelden() { try { return await request('/v1/abmelden', { method: 'POST' }); } finally { logout(); } },
  };
}
