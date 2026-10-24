// Multi-Backend-Rasteransicht (lokal + pi-hub-Server) für pi-lite.
// Oberer Teil: reine Funktionen (Ereignis → Zustand), per Node testbar (scripts/test-net.cjs).
// Unterer Teil: DOM/Tauri, läuft nur im Fenster mit window.__TAURI__ und nutzt renderMd/toast aus app.js.
// Ereignisform aus src-tauri/src/net.rs: {backend, inst, src:"watch"|"sub", tag?, kind, ev|info|up|text}.
(function (root) {
  "use strict";

  const STATE_LABELS = { starting: "startet", idle: "bereit", busy: "arbeitet", waiting: "Rückfrage", exited: "beendet" };
  const ASK_METHODS = ["select", "confirm", "input", "editor"];
  const MAX_TILES = 4;

  function stateLabel(s) { return STATE_LABELS[s] || "unbekannt"; }

  const textOf = (c) => typeof c === "string" ? c :
    (Array.isArray(c) ? c.filter(p => p && p.type === "text").map(p => p.text).join("") : "");

  // ---------- Leiste: Backends und Instanzen ----------
  function createNetModel() { return { backends: [], insts: {} }; }

  // Liste aus net_backends übernehmen; bekannte Instanzen bleiben erhalten.
  function applyBackends(m, list) {
    m.backends = (list || []).map(b => ({ ...b }));
    for (const b of m.backends) m.insts[b.id] ??= {};
    for (const id of Object.keys(m.insts)) if (!m.backends.some(b => b.id === id)) delete m.insts[id];
    return m;
  }

  function applyList(m, backend, infos) {
    const map = {};
    for (const i of infos || []) map[i.id] = { ...i };
    m.insts[backend] = map;
    return m;
  }

  // Beobachter-Ereignis (Info/Gone/Link) einarbeiten; true, wenn sich die Leiste ändert.
  function applyWatch(m, e) {
    if (!e || e.src !== "watch") return false;
    const b = m.backends.find(x => x.id === e.backend);
    if (e.kind === "link") {
      if (!b || b.link === e.up) return false;
      b.link = e.up; return true;
    }
    const map = (m.insts[e.backend] ??= {});
    if (e.kind === "info" && e.info) {
      const old = map[e.inst];
      if (old && JSON.stringify(old) === JSON.stringify(e.info)) return false;
      map[e.inst] = { ...e.info }; return true;
    }
    if (e.kind === "gone") {
      if (!(e.inst in map)) return false;
      delete map[e.inst]; return true;
    }
    return false;
  }

  function instList(m, backend) {
    return Object.values(m.insts[backend] || {}).sort((a, b) => a.id.localeCompare(b.id, "de", { numeric: true }));
  }

  function linkLabel(b) {
    if (b.error) return { cls: "err", text: "Fehler: " + b.error };
    if (!b.enabled) return { cls: "off", text: "abgeschaltet" };
    if (b.link === true) return { cls: "up", text: "verbunden" };
    if (b.link === false) return { cls: "down", text: "getrennt – Wiederaufbau läuft" };
    return { cls: "wait", text: "verbindet …" };
  }

  // Auswahl fürs Raster: an/aus; mehr als MAX_TILES verdrängt die älteste Kachel.
  function toggleTile(keys, key, max = MAX_TILES) {
    if (keys.includes(key)) return keys.filter(k => k !== key);
    const next = [...keys, key];
    return next.length > max ? next.slice(next.length - max) : next;
  }
  const tileKey = (backend, inst) => backend + "\u0000" + inst;
  const splitKey = (k) => { const i = k.indexOf("\u0000"); return [k.slice(0, i), k.slice(i + 1)]; };

  // CSS-Raster für 1–4 Kacheln (bei 3: erste Kachel links über zwei Zeilen).
  function gridLayout(n) {
    if (n <= 1) return { columns: "1fr", rows: "1fr", areas: null };
    if (n === 2) return { columns: "1fr 1fr", rows: "1fr", areas: null };
    if (n === 3) return { columns: "1fr 1fr", rows: "1fr 1fr", areas: '"t0 t1" "t0 t2"' };
    return { columns: "1fr 1fr", rows: "1fr 1fr", areas: null };
  }

  // ---------- Kachel: Verlauf einer Instanz ----------
  // items: {kind:"user"|"assistant"|"thinking"|"tool"|"note", text, ...}; dirty = neu zeichnen.
  function createTile(backend, inst, tag) {
    return { backend, inst, tag, items: [], tools: {}, cur: null, think: null, asks: [], live: false, busy: false, exited: false };
  }

  function push(t, item) { item.dirty = true; t.items.push(item); if (t.items.length > 2000) t.items.splice(0, t.items.length - 2000); return item; }
  function touch(item) { if (item) item.dirty = true; }

  // Ein Ereignis des Instanz-Abos verarbeiten; true, wenn neu gezeichnet werden muss.
  function tileEvent(t, e) {
    if (!e || e.src !== "sub" || e.backend !== t.backend || e.inst !== t.inst || e.tag !== t.tag) return false;
    switch (e.kind) {
      case "live": t.live = true; return true;
      case "exit": t.exited = true; t.busy = false; t.cur = t.think = null; t.asks = []; push(t, { kind: "note", text: "Agent beendet." }); return true;
      case "gone": push(t, { kind: "note", text: "Instanz wurde entfernt.", err: true }); return true;
      case "line": return e.ev ? piEvent(t, e.ev) : false;
      default: return false;
    }
  }

  function piEvent(t, ev) {
    switch (ev.type) {
      case "agent_start": t.busy = true; t.exited = false; return true;
      case "agent_settled": t.busy = false; t.asks = []; t.cur = t.think = null; return true;
      case "message_start":
        if (ev.message?.role === "user") { push(t, { kind: "user", text: textOf(ev.message.content) }); return true; }
        t.cur = t.think = null; return false;
      case "message_update": {
        const a = ev.assistantMessageEvent || {};
        if (a.type === "text_delta") { t.cur ??= push(t, { kind: "assistant", text: "" }); t.cur.text += a.delta || ""; touch(t.cur); return true; }
        if (a.type === "thinking_delta") { t.think ??= push(t, { kind: "thinking", text: "" }); t.think.text += a.delta || ""; touch(t.think); return true; }
        return false;
      }
      case "message_end": {
        if (ev.message?.role !== "assistant") return false;
        const full = textOf(ev.message.content);
        // Ohne Deltas (z. B. gekürzter Verlauf) die fertige Nachricht übernehmen.
        if (t.cur) { if (full && full.length >= t.cur.text.length) t.cur.text = full; t.cur.done = true; touch(t.cur); }
        else if (full.trim()) push(t, { kind: "assistant", text: full, done: true });
        t.cur = t.think = null; return true;
      }
      case "tool_execution_start":
        if (t.cur) { t.cur.done = true; touch(t.cur); }
        t.cur = null;
        t.tools[ev.toolCallId] = push(t, { kind: "tool", name: ev.toolName || "?", args: ev.args ?? {}, text: "", err: false });
        return true;
      case "tool_execution_update": case "tool_execution_end": {
        const it = t.tools[ev.toolCallId]; if (!it) return false;
        const r = ev.result || ev.partialResult;
        it.text = textOf(r?.content); if (ev.isError) it.err = true;
        if (ev.type === "tool_execution_end") it.done = true;
        touch(it); return true;
      }
      case "extension_ui_request":
        if (ASK_METHODS.includes(ev.method) && ev.id && !t.asks.some(a => a.id === ev.id)) { t.asks.push(ev); return true; }
        return false;
      case "auto_retry_start": push(t, { kind: "note", text: `Wiederholung ${ev.attempt}/${ev.maxAttempts}: ${ev.errorMessage || ""}`, err: true }); return true;
      case "compaction_start": push(t, { kind: "note", text: "Kontext wird komprimiert …" }); return true;
      case "response":
        if (ev.success === false) { push(t, { kind: "note", text: `${ev.command || "Befehl"}: ${ev.error || "Fehler"}`, err: true }); return true; }
        return false;
      default: return false;
    }
  }

  // Antwort auf eine Rückfrage: Befehl bauen und die Frage aus der Kachel nehmen.
  function answerAsk(t, id, resp) {
    t.asks = t.asks.filter(a => a.id !== id);
    return { type: "extension_ui_response", id, ...resp };
  }

  function promptCmd(text, busy) {
    return { type: "prompt", message: text, ...(busy ? { streamingBehavior: "steer" } : {}) };
  }

  // Formular „Backend hinzufügen“ prüfen; Fehlertext oder null.
  function validateBackendForm(f, isLocal) {
    if (!String(f.label || "").trim()) return "Name fehlt";
    if (isLocal) return null;
    if (!/^https?:\/\/\S+$/.test(String(f.url || "").trim())) return "URL muss mit http:// oder https:// beginnen";
    if (f.token_env && !/^[A-Za-z_][A-Za-z0-9_]*$/.test(f.token_env.trim())) return "Umgebungsvariable: nur Buchstaben, Ziffern und _";
    return null;
  }

  const api = { STATE_LABELS, MAX_TILES, stateLabel, createNetModel, applyBackends, applyList, applyWatch, instList, linkLabel,
    toggleTile, tileKey, splitKey, gridLayout, createTile, tileEvent, answerAsk, promptCmd, validateBackendForm };
  if (typeof module !== "undefined" && module.exports) { module.exports = api; return; }
  root.PiNet = api;
  if (!root.__TAURI__ || typeof document === "undefined") return;

  // =====================================================================
  // DOM-Teil (nur im Fenster)
  // =====================================================================
  const { invoke: call } = root.__TAURI__.core;
  const { listen: on } = root.__TAURI__.event;
  const el = (tag, cls, text) => { const d = document.createElement(tag); if (cls) d.className = cls; if (text != null) d.textContent = text; return d; };
  const q = (id) => document.getElementById(id);
  const say = (msg, type = "info") => (typeof toast === "function" ? toast(msg, type) : console.warn(msg));
  const md = (node, text) => { if (typeof renderMd === "function") renderMd(node, text, true); else node.textContent = text; };

  const model = createNetModel();
  let selected = [];             // Kachelschlüssel in Rasterreihenfolge
  const tiles = new Map();       // Schlüssel → { t, dom }
  let tagSeq = 0, barTimer = 0, started = false;

  function svg(path, vb = "0 0 16 16") { return `<svg viewBox="${vb}" aria-hidden="true"><path d="${path}"/></svg>`; }
  const ICON = { plus: svg("M8 3v10M3 8h10"), gear: svg("M8 5.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zM8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4"),
    stop: '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="4.5" y="4.5" width="7" height="7" rx="1"/></svg>',
    restart: svg("M3 8a5 5 0 1 0 1.5-3.6M3 2.5v2.5h2.5"), close: svg("M4 4l8 8M12 4l-8 8"), trash: svg("M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.7 8.5h5.6l.7-8.5") };

  // ---------- Grundgerüst ----------
  const view = q("netview"), bar = q("netbar"), grid = q("netgrid");

  function setMode(on, persist = true) {
    document.body.classList.toggle("netmode", on);
    view.hidden = !on;
    q("gridtoggle").setAttribute("aria-pressed", String(on));
    q("gridtoggle").title = (on ? "Zurück zum Einzel-Chat" : "Rasteransicht: mehrere Backends und Instanzen") + " · Strg/Cmd+Shift+G";
    if (persist) { try { localStorage.setItem("pi-lite-net-grid", on ? "1" : "0"); } catch {} }
    if (on && !started) { started = true; refreshAll(); }
    if (on) renderGrid();
  }
  q("gridtoggle").onclick = () => setMode(view.hidden);
  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && !e.altKey && e.key.toLowerCase() === "g") { e.preventDefault(); setMode(view.hidden); }
  });

  async function refreshAll() {
    try { applyBackends(model, await call("net_backends")); } catch (e) { say("Backends: " + e, "error"); return; }
    renderBar();
    await Promise.all(model.backends.filter(b => b.connected).map(async b => {
      try { applyList(model, b.id, await call("net_list", { backend: b.id })); b.listError = null; }
      catch (e) { b.listError = String(e); }
    }));
    // Kacheln, deren Instanz es nicht mehr gibt, verschwinden.
    selected = selected.filter(k => { const [b, i] = splitKey(k); return model.insts[b]?.[i]; });
    renderBar(); renderGrid();
  }

  on("net-ev", (msg) => {
    const e = msg.payload;
    if (e.src === "watch") {
      if (applyWatch(model, e)) {
        scheduleBar();
        const tile = tiles.get(tileKey(e.backend, e.inst));
        if (tile) renderTileHead(tile);
        if (e.kind === "link") for (const x of tiles.values()) if (x.t.backend === e.backend) renderTileHead(x);
      }
      return;
    }
    const tile = tiles.get(tileKey(e.backend, e.inst));
    if (tile && tileEvent(tile.t, e)) scheduleTile(tile);
  });

  function scheduleBar() { if (!barTimer) barTimer = setTimeout(() => { barTimer = 0; renderBar(); }, 60); }

  // ---------- Leiste ----------
  function renderBar() {
    bar.innerHTML = "";
    const head = el("div", "netbarhead");
    head.append(el("strong", null, "Backends"));
    const add = el("button", "netaddbackend", "＋ Backend"); add.title = "Hub-Server hinzufügen"; add.onclick = () => backendDialog(null);
    head.append(add); bar.append(head);
    for (const b of model.backends) {
      const sec = el("section", "netbackend"); sec.dataset.backend = b.id;
      const h = el("div", "netbhead");
      const ll = linkLabel(b);
      const dot = el("span", "netdot " + ll.cls); dot.title = ll.text; dot.setAttribute("aria-label", ll.text);
      const name = el("span", "netbname", b.label);
      const kind = el("span", "netkind", b.remote ? "remote" : "lokal");
      const cfg = el("button", "neticon"); cfg.innerHTML = ICON.gear; cfg.title = "Backend bearbeiten"; cfg.setAttribute("aria-label", cfg.title); cfg.onclick = () => backendDialog(b);
      const plus = el("button", "neticon netplus"); plus.innerHTML = ICON.plus; plus.title = "Neue Instanz auf " + b.label; plus.setAttribute("aria-label", plus.title);
      plus.disabled = !b.connected; plus.onclick = () => instanceDialog(b);
      h.append(dot, name, kind, cfg, plus); sec.append(h);
      if (b.link === false || b.error || b.listError) sec.append(el("div", "netwarn", b.error || b.listError || "Verbindung getrennt – Wiederaufbau läuft automatisch."));
      const list = instList(model, b.id);
      if (b.connected && !list.length) sec.append(el("div", "netempty", "Keine Instanzen"));
      for (const i of list) sec.append(instRow(b, i));
      bar.append(sec);
    }
  }

  function pill(state) { const p = el("span", "netpill " + (state || "idle"), stateLabel(state)); return p; }

  function instRow(b, i) {
    const k = tileKey(b.id, i.id), r = el("button", "netinst"); r.type = "button";
    r.setAttribute("aria-pressed", String(selected.includes(k)));
    r.title = (selected.includes(k) ? "Aus dem Raster nehmen" : "Im Raster zeigen") + (i.cwd ? " · " + i.cwd : "");
    const top = el("span", "netinsttop");
    top.append(el("span", "netiname", i.name || i.id), pill(i.state));
    if (i.asks > 0) { const badge = el("span", "netbadge", String(i.asks)); badge.title = i.asks + " offene Rückfrage(n)"; top.append(badge); }
    r.append(top, el("span", "netlast", i.last || (i.harness && i.harness !== "pi" ? i.harness + " · " : "") + (i.cwd || "")));
    r.onclick = () => { selected = toggleTile(selected, k); renderBar(); renderGrid(); };
    return r;
  }

  // ---------- Raster ----------
  function renderGrid() {
    // Abos nur für sichtbare Kacheln halten.
    for (const [k, x] of tiles) if (!selected.includes(k)) { call("net_unsubscribe", { backend: x.t.backend, inst: x.t.inst }).catch(() => {}); tiles.delete(k); }
    grid.innerHTML = "";
    const lay = gridLayout(selected.length);
    grid.style.gridTemplateColumns = lay.columns; grid.style.gridTemplateRows = lay.rows; grid.style.gridTemplateAreas = lay.areas || "";
    if (!selected.length) {
      const hint = el("div", "netgridempty");
      hint.append(el("strong", null, "Rasteransicht"), el("p", null, "Links Instanzen anklicken (bis zu vier). Mit ＋ eine neue Instanz starten, mit ⚙ ein Backend bearbeiten."));
      grid.append(hint); return;
    }
    selected.forEach((k, n) => {
      let x = tiles.get(k);
      if (!x) { const [b, i] = splitKey(k); x = { t: createTile(b, i, String(++tagSeq)), dom: null, timer: 0 }; tiles.set(k, x); buildTile(x); subscribe(x); }
      if (lay.areas) x.dom.root.style.gridArea = "t" + n; else x.dom.root.style.gridArea = "";
      grid.append(x.dom.root);
      renderTileHead(x);
    });
  }

  async function subscribe(x) {
    try { await call("net_subscribe", { backend: x.t.backend, inst: x.t.inst, tag: x.t.tag }); }
    catch (e) { tileNote(x, "Abo fehlgeschlagen: " + e); }
  }
  function tileNote(x, text) { push(x.t, { kind: "note", text, err: true }); scheduleTile(x); }

  function buildTile(x) {
    const root = el("section", "nettile"), head = el("div", "nettilehead"), log = el("div", "nettilelog"), asks = el("div", "netasks"), foot = el("div", "nettilefoot");
    log.tabIndex = 0; log.setAttribute("role", "region");
    const input = el("textarea", "nettileinput"); input.placeholder = "Nachricht (Enter = senden, Alt+Enter = Zeilenumbruch)"; input.rows = 2;
    const sendBtn = el("button", "primary", "Senden"), abortBtn = el("button", null, "Abbruch");
    const submit = async () => {
      const text = input.value.trim(); if (!text) return;
      const info = infoOf(x.t);
      if (info?.state === "exited") { say("Instanz ist beendet – erst neu starten.", "warning"); return; }
      input.value = "";
      try { await call("net_send", { backend: x.t.backend, inst: x.t.inst, cmd: promptCmd(text, x.t.busy || info?.state === "busy") }); }
      catch (e) { if (!input.value) input.value = text; say(String(e), "error"); }
    };
    input.onkeydown = (e) => { if (e.key === "Enter" && !e.altKey && !e.shiftKey && !e.isComposing) { e.preventDefault(); submit(); } else if (e.key === "Escape") { e.preventDefault(); abort(x); } };
    sendBtn.onclick = submit; abortBtn.onclick = () => abort(x);
    const btns = el("div", "nettilebtns"); btns.append(sendBtn, abortBtn);
    foot.append(input, btns);
    root.append(head, log, asks, foot);
    x.dom = { root, head, log, asks, input };
  }

  function infoOf(t) { return model.insts[t.backend]?.[t.inst]; }
  function backendOf(t) { return model.backends.find(b => b.id === t.backend); }
  function abort(x) { call("net_send", { backend: x.t.backend, inst: x.t.inst, cmd: { type: "abort" } }).catch(e => say(String(e), "error")); }

  function renderTileHead(x) {
    const { head } = x.dom, t = x.t, info = infoOf(t) || {}, b = backendOf(t) || {};
    head.innerHTML = "";
    const ll = linkLabel(b);
    const dot = el("span", "netdot " + ll.cls); dot.title = ll.text;
    const title = el("span", "nettiletitle", info.name || t.inst); title.title = (info.cwd || "") + (info.harness ? " · " + info.harness : "");
    head.append(dot, title, el("span", "netkind", b.label || t.backend), pill(info.state));
    if (info.asks > 0) head.append(el("span", "netbadge", String(info.asks)));
    if (!t.live) head.append(el("span", "netloading", "lädt Verlauf …"));
    const tool = (icon, label, fn, cls) => { const btn = el("button", "neticon" + (cls ? " " + cls : "")); btn.innerHTML = icon; btn.title = label; btn.setAttribute("aria-label", label); btn.onclick = fn; return btn; };
    const tools = el("span", "nettiletools");
    if (info.state === "exited") tools.append(tool(ICON.restart, "Neu starten", () => call("net_restart", { backend: t.backend, inst: t.inst }).then(() => { tiles.delete(tileKey(t.backend, t.inst)); renderGrid(); }).catch(e => say(String(e), "error"))));
    else tools.append(tool(ICON.stop, "Abbrechen (Esc)", () => abort(x)));
    tools.append(tool(ICON.trash, "Instanz löschen …", () => removeInst(x), "danger"), tool(ICON.close, "Aus dem Raster nehmen", () => { selected = selected.filter(k => k !== tileKey(t.backend, t.inst)); renderBar(); renderGrid(); }));
    head.append(tools);
    x.dom.root.dataset.state = info.state || "";
  }

  async function removeInst(x) {
    const info = infoOf(x.t) || {};
    const ok = typeof ask === "function" ? await ask({ method: "confirm", title: "Instanz löschen?", message: `„${info.name || x.t.inst}“ auf ${backendOf(x.t)?.label || x.t.backend} wird beendet und aus der Liste entfernt. Die Session-Datei bleibt erhalten.` }) : { confirmed: confirm("Instanz löschen?") };
    if (!ok?.confirmed) return;
    try { await call("net_remove", { backend: x.t.backend, inst: x.t.inst }); } catch (e) { say(String(e), "error"); }
  }

  function scheduleTile(x) {
    if (x.timer) return;
    x.timer = setTimeout(() => { x.timer = 0; renderTile(x); }, 80);
  }

  function renderTile(x) {
    if (!x.dom) return;
    const { log } = x.dom, t = x.t;
    const atBottom = log.scrollHeight - log.scrollTop - log.clientHeight < 80;
    // Gekürzten Anfang entfernen.
    while (log.firstChild && !t.items.includes(log.firstChild._item)) log.firstChild.remove();
    for (const it of t.items) {
      if (!it.el) { it.el = itemNode(it); it.el._item = it; log.append(it.el); }
      if (it.dirty) { fillItem(it); it.dirty = false; }
    }
    if (atBottom) log.scrollTop = log.scrollHeight;
    renderAsks(x); renderTileHead(x);
  }

  function itemNode(it) {
    if (it.kind === "tool") { const d = el("details", "msg tool"); d.append(el("summary"), el("pre")); return d; }
    if (it.kind === "assistant") { const d = el("div", "msg assistant"); d.append(el("div", "md")); return d; }
    return el("div", it.kind === "note" ? "note" : "msg " + it.kind);
  }

  function fillItem(it) {
    const n = it.el;
    if (it.kind === "assistant") md(n.firstChild, it.text);
    else if (it.kind === "tool") {
      n.querySelector("summary").textContent = `🔧 ${it.name} ${JSON.stringify(it.args).slice(0, 160)}`;
      n.querySelector("pre").textContent = it.text; n.classList.toggle("err", !!it.err);
    } else { n.textContent = it.text; if (it.kind === "note") n.classList.toggle("err", !!it.err); }
  }

  // Rückfragen inline unter dem Verlauf.
  function renderAsks(x) {
    const box = x.dom.asks, t = x.t;
    if (box._sig === t.asks.map(a => a.id).join(",")) return;
    box._sig = t.asks.map(a => a.id).join(",");
    box.innerHTML = "";
    for (const a of t.asks) {
      const card = el("div", "netask"), row = el("div", "netaskrow");
      card.append(el("strong", null, a.title || "Rückfrage"));
      if (a.message) card.append(el("div", "netaskmsg", a.message));
      const reply = (resp) => {
        const cmd = answerAsk(t, a.id, resp); box._sig = null; renderAsks(x);
        call("net_send", { backend: t.backend, inst: t.inst, cmd }).catch(e => { say(String(e), "error"); t.asks.push(a); box._sig = null; renderAsks(x); });
      };
      const btn = (label, fn, cls) => { const b = el("button", cls, label); b.onclick = fn; return b; };
      if (a.method === "confirm") row.append(btn("Nein", () => reply({ confirmed: false })), btn("Ja", () => reply({ confirmed: true }), "primary"));
      else if (a.method === "select") { for (const o of a.options || []) row.append(btn(String(o), () => reply({ value: o }))); row.append(btn("Abbrechen", () => reply({ cancelled: true }))); }
      else {
        const f = el(a.method === "editor" ? "textarea" : "input"); f.value = a.prefill ?? ""; if (a.placeholder) f.placeholder = a.placeholder;
        card.append(f);
        f.onkeydown = (e) => { if (e.key === "Enter" && (a.method !== "editor" || e.ctrlKey || e.metaKey)) { e.preventDefault(); reply({ value: f.value }); } };
        row.append(btn("Abbrechen", () => reply({ cancelled: true })), btn("OK", () => reply({ value: f.value }), "primary"));
      }
      card.append(row); box.append(card);
    }
  }

  // ---------- Dialoge ----------
  function dialog(title) {
    const back = el("div", "netdlgback"), d = el("div", "netdlg"); d.setAttribute("role", "dialog"); d.setAttribute("aria-label", title);
    d.append(el("h4", null, title)); back.append(d); document.body.append(back);
    const close = () => back.remove();
    back.onkeydown = (e) => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); } };
    back.onclick = (e) => { if (e.target === back) close(); };
    return { d, close };
  }
  function field(d, label, input) { const l = el("label", "netfield"); l.append(el("span", null, label), input); d.append(l); return input; }
  function inputEl(type, value, ph) { const i = el("input"); i.type = type; i.value = value || ""; if (ph) i.placeholder = ph; i.autocomplete = "off"; i.spellcheck = false; return i; }

  function backendDialog(b) {
    const local = b?.kind === "local";
    const { d, close } = dialog(b ? "Backend bearbeiten" : "Backend hinzufügen");
    const label = field(d, "Name", inputEl("text", b?.label, "z. B. Server Büro"));
    let url, token, env;
    if (!local) {
      url = field(d, "URL", inputEl("url", b?.url, "http://127.0.0.1:8787"));
      token = field(d, "Token", inputEl("password", "", b?.token_set ? "gespeichert – leer lassen = unverändert" : "Token des Hubs"));
      env = field(d, "oder Umgebungsvariable", inputEl("text", b?.token_env, "z. B. PI_HUB_TOKEN"));
      d.append(el("p", "nethint", "Das Token wird nur in backends.json (Modus 0600) gespeichert und nie angezeigt. Ist eine Umgebungsvariable gesetzt, hat sie Vorrang."));
    }
    const en = inputEl("checkbox"); en.checked = b ? b.enabled !== false : true; field(d, "Verbinden", en);
    const err = el("div", "netformerr"); d.append(err);
    const row = el("div", "netdlgrow");
    if (b && !local) { const del = el("button", "danger", "Entfernen"); del.onclick = async () => { try { await call("net_delete_backend", { backend: b.id }); close(); refreshAll(); } catch (e) { err.textContent = String(e); } }; row.append(del); }
    const cancel = el("button", null, "Abbrechen"), save = el("button", "primary", "Speichern");
    cancel.onclick = close;
    save.onclick = async () => {
      const form = { id: b?.id ?? null, label: label.value.trim(), url: url?.value.trim() ?? "", token: token?.value ?? "", token_env: env?.value.trim() ?? "", enabled: en.checked };
      const bad = validateBackendForm(form, local); if (bad) { err.textContent = bad; return; }
      save.disabled = true; err.textContent = "";
      try { await call("net_save_backend", { form }); close(); refreshAll(); }
      catch (e) { err.textContent = String(e); save.disabled = false; }
    };
    row.append(cancel, save); d.append(row);
    d.addEventListener("keydown", (e) => { if (e.key === "Enter" && e.target.tagName === "INPUT") { e.preventDefault(); save.click(); } });
    label.focus();
  }

  function instanceDialog(b) {
    const { d, close } = dialog("Neue Instanz · " + b.label);
    const name = field(d, "Name (optional)", inputEl("text", "", "z. B. Doku"));
    const harness = field(d, "Harness", el("select"));
    for (const h of b.remote ? ["pi", "mock"] : ["pi"]) harness.append(new Option(h === "mock" ? "mock (Test, ohne Modell)" : h, h));
    const cwd = field(d, "Arbeitsordner", inputEl("text", "", b.remote ? "Ordner auf dem Server" : "Ordner auf diesem Rechner"));
    const browse = el("div", "netdirs"); d.append(browse);
    const load = async (path) => {
      browse.textContent = "lädt …";
      try {
        const r = await call("net_dirs", { backend: b.id, path: path || null });
        cwd.value = r.path; browse.innerHTML = "";
        if (r.parent) { const up = el("button", null, "↑ Übergeordnet"); up.onclick = () => load(r.parent); browse.append(up); }
        for (const n of r.dirs) { const x = el("button", null, n); x.onclick = () => load(r.path.replace(/\/$/, "") + "/" + n); browse.append(x); }
        if (!r.dirs.length) browse.append(el("span", "nethint", "Keine Unterordner"));
      } catch (e) { browse.textContent = String(e); }
    };
    const err = el("div", "netformerr"); d.append(err);
    const row = el("div", "netdlgrow"), cancel = el("button", null, "Abbrechen"), go = el("button", "primary", "Starten");
    cancel.onclick = close;
    go.onclick = async () => {
      go.disabled = true; err.textContent = "";
      try {
        const info = await call("net_create", { backend: b.id, spec: { name: name.value, harness: harness.value, cwd: cwd.value } });
        (model.insts[b.id] ??= {})[info.id] = info;
        const k = tileKey(b.id, info.id); if (!selected.includes(k)) selected = toggleTile(selected, k);
        close(); renderBar(); renderGrid();
      } catch (e) { err.textContent = String(e); go.disabled = false; }
    };
    row.append(cancel, go); d.append(row);
    load(null); name.focus();
  }

  try { setMode(localStorage.getItem("pi-lite-net-grid") === "1", false); } catch { setMode(false, false); }
})(globalThis);
