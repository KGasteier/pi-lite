// pi-hub – Prototyp. Keine externen Abhängigkeiten (Node >= 20).
//
// Transport: HTTP + SSE (Server-Sent Events)
//   Hub -> Client : GET  /api/instances/:id/events   (SSE, Replay per Last-Event-ID / ?since=)
//                   GET  /api/events                 (SSE, Instanzliste + Benachrichtigungen)
//   Client -> Hub : POST /api/instances/:id/rpc      (ein JSON-Befehl, wird an den Harness weitergereicht)
//
// Der Hub reicht die Ereignisse des Harness unverändert durch ("raw") und ergänzt nur
// einen Rahmen { seq, ev } sowie einen abgeleiteten Zustand (idle/busy/waiting/exited).
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PORT = Number(process.env.HUB_PORT || 8787);
const HOST = process.env.HUB_HOST || '127.0.0.1';          // bewusst nur lokal; Zugriff von außen via Tailscale/SSH-Tunnel
const MAX_BUF = Number(process.env.HUB_BUFFER || 5000);    // Ereignisse pro Instanz im Speicher
const PI_BIN = process.env.HUB_PI_BIN || 'pi';
const NTFY_URL = process.env.HUB_NTFY_URL || '';           // z. B. https://ntfy.sh/mein-geheimes-thema
const ROOTS = (process.env.HUB_ROOTS || os.homedir()).split(path.delimiter).map(p => fs.realpathSync(p));
const STATE_FILE = process.env.HUB_STATE === 'off' ? null
  : (process.env.HUB_STATE || path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config'), 'pi-hub', 'instances.json'));
const SESSIONS_DIR = process.env.PI_SESSIONS_DIR || path.join(process.env.PI_CODING_AGENT_DIR || path.join(os.homedir(), '.pi', 'agent'), 'sessions');

// ---------- Token ----------
function loadToken() {
  if (process.env.HUB_TOKEN) return process.env.HUB_TOKEN;
  const dir = path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config'), 'pi-hub');
  const file = path.join(dir, 'token');
  try { return fs.readFileSync(file, 'utf8').trim(); } catch {}
  const t = crypto.randomBytes(24).toString('base64url');
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(file, t + '\n', { mode: 0o600 });
  return t;
}
const TOKEN = loadToken();
function tokenOk(given) {
  if (!given) return false;
  const a = Buffer.from(String(given)), b = Buffer.from(TOKEN);
  return a.length === b.length && crypto.timingSafeEqual(a, b);
}

// ---------- Instanzen ----------
const instances = new Map();
const globalClients = new Set();

// Registry: überlebt Hub-Neustarts (nur Metadaten, keine Verläufe). Atomar schreiben.
function persist() {
  if (!STATE_FILE) return;
  try {
    fs.mkdirSync(path.dirname(STATE_FILE), { recursive: true });
    const list = [...instances.values()].map(i => ({ id: i.id, name: i.name, harness: i.harness, cwd: i.cwd, session: i.session, created: i.created }));
    const tmp = STATE_FILE + '.new';
    fs.writeFileSync(tmp, JSON.stringify({ version: 1, instances: list }, null, 1) + '\n', { mode: 0o600 });
    fs.renameSync(tmp, STATE_FILE);
  } catch (e) { console.error('Registry nicht gespeichert:', e.message); }
}

function sse(res, id, data) { res.write((id != null ? `id: ${id}\n` : '') + `data: ${JSON.stringify(data)}\n\n`); }
function broadcastGlobal(msg) { for (const r of globalClients) sse(r, null, msg); }

async function notifyPush(title, body) {
  if (!NTFY_URL) return;
  try { await fetch(NTFY_URL, { method: 'POST', body, headers: { Title: encodeURIComponent(title).replace(/%20/g, ' ') } }); } catch {}
}

const ASK_METHODS = ['select', 'confirm', 'input', 'editor'];
const textOf = c => typeof c === 'string' ? c : Array.isArray(c) ? c.filter(b => b?.type === 'text').map(b => b.text).join('') : '';

class Instance {
  constructor({ id, name, harness, cwd, session, created }) {
    this.id = id || crypto.randomBytes(4).toString('hex');
    this.name = name || path.basename(cwd) || cwd;
    this.harness = harness; this.cwd = cwd; this.session = session || null;
    this.created = created || Date.now();
    this.seq = 0; this.buf = []; this.clients = new Set();
    this.state = 'starting'; this.asks = new Map(); this.last = '';
  }
  summary() {
    return { id: this.id, name: this.name, harness: this.harness, cwd: this.cwd, state: this.state,
             seq: this.seq, asks: this.asks.size, last: this.last, created: this.created, session: this.session };
  }
  setState(s) { if (this.state !== s) { this.state = s; broadcastGlobal({ type: 'inst', inst: this.summary() }); } }
  emit(ev) {
    const rec = { seq: ++this.seq, ev };
    this.buf.push(rec); if (this.buf.length > MAX_BUF) this.buf.shift();
    for (const r of this.clients) sse(r, rec.seq, rec);
    this.track(ev);
  }
  track(ev) {
    if (ev.type === 'response' && ev.success) {
      const f = ev.data?.sessionFile;
      if (ev.command === 'get_state' && f && f !== this.session) { this.session = f; persist(); broadcastGlobal({ type: 'inst', inst: this.summary() }); }
      if (['new_session', 'switch_session', 'fork', 'clone'].includes(ev.command) && this.harness === 'pi') this.write({ id: 'hub-state', type: 'get_state' });
    }
    switch (ev.type) {
      case 'agent_start': this.setState('busy'); break;
      case 'agent_settled':
        this.asks.clear(); this.setState('idle');
        this.notify(this.name + ': fertig', this.last.slice(0, 140) || 'Der Agent ist fertig.');
        break;
      case 'message_end':
        if (ev.message?.role === 'assistant') { const t = textOf(ev.message.content).trim(); if (t) this.last = t.slice(0, 200); }
        broadcastGlobal({ type: 'inst', inst: this.summary() });
        break;
      case 'extension_ui_request':
        if (ASK_METHODS.includes(ev.method)) {
          this.asks.set(ev.id, ev); this.setState('waiting');
          this.notify(this.name + ': Rückfrage', ev.title || 'Der Agent wartet auf deine Antwort.');
        }
        break;
      case 'hub_exit': this.asks.clear(); this.setState('exited'); this.notify(this.name + ': beendet', 'Der Prozess wurde beendet.'); break;
    }
  }
  notify(title, body) { broadcastGlobal({ type: 'notify', inst: this.id, title, body }); notifyPush(title, body); }
  send(cmd) {
    if (cmd.type === 'extension_ui_response' && this.asks.delete(cmd.id)) this.setState(this.asks.size ? 'waiting' : 'busy');
    this.write(cmd);
  }
  restart() {
    if (this.state !== 'exited') return false;
    this.asks.clear();
    this.emit({ type: 'hub_restart' });
    this.start();
    broadcastGlobal({ type: 'inst', inst: this.summary() });
    return true;
  }
  attach(res, since) {
    for (const rec of this.buf) {
      if (rec.seq <= since) continue;
      // Bereits beantwortete oder verfallene Rückfragen nicht erneut als Dialog anbieten.
      if (rec.ev.type === 'extension_ui_request' && ASK_METHODS.includes(rec.ev.method) && !this.asks.has(rec.ev.id)) continue;
      sse(res, rec.seq, rec);
    }
    this.clients.add(res);
    res.on('close', () => this.clients.delete(res));
  }
}

// --- Harness: pi (pi --mode rpc) ---
class PiInstance extends Instance {
  start() {
    const args = ['--mode', 'rpc'];
    if (this.session) args.push('--session', this.session);
    this.proc = spawn(PI_BIN, args, { cwd: this.cwd, stdio: ['pipe', 'pipe', 'pipe'] });
    let acc = '';
    this.proc.stdout.setEncoding('utf8');
    this.proc.stdout.on('data', chunk => {
      acc += chunk; let i;
      while ((i = acc.indexOf('\n')) >= 0) {           // nur an LF trennen (docs/rpc.md)
        const line = acc.slice(0, i).replace(/\r$/, ''); acc = acc.slice(i + 1);
        if (!line) continue;
        try { this.emit(JSON.parse(line)); } catch { this.emit({ type: 'hub_stdout', line }); }
      }
    });
    this.proc.stderr.setEncoding('utf8');
    this.proc.stderr.on('data', t => this.emit({ type: 'hub_stderr', text: t }));
    this.proc.on('error', e => { this.emit({ type: 'hub_stderr', text: `Start fehlgeschlagen: ${e.message}\n` }); this.emit({ type: 'hub_exit', code: -1 }); });
    this.proc.on('exit', code => this.emit({ type: 'hub_exit', code }));
    this.setState('idle');
    // Verlauf und Zustand für Clients, die später einsteigen:
    this.write({ id: 'hub-state', type: 'get_state' });
    if (this.session) this.write({ id: 'hub-hist', type: 'get_messages' });
  }
  write(cmd) { if (this.proc?.stdin.writable) this.proc.stdin.write(JSON.stringify(cmd) + '\n'); }
  stop() { try { this.proc?.kill('SIGTERM'); } catch {} }
}

// --- Harness: mock (Demo/Tests ohne Modellkosten) ---
class MockInstance extends Instance {
  start() { this.timers = new Set(); this.setState('idle'); this.emit({ type: 'response', id: 'hub-state', command: 'get_state', success: true, data: { model: { id: 'mock-1', provider: 'demo' }, sessionName: this.name, sessionFile: this.session || path.join(SESSIONS_DIR, 'mock', this.id + '.jsonl'), isStreaming: false } }); }
  later(ms, fn) { const t = setTimeout(() => { this.timers.delete(t); fn(); }, ms); this.timers.add(t); }
  write(cmd) {
    if (cmd.type === 'abort') { for (const t of this.timers) clearTimeout(t); this.timers.clear(); this.emit({ type: 'agent_end', messages: [], willRetry: false }); this.emit({ type: 'agent_settled', aborted: true }); return; }
    if (cmd.type === 'extension_ui_response') { this.later(200, () => this.reply(cmd.confirmed === false || cmd.cancelled ? 'Okay, abgelehnt – ich lasse die Dateien in Ruhe.' : 'Erledigt (nur simuliert).')); return; }
    if (cmd.type !== 'prompt') { if (cmd.id) this.emit({ type: 'response', id: cmd.id, command: cmd.type, success: true, data: {} }); return; }
    if (cmd.id) this.emit({ type: 'response', id: cmd.id, command: 'prompt', success: true, data: { disposition: 'started' } });
    const text = cmd.message || '';
    this.emit({ type: 'agent_start' });
    this.emit({ type: 'message_start', message: { role: 'user', content: [{ type: 'text', text }] } });
    this.emit({ type: 'message_end', message: { role: 'user', content: [{ type: 'text', text }] } });
    if (/frage|lösch|delete/i.test(text)) {
      this.later(300, () => this.emit({ type: 'extension_ui_request', id: crypto.randomUUID(), method: 'confirm', title: 'Dateien löschen?', message: 'Der Agent möchte `build/` entfernen.' }));
      return;
    }
    this.later(200, () => this.reply(`Du hast gesagt: **${text}**\n\nHier ein Beispiel:\n\n\`\`\`js\nconsole.log("Hallo vom Mock-Harness");\n\`\`\`\n\n- Punkt eins\n- Punkt zwei`, true));
  }
  reply(full, withTool) {
    this.emit({ type: 'message_start', message: { role: 'assistant', content: [] } });
    let d = 0;
    if (withTool) {
      this.later(d += 100, () => this.emit({ type: 'message_update', assistantMessageEvent: { type: 'thinking_delta', delta: 'Ich überlege kurz, was zu tun ist …' } }));
      this.later(d += 200, () => this.emit({ type: 'tool_execution_start', toolCallId: 't1', toolName: 'bash', args: { command: 'ls -la' } }));
      this.later(d += 400, () => this.emit({ type: 'tool_execution_end', toolCallId: 't1', toolName: 'bash', result: { content: [{ type: 'text', text: 'total 0\ndrwxr-xr-x demo' }] }, isError: false }));
    }
    for (const w of full.match(/\S+\s*/g) || []) this.later(d += 40, () => this.emit({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: w } }));
    this.later(d += 100, () => {
      this.emit({ type: 'message_end', message: { role: 'assistant', content: [{ type: 'text', text: full }] } });
      this.emit({ type: 'agent_end', messages: [], willRetry: false });
      this.emit({ type: 'agent_settled', aborted: false });
    });
  }
  stop() { for (const t of this.timers || []) clearTimeout(t); this.emit({ type: 'hub_exit', code: 0 }); }
}

const HARNESSES = { pi: PiInstance, mock: MockInstance };

// ---------- Dateien: Sandbox ----------
function inRoots(p) {
  let real; try { real = fs.realpathSync(p); } catch { return null; }
  return ROOTS.some(r => real === r || real.startsWith(r + path.sep)) ? real : null;
}
function listDirs(p) {
  const real = inRoots(p || ROOTS[0]); if (!real) throw Object.assign(new Error('Pfad außerhalb der erlaubten Wurzeln'), { status: 403 });
  const dirs = fs.readdirSync(real, { withFileTypes: true }).filter(e => e.isDirectory() && !e.name.startsWith('.')).map(e => e.name).sort((a, b) => a.localeCompare(b));
  const parent = ROOTS.includes(real) ? null : path.dirname(real);
  return { path: real, parent: parent && inRoots(parent) ? parent : null, dirs };
}
function listSessions() {
  const out = [];
  let projects = []; try { projects = fs.readdirSync(SESSIONS_DIR); } catch {}
  for (const p of projects) {
    let files = []; try { files = fs.readdirSync(path.join(SESSIONS_DIR, p)).filter(f => f.endsWith('.jsonl')); } catch { continue; }
    for (const f of files) { const full = path.join(SESSIONS_DIR, p, f); try { out.push({ path: full, modified: fs.statSync(full).mtimeMs }); } catch {} }
  }
  out.sort((a, b) => b.modified - a.modified);
  return out.slice(0, 40).map(s => {
    let cwd = '', title = '';
    try {
      const fd = fs.openSync(s.path, 'r'); const b = Buffer.alloc(65536); const n = fs.readSync(fd, b, 0, b.length, 0); fs.closeSync(fd);
      for (const line of b.toString('utf8', 0, n).split('\n')) {
        let j; try { j = JSON.parse(line); } catch { continue; }
        if (j.type === 'session') cwd = j.cwd || '';
        if (j.type === 'session_info' && j.name) title = j.name;
        if (!title && j.type === 'message' && j.message?.role === 'user') title = textOf(j.message.content).replace(/\s+/g, ' ').trim().slice(0, 80);
        if (cwd && title) break;
      }
    } catch {}
    return { ...s, cwd, title: title || path.basename(s.path) };
  }).filter(s => s.cwd && inRoots(s.cwd));
}

// ---------- HTTP ----------
const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.json': 'application/json', '.webmanifest': 'application/manifest+json', '.svg': 'image/svg+xml', '.png': 'image/png' };
function json(res, code, obj) { res.writeHead(code, { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' }); res.end(JSON.stringify(obj)); }
function body(req) { return new Promise((ok, no) => { let s = ''; req.on('data', c => { s += c; if (s.length > 20e6) { no(new Error('zu groß')); req.destroy(); } }); req.on('end', () => { try { ok(s ? JSON.parse(s) : {}); } catch (e) { no(e); } }); }); }
function sseHead(res) { res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-cache, no-transform', connection: 'keep-alive', 'x-accel-buffering': 'no' }); res.write('retry: 2000\n\n'); }

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, 'http://x');
  try {
    if (url.pathname.startsWith('/api/')) {
      const bearer = (req.headers.authorization || '').replace(/^Bearer /, '');
      if (!tokenOk(bearer || url.searchParams.get('token'))) return json(res, 401, { error: 'unauthorized' });
      const p = url.pathname.slice(5).split('/').filter(Boolean);
      if (p[0] === 'ping') return json(res, 200, { ok: true, harnesses: Object.keys(HARNESSES), roots: ROOTS, push: !!NTFY_URL });
      if (p[0] === 'events') {
        sseHead(res); globalClients.add(res);
        sse(res, null, { type: 'list', instances: [...instances.values()].map(i => i.summary()) });
        res.on('close', () => globalClients.delete(res)); return;
      }
      if (p[0] === 'dirs') return json(res, 200, listDirs(url.searchParams.get('path')));
      if (p[0] === 'sessions' && req.method === 'GET') return json(res, 200, listSessions());
      if (p[0] === 'sessions' && req.method === 'DELETE') {
        const f = path.resolve(url.searchParams.get('path') || '');
        if (!f.endsWith('.jsonl') || !f.startsWith(path.resolve(SESSIONS_DIR) + path.sep)) return json(res, 403, { error: 'Session außerhalb' });
        if ([...instances.values()].some(i => i.session === f && i.state !== 'exited')) return json(res, 409, { error: 'Session ist in einer laufenden Instanz offen' });
        try { fs.unlinkSync(f); } catch { return json(res, 404, { error: 'unbekannte Session' }); }
        return json(res, 200, { ok: true });
      }
      if (p[0] === 'instances') {
        if (p.length === 1 && req.method === 'GET') return json(res, 200, [...instances.values()].map(i => i.summary()));
        if (p.length === 1 && req.method === 'POST') {
          const b = await body(req);
          const H = HARNESSES[b.harness || 'pi']; if (!H) return json(res, 400, { error: 'unbekannter Harness' });
          const cwd = inRoots(b.cwd || ROOTS[0]); if (!cwd) return json(res, 403, { error: 'cwd außerhalb der erlaubten Wurzeln' });
          let session = null;
          if (b.session) { session = path.resolve(b.session); if (!session.startsWith(path.resolve(SESSIONS_DIR) + path.sep)) return json(res, 403, { error: 'Session außerhalb' }); }
          const inst = new H({ name: b.name, harness: b.harness || 'pi', cwd, session });
          instances.set(inst.id, inst); inst.start(); persist();
          broadcastGlobal({ type: 'inst', inst: inst.summary() });
          return json(res, 201, inst.summary());
        }
        const inst = instances.get(p[1]); if (!inst) return json(res, 404, { error: 'unbekannte Instanz' });
        if (p.length === 2 && req.method === 'DELETE') { inst.stop(); instances.delete(inst.id); persist(); broadcastGlobal({ type: 'gone', id: inst.id }); return json(res, 200, { ok: true }); }
        if (p[2] === 'restart' && req.method === 'POST') return inst.restart() ? json(res, 200, inst.summary()) : json(res, 409, { error: 'Instanz läuft noch' });
        if (p[2] === 'events') {
          const since = Number(req.headers['last-event-id'] || url.searchParams.get('since') || 0);
          sseHead(res); inst.attach(res, since); return;
        }
        if (p[2] === 'rpc' && req.method === 'POST') { const cmd = await body(req); if (!cmd || typeof cmd.type !== 'string') return json(res, 400, { error: 'type fehlt' }); inst.send(cmd); return json(res, 202, { ok: true }); }
      }
      return json(res, 404, { error: 'nicht gefunden' });
    }
    // statische Dateien (ohne Auth: enthalten keine Daten)
    let rel = decodeURIComponent(url.pathname); if (rel === '/') rel = '/index.html';
    const file = path.join(HERE, 'client', path.normalize(rel).replace(/^(\.\.[/\\])+/, ''));
    if (!file.startsWith(path.join(HERE, 'client'))) { res.writeHead(403); return res.end(); }
    fs.readFile(file, (e, data) => {
      if (e) { res.writeHead(404); return res.end('nicht gefunden'); }
      res.writeHead(200, { 'content-type': MIME[path.extname(file)] || 'application/octet-stream', 'cache-control': 'no-cache' }); res.end(data);
    });
  } catch (e) { json(res, e.status || 500, { error: e.message }); }
});

setInterval(() => { for (const r of globalClients) r.write(': hb\n\n'); for (const i of instances.values()) for (const r of i.clients) r.write(': hb\n\n'); }, 20000).unref();
// Beim Beenden bleibt die Registry unverändert, damit der nächste Start die Instanzen wieder aufnimmt.
const shutdown = () => { for (const i of instances.values()) i.stop(); process.exit(0); };
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

function restore() {
  if (!STATE_FILE) return;
  let saved; try { saved = JSON.parse(fs.readFileSync(STATE_FILE, 'utf8')); } catch { return; }
  for (const e of saved.instances || []) {
    const H = HARNESSES[e.harness]; const cwd = inRoots(e.cwd || '');
    if (!H || !cwd || instances.has(e.id)) continue;
    const inst = new H({ id: e.id, name: e.name, harness: e.harness, cwd, session: e.session, created: e.created });
    instances.set(inst.id, inst); inst.start();
  }
  console.log(`${instances.size} Instanz(en) aus der Registry wieder aufgenommen.`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  restore();
  server.listen(PORT, HOST, () => {
    console.log(`pi-hub läuft auf http://${HOST}:${PORT}`);
    console.log(`Token: ${TOKEN}`);
    console.log(`Anmelde-Link: http://${HOST}:${PORT}/#token=${TOKEN}`);
    if (HOST !== '127.0.0.1' && HOST !== 'localhost') console.log('WARNUNG: Hub ist nicht nur lokal gebunden. Nur hinter Tailscale/WireGuard/TLS betreiben!');
  });
}
export { server };
