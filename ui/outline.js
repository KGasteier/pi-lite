// Seitenansicht „Gliederung“ (gleiche Datei und Logik wie copi/cocopi):
// virtuelle, sortier- und einrückbare Sicht auf Sessions und md-Dateien mit benennbaren,
// auf-/zuklappbaren Kategorien. Ablage `<config>/pi-frontends/outline.json`; alle Änderungen
// laufen als Operation über den Tauri-Befehl `outline_apply` (Rust `shared/outline.rs`),
// damit beide Frontends exakt gleich sortieren/einrücken. Dateien und Sessions selbst
// werden nie verschoben, umbenannt oder gelöscht.
// Reine Funktionen per Node testbar (scripts/test-outline.cjs).
(function (root) {
  "use strict";
  const INDENT = 14, LOOSE = [["sessions", "Sessions ohne Einordnung"], ["md", "md-Dateien ohne Einordnung"]];

  const same = (a, b) => a.t === b.t && (a.t === "cat" ? a.id === b.id : a.t === "session" ? a.key === b.key : a.path === b.path);
  const refOf = it => it.t === "cat" ? { t: "cat", id: it.id, name: it.name } : it.t === "session" ? { t: "session", key: it.key } : { t: "md", path: it.path };
  function subtreeEnd(items, i) {
    const l = items[i].level; let j = i + 1;
    while (j < items.length && items[j].level > l) j++;
    return j;
  }
  function visible(items) {
    const out = []; let i = 0;
    while (i < items.length) { out.push(i); i = items[i].collapsed ? subtreeEnd(items, i) : i + 1; }
    return out;
  }
  // AUGMOST-Klemme: höchstens eine Ebene tiefer als oben, mindestens die Ebene darunter.
  function dropLevel(above, below, want) {
    const max = above == null ? 0 : Math.min(12, above + 1), min = below == null ? 0 : Math.min(below, max);
    return Math.max(min, Math.min(max, want));
  }
  const isMd = p => /\.md$/i.test(p || "");
  const baseName = p => String(p).split(/[\\/]/).pop();
  const sessionLabel = s => s.name || s.title || "(leer)";

  /// Zeilen wie copi `outline_rows`: sichtbarer Baum, danach die Gruppen ohne Einordnung.
  /// `sessions`: [{key, name, title, modified, cwd, backend, backendLabel}], `md`: inventory-Einträge.
  function rows(outline, sessions, md) {
    const items = outline.items || [], out = [];
    const sKey = s => s.key || (s.backend ? `remote:${s.backend}:${s.path}` : s.path);
    const sByKey = new Map(sessions.map(s => [sKey(s), s])), mByPath = new Map(md.map(e => [e.path, e]));
    for (const i of visible(items)) {
      const it = items[i], end = subtreeEnd(items, i);
      let label, tip = "", missing = false, remote = false, data = null;
      if (it.t === "cat") label = it.name;
      else if (it.t === "session") {
        data = sByKey.get(it.key) || null; missing = !data; remote = it.key.startsWith("remote:");
        label = data ? sessionLabel(data) : baseName(it.key);
        tip = data ? "Session" : "Session nicht (mehr) gefunden: " + it.key;
      } else {
        data = mByPath.get(it.path) || null; missing = !data;
        label = data ? data.name : baseName(it.path);
        tip = data ? it.path : "Datei nicht im Inventar: " + it.path;
      }
      out.push({ kind: "tree", item: i, ref: refOf(it), level: it.level, label, tip, missing, remote, data, hasChildren: end > i + 1, collapsed: !!it.collapsed, count: end - i - 1 });
    }
    const inTree = r => items.some(it => same(it, r));
    const looseS = sessions.filter(s => !inTree({ t: "session", key: sKey(s) }));
    const looseM = md.filter(e => !inTree({ t: "md", path: e.path }));
    const open = outline.open_unsorted || [];
    for (const [group, label] of LOOSE) {
      const list = group === "sessions" ? looseS : looseM, isOpen = open.includes(group);
      out.push({ kind: "group", group, level: 0, label, hasChildren: list.length > 0, collapsed: !isOpen, count: list.length });
      if (!isOpen) continue;
      for (const x of list) {
        const ref = group === "sessions" ? { t: "session", key: sKey(x) } : { t: "md", path: x.path };
        out.push({ kind: "loose", ref, level: 1, label: group === "sessions" ? sessionLabel(x) : x.name, tip: group === "md" ? x.path : "Session", missing: false, remote: !!x.backend, data: x, hasChildren: false, collapsed: false, count: 0 });
      }
    }
    return out;
  }
  const rowId = r => r.kind === "group" ? "g:" + r.group : (r.kind === "loose" ? "l:" : "t:") + JSON.stringify(r.ref.t === "cat" ? { t: "cat", id: r.ref.id } : r.ref);

  /// Ansicht in `box`. `env`: {invoke, ask, toast, openSession(s), openMd(entry), previewMd(row, entry)}.
  function createView(box, env) {
    let outline = { version: 1, rev: 0, items: [], open_unsorted: [] }, list = [], cursor = null, sessions = [], md = [], drag = null;
    let view = false;
    const inventory = () => sessions.map(s=>({t:'session',key:s.key || (s.backend?`remote:${s.backend}:${s.path}`:s.path)})).concat(md.map(e=>({t:'md',path:e.path})));
    const read = () => env.invoke('outline_read',{inventory:inventory()});
    async function load() {
      try { outline = await read(); } catch (e) { env.toast(String(e), "error"); }
      render();
    }
    async function op(o, rev = outline.rev) {
      try {
        const r = await env.invoke("outline_apply", { rev, op: o });
        outline = r.outline;
        await env.bookmarksChanged?.();
        if (r.at != null && outline.items[r.at]) cursor = rowId({ kind: "tree", ref: refOf(outline.items[r.at]) });
      } catch (e) {
        env.toast(String(e), "error");
        try { outline = await read(); } catch {}
      }
      render(true);
    }
    const name = async (title, prefill) => {
      const r = await env.ask({ method: "input", title, message: "Kategorien gibt es nur in der Gliederung; Dateien und Sessions bleiben, wo sie sind.", prefill: prefill || "" });
      return r && r.value ? r.value.trim() : "";
    };
    async function newCat(after, child) {
      const rev = outline.rev;
      const n = await name(child ? "Neue Unterkategorie" : "Neue Kategorie");
      if (n) op({ op: "add_cat", after: after ?? null, child: !!child, name: n }, rev);
    }
    async function rename(r) {
      if (r.kind === "tree" && r.ref.t === "cat") {
        if(r.ref.id.startsWith('bookmark-color:')) {
          const c=r.ref.id.split(':')[1]; if(c!=='none') await env.renameColor?.(Number(c));
          await load(); return;
        }
        const rev = outline.rev;
        const n = await name("Kategorie umbenennen", r.ref.name);
        if (n) op({ op: "rename", i: r.item, name: n }, rev);
      } else if (r.ref?.t === "session" && r.data) env.renameSession?.(r.data);
      else env.toast("md-Dateien werden in der Gliederung nicht umbenannt (nur Kategorien und Sessions).", "info");
    }
    async function remove(r) {
      if (r.kind !== "tree" || r.ref?.id?.startsWith('bookmark-color:')) return;
      const rev = outline.rev;
      if (r.ref.t === "cat") {
        const msg = `„${r.label}“` + (r.count ? `\n\nDie ${r.count} Einträge darin rücken eine Ebene auf; nichts wird gelöscht.` : "");
        const a = await env.ask({ method: "confirm", title: "Kategorie entfernen?", message: msg });
        if (!a || !a.confirmed) return;
      }
      op({ op: "remove", i: r.item }, rev);
    }
    function open(r) {
      if (r.kind === "group") return op({ op: "toggle_unsorted", group: r.group });
      if (r.ref.t === "cat") return r.hasChildren && op({ op: "toggle", i: r.item });
      if (r.missing) return env.toast(r.tip, "info");
      if (r.ref.t === "session") env.openSession(r.data); else env.openMd(r.data);
    }
    const adopt = r => op({ op: "insert", r: r.ref, dest: outline.items.length, level: 0 });
    const cur = () => Math.max(0, list.findIndex(r => rowId(r) === cursor));
    function focusRow(i) {
      const el = box.querySelectorAll(".ol-row")[Math.max(0, Math.min(list.length - 1, i))];
      if (el) { cursor = el.dataset.id; el.focus(); }
    }
    function key(e, r) {
      const i = list.indexOf(r), k = e.key;
      const parentOf = idx => { const l = outline.items[idx].level; for (let j = idx - 1; j >= 0; j--) if (outline.items[j].level < l) return j; return null; };
      let used = true;
      if(view && (k==='Tab' || e.altKey && ['ArrowUp','ArrowDown'].includes(k))) { e.preventDefault(); return; }
      if (k === "Tab") { if (r.kind === "tree") op({ op: e.shiftKey ? "outdent" : "indent", i: r.item }); else if (r.kind === "loose" && !e.shiftKey) adopt(r); }
      else if (e.altKey && k === "ArrowUp") { if (r.kind === "tree") op({ op: "up", i: r.item }); }
      else if (e.altKey && k === "ArrowDown") { if (r.kind === "tree") op({ op: "down", i: r.item }); }
      else if (e.altKey || e.ctrlKey || e.metaKey) used = false;
      else if (k === "ArrowDown") focusRow(i + 1);
      else if (k === "ArrowUp") focusRow(i - 1);
      else if (k === "Home") focusRow(0);
      else if (k === "End") focusRow(list.length - 1);
      else if (k === "ArrowLeft") {
        if (r.kind === "tree" && r.hasChildren && !r.collapsed) op({ op: "set_collapsed", i: r.item, collapsed: true });
        else if (r.kind === "tree") { const p = parentOf(r.item); if (p != null) focusRow(list.findIndex(x => x.item === p)); }
        else if (r.kind === "group" && !r.collapsed) op({ op: "toggle_unsorted", group: r.group });
        else if (r.kind === "loose") { for (let j = i; j >= 0; j--) if (list[j].kind === "group") { focusRow(j); break; } }
      } else if (k === "ArrowRight") {
        if (r.collapsed && r.hasChildren) r.kind === "group" ? op({ op: "toggle_unsorted", group: r.group }) : op({ op: "set_collapsed", i: r.item, collapsed: false });
        else if (r.hasChildren) focusRow(i + 1);
      } else if (k === "Enter" || k === " ") open(r);
      else if (k === "F2" || k === "r") rename(r);
      else if (k === "Delete" || k === "Backspace") remove(r);
      else if (k === "n") newCat(r.kind === "tree" ? r.item : null, false);
      else if (k === "N") newCat(r.kind === "tree" && r.ref.t === "cat" ? r.item : null, true);
      else used = false;
      if (used) { e.preventDefault(); e.stopPropagation(); }
    }
    function menu(e, r) {
      e.preventDefault();
      const items = [];
      if (r.kind === "tree") {
        outline.items.forEach((it,i)=>{if(it.t==='cat' && i!==r.item) items.push(['→ '+it.name,()=>op({op:'into_cat',r:r.ref,cat:i})]);});
        if (r.ref.t !== "cat") items.push(["Öffnen", () => open(r)]);
        items.push(["Neue Kategorie danach", () => newCat(r.item, false)]);
        if (r.ref.t === "cat") items.push(["Neue Unterkategorie", () => newCat(r.item, true)], ["Umbenennen …", () => rename(r)]);
        items.push(null, ["Einrücken (Tab)", () => op({ op: "indent", i: r.item })], ["Ausrücken (Umschalt+Tab)", () => op({ op: "outdent", i: r.item })],
          ["Nach oben (Alt+↑)", () => op({ op: "up", i: r.item })], ["Nach unten (Alt+↓)", () => op({ op: "down", i: r.item })], null,
          [r.ref.t === "cat" ? "Kategorie entfernen …" : "Aus Gliederung nehmen", () => remove(r)]);
      } else if (r.kind === "loose") {
        items.push(["Öffnen", () => open(r)], ["In Gliederung übernehmen (Tab)", () => adopt(r)]);
        outline.items.forEach((it, i) => { if (it.t === "cat") items.push(["→ " + "  ".repeat(it.level) + it.name, () => op({ op: "into_cat", r: r.ref, cat: i })]); });
      } else return;
      const m = document.createElement("div"); m.className = "ol-menu"; m.setAttribute("role", "menu");
      const close = () => { m.remove(); document.removeEventListener("pointerdown", outside, true); };
      const outside = ev => { if (!m.contains(ev.target)) close(); };
      for (const x of items) {
        if (!x) { m.appendChild(document.createElement("hr")); continue; }
        const b = document.createElement("button"); b.textContent = x[0]; b.setAttribute("role", "menuitem");
        b.onclick = () => { close(); x[1](); };
        m.appendChild(b);
      }
      m.onkeydown = ev => { if (ev.key === "Escape") { close(); focusRow(list.indexOf(r)); } };
      document.body.appendChild(m);
      m.style.left = Math.min(e.clientX, innerWidth - m.offsetWidth - 4) + "px";
      m.style.top = Math.min(e.clientY, innerHeight - m.offsetHeight - 4) + "px";
      document.addEventListener("pointerdown", outside, true);
      m.querySelector("button")?.focus();
    }
    // Ziehen: Einfügelinie zwischen Baumzeilen, Ebene aus waagrechter Position (AUGMOST).
    function dropTarget(ev) {
      const tree = [...box.querySelectorAll(".ol-row[data-item]")], left = box.getBoundingClientRect().left + 8;
      const want = Math.max(0, Math.round((ev.clientX - drag.grabDx - left) / INDENT));
      if (!tree.length) return { dest: 0, level: 0, el: null, after: false };
      let slot = tree.findIndex(el => { const b = el.getBoundingClientRect(); return ev.clientY < b.top + b.height / 2; });
      if (slot < 0) slot = tree.length;
      const lv = el => list.find(r => rowId(r) === el.dataset.id).level;
      const level = dropLevel(slot > 0 ? lv(tree[slot - 1]) : null, slot < tree.length ? lv(tree[slot]) : null, want);
      const dest = slot < tree.length ? +tree[slot].dataset.item : outline.items.length;
      return { dest, level, el: tree[Math.min(slot, tree.length - 1)], after: slot >= tree.length };
    }
    function showLine(t) {
      let ln = box.querySelector(".ol-drop");
      if (!t) { ln?.remove(); return; }
      if (!ln) { ln = document.createElement("div"); ln.className = "ol-drop"; box.appendChild(ln); }
      const b = t.el ? t.el.getBoundingClientRect() : box.getBoundingClientRect(), bb = box.getBoundingClientRect();
      ln.style.top = ((t.el ? (t.after ? b.bottom : b.top) : bb.top + 4) - bb.top + box.scrollTop) + "px";
      ln.style.left = (8 + t.level * INDENT + 6) + "px";
    }
    function startDrag(ev, r, el) {
      if (ev.button !== 0 || r.kind === "group") return;
      const x0 = ev.clientX, y0 = ev.clientY, rx = el.getBoundingClientRect().left + 3 + r.level * INDENT;
      let moved = false;
      const move = e => {
        if (!moved && Math.hypot(e.clientX - x0, e.clientY - y0) < 5) return;
        if (!moved) { moved = true; drag = { r, grabDx: x0 - rx }; el.classList.add("dragging"); document.body.classList.add("ol-grabbing"); }
        drag.target = dropTarget(e); showLine(drag.target);
      };
      const end = e => {
        removeEventListener("pointermove", move); removeEventListener("pointerup", end); removeEventListener("keydown", esc, true);
        document.body.classList.remove("ol-grabbing"); el.classList.remove("dragging"); showLine(null);
        const d = drag; drag = null;
        if (!moved || !d || !d.target || e === null) return;
        suppressClick = true; setTimeout(() => suppressClick = false, 0);
        if (r.kind === "tree") op({ op: "move_to", src: r.item, dest: d.target.dest, level: d.target.level });
        else op({ op: "insert", r: r.ref, dest: d.target.dest, level: d.target.level });
      };
      const esc = e => { if (e.key === "Escape" && moved) { e.preventDefault(); e.stopPropagation(); drag = null; end(null); } };
      addEventListener("pointermove", move); addEventListener("pointerup", end); addEventListener("keydown", esc, true);
    }
    let suppressClick = false;
    const tri = open => `<svg viewBox="0 0 12 12" aria-hidden="true"><path d="${open ? "M2.5 4h7L6 8.5z" : "M4 2.5L8.5 6 4 9.5z"}" fill="currentColor"/></svg>`;
    const SYMS = { empty: `<svg viewBox="0 0 12 12" aria-hidden="true"><path d="M4 2.5L8.5 6 4 9.5z" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round"/></svg>`,
      md: `<svg viewBox="0 0 12 12" aria-hidden="true"><rect x="3.5" y="3" width="5" height="6" rx="1" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>`,
      session: `<svg viewBox="0 0 12 12" aria-hidden="true"><circle cx="6" cy="6" r="2.4" fill="currentColor"/></svg>` };
    const viewIndent = level => Math.min(5, level) * 9;
    function render(keepFocus) {
      const hadFocus = keepFocus || box.contains(document.activeElement);
      list = rows(outline, sessions, md);
      box.replaceChildren();
      const head = document.createElement("div"); head.className = "ol-head";
      const add = document.createElement("button"); add.textContent = "+ Farbe"; add.onclick = () => env.newColor?.();
      const edit=document.createElement('button'), show=document.createElement('button');
      edit.textContent='Edit'; show.textContent='View';
      edit.setAttribute('aria-pressed',String(!view)); show.setAttribute('aria-pressed',String(view));
      edit.onclick=()=>{view=false;render();}; show.onclick=()=>{view=true;render();};
      const help = document.createElement("span"); help.className = "ol-help"; help.textContent = "?";
      help.title = "Gliederung: nur eine Sicht – Dateien und Sessions werden nie verschoben.\nZiehen: einsortieren (waagrecht = Ebene) · Esc bricht ab\n↑/↓ wählen · ←/→ zu-/aufklappen · Tab/Umschalt+Tab ein-/ausrücken\nAlt+↑/↓ verschieben · Enter öffnen · F2/R umbenennen\nN neue Kategorie · Umschalt+N Unterkategorie · Entf aus Gliederung nehmen";
      head.append(add, edit, show, help); box.appendChild(head);
      if (!outline.items.length) { const p = document.createElement("p"); p.className = "ol-empty"; p.textContent = "Noch keine Gliederung. „+ Kategorie“ anlegen und Einträge aus den Gruppen unten hineinziehen (oder Tab)."; box.appendChild(p); }
      let sep = false;
      if (!list.some(r => rowId(r) === cursor)) cursor = list[0] ? rowId(list[0]) : null;
      for (const r of list) {
        if (!sep && r.kind !== "tree") { sep = true; box.appendChild(document.createElement("hr")); }
        if(view && r.ref?.t==='session' && r.data && env.sessionCard) {
          const wrap=document.createElement('div');
          wrap.style.marginLeft=viewIndent(r.level)+'px';
          const card=env.sessionCard(r.data);
          if(r.hasChildren) {
            const b=document.createElement('button'); b.innerHTML=tri(!r.collapsed);
            b.title=r.collapsed?'Untereinträge aufklappen':'Untereinträge zuklappen';
            b.setAttribute('aria-label',b.title); b.setAttribute('aria-expanded',String(!r.collapsed));
            b.style.cssText='position:absolute;left:4px;top:4px;width:22px;height:22px;padding:4px;background:transparent';
            b.onpointerdown=e=>e.stopPropagation(); b.onkeydown=e=>e.stopPropagation();
            b.onclick=e=>{e.stopPropagation();op({op:'toggle',i:r.item});};
            const title=card.querySelector('.t'); if(title) title.style.paddingLeft='16px';
            card.appendChild(b);
          }
          wrap.appendChild(card); box.appendChild(wrap); continue;
        }
        const el = document.createElement("div"), id = rowId(r);
        el.className = "ol-row " + r.kind + (r.ref?.t === "cat" ? " cat" : "") + (r.missing ? " missing" : "") + (r.remote ? " remote" : "");
        el.dataset.id = id; if (r.kind === "tree") el.dataset.item = r.item;
        el.tabIndex = id === cursor ? 0 : -1; el.title = r.tip || "";
        el.setAttribute("role", "treeitem"); el.setAttribute("aria-level", String(r.level + 1));
        if (r.hasChildren) el.setAttribute("aria-expanded", String(!r.collapsed));
        el.style.paddingLeft = (view ? 3 : 3 + r.level * INDENT) + "px";
        if(view) el.style.marginLeft=viewIndent(r.level)+'px';
        const color=env.colorOf?.(r.ref || {}), tone=color == null ? 'var(--dim,#89939d)' : BookmarkPalette.hex(color);
        const colorRoot=r.ref?.t==='cat' && r.ref.id.startsWith('bookmark-color:');
        if(colorRoot) { el.style.borderLeft=`3px solid ${tone}`; el.style.background=`color-mix(in srgb, ${tone} 18%, transparent)`; }
        el.style.setProperty('--ol-color',tone);
        const sym = document.createElement("span"); sym.className = "ol-sym"; sym.style.color=tone;
        const isCat = r.kind === "group" || r.ref.t === "cat";
        sym.innerHTML = r.hasChildren ? tri(!r.collapsed) : isCat ? SYMS.empty : r.ref.t === "md" ? SYMS.md : SYMS.session;
        const t = document.createElement("span"); t.className = "ol-t"; t.textContent = r.label;
        el.append(sym, t);
        if (r.count && (r.collapsed || r.kind === "group")) { const c = document.createElement("span"); c.className = "ol-n"; c.textContent = r.count; el.appendChild(c); }
        el.onfocus = () => { cursor = id; for (const o of box.querySelectorAll(".ol-row")) o.tabIndex = o === el ? 0 : -1; };
        el.onkeydown = e => key(e, r);
        el.oncontextmenu = e => { cursor = id; menu(e, r); };
        el.onpointerdown = e => { if(!view && !colorRoot) startDrag(e, r, el); };
        el.onclick = e => {
          if (suppressClick) return;
          cursor = id; el.focus();
          const onSym = e.clientX < sym.getBoundingClientRect().right + 3;
          if (isCat) { open(r); }
          else if (r.ref.t === "md" && !r.missing && env.previewMd) env.previewMd(el, r.data);
          else open(r);
        };
        el.ondblclick = e => {
          // Symbolklicks klappen bereits einzeln um; nur Doppelklick auf den Titel behandeln.
          if (isCat && (r.kind === "group" || e.clientX < sym.getBoundingClientRect().right + 3)) return;
          if(isCat) return;
          if (r.ref.t === "md") open(r);
        };
        box.appendChild(el);
      }
      if (hadFocus) box.querySelector(`.ol-row[tabindex="0"]`)?.focus();
    }
    return {
      load,
      setData(s, m) { sessions = s; md = m.filter(e => isMd(e.path)); render(); },
      refresh: async () => { if (drag) return; try { const o = await read(); if (o.rev !== outline.rev) { outline = o; render(); } } catch {} },
      focus: () => focusRow(cur()),
    };
  }

  const api = { INDENT, same, subtreeEnd, visible, dropLevel, rows, rowId, createView };
  if (typeof module !== "undefined" && module.exports) module.exports = api; else root.Outline = api;
})(globalThis);
