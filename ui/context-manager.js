// Kontextmanager für pi-lite: Suche/Ordnung der Sessionliste (reine Funktionen, mit Node
// testbar) und der Dialog „Kontext-Repair“. Gleiche Regeln wie copi/cocopi (app.rs:
// search_words/search_match/visible_sessions, repair_ui.rs); Backend: src-tauri/src/context.rs.
(function (root) {
  const BOOKMARK_LABELS = { off: "aus", on: "an", colors: "Farbe" };
  const BOOKMARK_TITLES = {
    off: "Bookmark aus: alle Sessions · Klick: nur Bookmarks",
    on: "Bookmark an: nur Bookmarks · Klick: nach Farbe gruppiert",
    colors: "Bookmark Farbe: nur Bookmarks, nach Farbe gruppiert · Klick: alle Sessions",
  };
  const ORIGIN_SHORT = { all: "alle", local: "lokal", remote: "remote" };

  /// Suchwörter: kleingeschrieben, höchstens acht (wie copi).
  function searchWords(q) {
    return String(q || "").toLowerCase().split(/\s+/).filter(Boolean).slice(0, 8);
  }

  /// UND-Verknüpfung: jedes Wort steht in Name/Titel/Ordner/Server oder hat Inhaltstreffer.
  /// Remote-Sessions nur über Metadaten (Inhaltssuche remote: GEPLANT, NICHT IMPLEMENTIERT).
  function searchMatch(s, words, hits) {
    if (!words.length) return true;
    const meta = [s.name, s.title, s.cwd, s.backend ? s.backendLabel : ""].join(" ").toLowerCase();
    const hit = s.backend ? null : hits?.[s.path];
    return words.every((w, i) => meta.includes(w) || !!(hit && hit.counts && hit.counts[i] > 0));
  }

  /// Reihenfolge: im Farbmodus erst nach Farbrang, dann Zeit (aktuell = neueste zuerst).
  function order(list, { inverse = false, colorRank = null } = {}) {
    const key = s => s.key || (s.backend ? `remote:${s.backend}:${s.path}` : s.path);
    return [...list].sort((a, b) =>
      (colorRank ? colorRank(a) - colorRank(b) : 0) ||
      (inverse ? a.modified - b.modified : b.modified - a.modified) ||
      key(a).localeCompare(key(b)));
  }

  /// Trefferzeile einer Karte: Anzahl je Wort plus Ausschnitt.
  function hitLine(hit, words) {
    if (!hit) return "";
    const n = words.map((w, i) => `${w} ${hit.counts?.[i] || 0}×`).join(" · ");
    return n + (hit.snippet ? " – " + hit.snippet.replace(/\s+/g, " ") : "");
  }

  // Wie copi `token_short`: eine Nachkommastelle, „.0“ entfällt (22000 → 22k, 22400 → 22.4k).
  const tokenShort = n => { const f = (v, s) => v.toFixed(1).replace(/\.0$/, "") + s; return n >= 1e6 ? f(n / 1e6, "M") : n >= 1e3 ? f(n / 1e3, "k") : String(Math.round(n)); };

  /// Dialog „Kontext-Repair“. env: {invoke, ask, toast, isOpen(s), isBusy(), afterWrite(s, applied), remove(s), importMd()}
  function createRepairDialog(box, env) {
    let st = null, seq = 0;
    const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; };
    const button = (text, fn, primary) => { const b = el("button", primary ? "primary" : "", text); b.onclick = fn; return b; };
    let returnFocus = null;
    function open(s) {
      if (s.backend) { env.toast("Kontext-Repair ist für Remote-Sessions noch nicht verfügbar (GEPLANT, NICHT IMPLEMENTIERT).", "info"); return; }
      if (box.hidden) returnFocus = document.activeElement;
      st = { s, loading: true, error: "", rep: null, on: [], title: "", setTitle: false, applied: null, working: false };
      box.hidden = false; analyze();
    }
    function close() { st = null; box.hidden = true; box.replaceChildren(); seq++; returnFocus?.focus?.(); }
    async function analyze() {
      const my = ++seq; st.loading = true; st.error = ""; render();
      try {
        const rep = await env.invoke("repair_analyze", { path: st.s.path });
        if (my !== seq || !st) return;
        st.rep = rep; st.on = rep.findings.map(f => f.default_on);
        st.setTitle = !rep.name && !!rep.suggested_title;
        st.title = rep.name || rep.suggested_title || "";
      } catch (e) { if (my === seq && st) st.error = String(e); }
      if (my === seq && st) { st.loading = false; render(); }
    }
    const saved = () => st.rep ? st.rep.findings.reduce((n, f, i) => n + (st.on[i] ? f.tokens_saved : 0), 0) : 0;
    async function apply() {
      const rep = st.rep; if (!rep) return;
      if (env.isOpen(st.s) && env.isBusy()) { env.toast("pi arbeitet noch in dieser Session – erst abbrechen oder warten.", "error"); return; }
      const edits = rep.findings.filter((_, i) => st.on[i]).map(f => [f.target, f.replacement]);
      const t = st.title.trim(), title = st.setTitle && t && t !== rep.name ? t : null;
      if (!edits.length && !title) { env.toast("Nichts ausgewählt.", "info"); return; }
      const s = st.s, n = saved();
      st.working = true; render();
      try {
        const a = await env.invoke("repair_apply", { path: rep.path, len: rep.len, leaf: rep.leaf, edits, title });
        env.toast(`Kontext-Repair angewandt: ${a.entries} Eintrag/Einträge angehängt, ≈ ${tokenShort(n)} Tokens weniger.`);
        if (st && st.s === s) st.applied = a;
        await env.afterWrite(s);
      } catch (e) { env.toast(String(e), "error"); }
      if (st && st.s === s) { st.working = false; analyze(); }
    }
    async function undo() {
      const a = st.applied, s = st.s; if (!a) return;
      if (env.isOpen(s) && env.isBusy()) { env.toast("pi arbeitet noch in dieser Session – erst abbrechen oder warten.", "error"); return; }
      try {
        await env.invoke("repair_undo", { path: s.path, oldLen: a.old_len, newLen: a.new_len, entries: a.entries });
        env.toast("Kontext-Repair rückgängig gemacht.");
        if (st && st.s === s) st.applied = null;
        await env.afterWrite(s);
      } catch (e) { env.toast(String(e), "error"); }
      if (st && st.s === s) analyze();
    }
    async function exportMd() {
      const rep = st.rep; if (!rep) return;
      const stem = rep.path.split("/").pop().replace(/\.jsonl$/, "");
      const dir = (rep.cwd || "~").replace(/\/$/, "");
      const r = await env.ask({ method: "input", title: "Session als Markdown exportieren", message: "Zielpfad der .md-Datei (wird nicht überschrieben, falls vorhanden):", prefill: `${dir}/${stem}.md` });
      if (!r?.value?.trim()) return;
      try { env.toast("Exportiert: " + await env.invoke("repair_export", { path: rep.path, target: r.value.trim() })); }
      catch (e) { env.toast(String(e), "error"); }
    }
    function render() {
      if (!st) return;
      const focusId = document.activeElement && box.contains(document.activeElement) ? document.activeElement.dataset.fid : null;
      box.replaceChildren();
      const card = el("div", "rp-card"); card.setAttribute("role", "dialog"); card.setAttribute("aria-label", "Kontext-Repair");
      const head = el("div", "rp-head");
      head.append(el("strong", "", "Kontext-Repair"), el("span", "rp-name", st.s.name || st.s.title || "(leer)"));
      const re = button("Neu analysieren", analyze); re.disabled = st.loading || st.working; re.dataset.fid = "re";
      const x = button("×", close); x.title = "Schließen (Esc)"; x.setAttribute("aria-label", "Schließen"); x.dataset.fid = "x";
      head.append(re, x); card.appendChild(head);
      card.appendChild(el("div", "rp-path", st.s.path.replace(/^\/home\/[^/]+/, "~")));
      const body = el("div", "rp-body"); card.appendChild(body);
      if (st.loading) body.appendChild(el("p", "rp-dim", "Session wird gelesen und analysiert (nur lesend) …"));
      else if (st.error) body.appendChild(el("p", "rp-err", st.error));
      else if (st.rep) {
        const rep = st.rep;
        body.appendChild(el("p", "", `Kontext ≈ ${tokenShort(rep.context_tokens)} Tokens · ${rep.messages} Nachrichten · ${rep.compactions} Kompaktierungen · ${rep.context_edits} frühere Bearbeitungen${rep.last_activity ? " · zuletzt " + rep.last_activity : ""}`));
        body.appendChild(el("h5", "", "Kontextfäule"));
        if (!rep.rot.length) body.appendChild(el("p", "rp-ok", "Keine Anzeichen."));
        for (const r of rep.rot) body.appendChild(el("p", r.level >= 2 ? "rp-err" : "rp-warn", (r.level >= 2 ? "Blockierend: " : "Warnung: ") + r.text));
        if (rep.delete_hint) {
          const p = el("div", "rp-hint", "Geringe Relevanz: " + rep.delete_hint);
          if (!env.isOpen(st.s)) { const s = st.s; const d = button("Löschen …", () => { close(); env.remove(s); }); p.append(" ", d); }
          body.appendChild(p);
        }
        const fh = el("h5", "", `Platzfresser (${rep.findings.length}) `);
        if (rep.findings.length) {
          const all = button("alle", () => { st.on = st.on.map(() => true); render(); }), none = button("keine", () => { st.on = st.on.map(() => false); render(); });
          all.className = none.className = "rp-small"; fh.append(all, none);
        }
        body.appendChild(fh);
        if (!rep.findings.length) body.appendChild(el("p", "rp-dim", "Nichts Kürzbares gefunden (die letzten Werkzeugergebnisse bleiben stets unangetastet)."));
        rep.findings.forEach((f, i) => {
          const row = el("label", "rp-find");
          const cb = el("input"); cb.type = "checkbox"; cb.checked = !!st.on[i]; cb.dataset.fid = "f" + i;
          cb.onchange = () => { st.on[i] = cb.checked; render(); };
          row.title = "Ersatz im Modellkontext:\n" + f.replacement.slice(0, 700);
          row.append(cb, el("span", "rp-ok", "−" + tokenShort(f.tokens_saved)), el("span", "rp-kind", f.kind), el("span", "rp-label", f.label));
          body.appendChild(row);
        });
        body.appendChild(el("h5", "", "Titel"));
        const tr = el("div", "rp-title");
        const cb = el("input"); cb.type = "checkbox"; cb.checked = st.setTitle; cb.dataset.fid = "tc";
        cb.title = rep.name ? "Vorhandener Name; ändern setzt einen neuen session_info-Eintrag" : "Noch kein Name gesetzt – Vorschlag aus Ziel bzw. erstem Prompt";
        cb.onchange = () => { st.setTitle = cb.checked; render(); };
        const inp = el("input"); inp.value = st.title; inp.maxLength = 120; inp.dataset.fid = "ti"; inp.setAttribute("aria-label", "Titel");
        inp.oninput = () => { st.title = inp.value; st.setTitle = !!inp.value.trim() && inp.value.trim() !== rep.name; cb.checked = st.setTitle; syncApply(); };
        const lab = el("label", "", " setzen"); lab.prepend(cb);
        tr.append(lab, inp); body.appendChild(tr);
        const list = (h, items, cls) => { if (!items?.length) return; body.appendChild(el("h5", "", h)); const ul = el("ul", cls); for (const t of items) ul.appendChild(el("li", "", t)); body.appendChild(ul); };
        list("Status", rep.status); list("ToDos", rep.todos);
        list("Letzte Prompts", rep.last_prompts.map(p => p.slice(0, 240)), "rp-dim");
        list("Geänderte Dateien", rep.modified_files, "rp-dim");
        list("Überschneidungen mit anderen Sessions", rep.overlaps.map(o => `${o.title} – ${o.shared.join(", ")}`));
      }
      const open = env.isOpen(st.s), busy = env.isBusy();
      const note = open && busy ? "pi arbeitet gerade in dieser Session – Anwenden erst danach." : open ? "Offene Session: wird nach dem Anwenden neu geladen." : "Nicht anwenden, solange die Session in einem anderen Fenster läuft.";
      card.appendChild(el("p", "rp-note", "Hängt nur pi-eigene context_edit-/session_info-Einträge an; Originale bleiben in der Datei. " + note));
      const foot = el("div", "rp-foot");
      const ap = button("", apply, true); ap.dataset.fid = "ap"; ap.id = "repairapply";
      foot.appendChild(ap);
      if (st.applied) { const u = button("Rückgängig", undo); u.title = "Schneidet genau den eben angehängten Repair wieder ab (nur solange die Session nicht weiterlief)"; u.disabled = st.working; foot.appendChild(u); }
      const sp = el("span", "rp-spacer"); foot.appendChild(sp);
      const ex = button(".md exportieren …", exportMd); ex.disabled = !st.rep; ex.title = "Bericht und aktiven Kontext (mit Kürzungen) als Markdown speichern";
      const im = button(".md importieren …", () => { close(); env.importMd(); }); im.title = "Markdown-Datei als neue Session anlegen";
      foot.append(ex, im); card.appendChild(foot);
      box.appendChild(card);
      syncApply();
      const f = focusId && box.querySelector(`[data-fid="${focusId}"]`);
      (f || box.querySelector("#repairapply:not(:disabled)") || x).focus();
    }
    function syncApply() {
      const ap = box.querySelector("#repairapply"); if (!ap || !st) return;
      const n = st.on.filter(Boolean).length;
      // Genau ein Minuszeichen vor der Tokenzahl.
      ap.textContent = `Anwenden (${n} · −${tokenShort(saved())})`;
      ap.disabled = !st.rep || st.working || !(n > 0 || st.setTitle) || (env.isOpen(st.s) && env.isBusy());
    }
    return { open, close, get isOpen() { return !box.hidden; }, get session() { return st?.s || null; } };
  }

  const api = { BOOKMARK_LABELS, BOOKMARK_TITLES, ORIGIN_SHORT, searchWords, searchMatch, order, hitLine, tokenShort, createRepairDialog };
  if (typeof module !== "undefined" && module.exports) module.exports = api; else root.ContextManager = api;
})(globalThis);
