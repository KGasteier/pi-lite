// Laufaktivität für die Statuszeile über der Eingabe (Gegenstück: copi/app/src/run_activity.rs).
//
// Leitet aus dem pi-Ereignisstrom ab, *was* gerade passiert (Phase), und zählt Runden,
// Werkzeugaufrufe, Fehler und die Warteschlange. Reine Anzeige: nichts davon wird
// gespeichert oder gesendet; die Prozentschätzung bleibt in duration-estimate.js.
function runClock(seconds) {
  const s = Math.max(0, Math.floor(seconds || 0));
  return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
}

// Kurzdetail eines Werkzeugaufrufs: Pfad, Befehl oder Suchmuster (einzeilig, gekürzt).
function runToolDetail(args) {
  const a = args && typeof args === "object" ? args : {};
  let raw = ["path", "command", "pattern", "url", "query"].map(k => a[k]).find(v => typeof v === "string");
  if (raw === undefined) raw = Object.values(a).find(v => typeof v === "string") || "";
  const line = (raw.split(/\r?\n/)[0] || "").trim();
  const chars = [...line];
  let out = chars.slice(0, 120).join("");
  if (chars.length > 120 || raw.trim().split(/\r?\n/).length > 1) out += "…";
  return out;
}

// Subagenten-Werkzeug? (Extension `subagent`; tolerant für ähnlich benannte Varianten.)
function runIsSubagentTool(name) { return String(name || "").toLowerCase().includes("subagent"); }
// Geplante Subagenten eines Aufrufs: tasks (parallel), chain (Kette) oder einer.
function runSubagentTotal(args) {
  const n = k => Array.isArray(args?.[k]) && args[k].length ? args[k].length : 0;
  return n("tasks") || n("chain") || 1;
}
// Detail für die rechte Seite: Agentname(n) statt roher Argumente.
function runSubagentDetail(args) {
  const names = k => Array.isArray(args?.[k]) ? args[k].map(t => t?.agent).filter(a => typeof a === "string") : [];
  let joined;
  if (names("tasks").length) joined = "parallel: " + names("tasks").join(", ");
  else if (names("chain").length) joined = "Kette: " + names("chain").join(" → ");
  else {
    const agent = typeof args?.agent === "string" ? args.agent : "Subagent";
    const task = runToolDetail({ task: args?.task });
    joined = task ? `${agent}: ${task}` : agent;
  }
  const chars = [...joined];
  return chars.slice(0, 120).join("") + (chars.length > 120 ? "…" : "");
}

function createRunActivity() {
  const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;
  const fresh = () => ({ active: false, phase: "idle", retry: [0, 0], turns: 0, toolsDone: 0, toolErrors: 0, running: [], steering: 0, followUp: 0, last: null, subagentCalls: [], subagentsStarted: 0 });
  let st = fresh();
  const afterStep = () => st.running.length ? "tool" : st.active ? "waiting" : "idle";
  const api = {
    get state() { return st; },
    reset() { st = fresh(); },
    // pi beendet/abgestürzt: Lauf ohne Zusammenfassung beenden.
    halt() { st.active = false; st.running = []; st.subagentCalls = []; st.phase = "idle"; },
    // elapsed = Laufzeit seit Beginn in Sekunden (nur für agent_settled).
    event(ev, elapsed = 0) {
      switch (ev?.type) {
        case "agent_start":
          if (!st.active) st = { ...fresh(), active: true, phase: "waiting", last: st.last, steering: st.steering, followUp: st.followUp };
          break;
        case "turn_start": st.turns++; st.phase = "waiting"; break;
        case "message_update": {
          const t = ev.assistantMessageEvent?.type;
          if (t === "thinking_start" || t === "thinking_delta") st.phase = "thinking";
          else if (t === "text_start" || t === "text_delta") st.phase = "writing";
          else if (t === "toolcall_start" || t === "toolcall_delta") st.phase = "toolprep";
          break;
        }
        case "message_end": if (ev.message?.role === "assistant") st.phase = afterStep(); break;
        case "tool_execution_start": {
          const id = String(ev.toolCallId ?? "");
          st.running = st.running.filter(r => r.id !== id);
          let detail;
          if (runIsSubagentTool(ev.toolName)) {
            const total = runSubagentTotal(ev.args);
            st.subagentCalls = st.subagentCalls.filter(c => c.id !== id);
            st.subagentCalls.push({ id, total, done: 0 });
            st.subagentsStarted += total;
            detail = runSubagentDetail(ev.args);
          } else detail = runToolDetail(ev.args);
          st.running.push({ id, name: ev.toolName || "?", detail });
          st.phase = "tool";
          break;
        }
        case "tool_execution_update": {
          // Fortschritt aus partialResult.details: parallel → Ergebnisse mit exitCode ≠ -1,
          // Kette → abgeschlossene Schritte vor dem aktuellen; einzeln bleibt 0 bis zum Ende.
          const call = st.subagentCalls.find(c => c.id === String(ev.toolCallId ?? ""));
          const d = ev.partialResult?.details;
          if (call && Array.isArray(d?.results)) {
            const done = d.mode === "parallel" ? d.results.filter(r => typeof r?.exitCode === "number" && r.exitCode !== -1).length
              : d.mode === "chain" ? Math.max(0, d.results.length - 1) : 0;
            call.done = Math.min(done, call.total);
          }
          break;
        }
        case "tool_execution_end":
          st.running = st.running.filter(r => r.id !== String(ev.toolCallId ?? ""));
          st.subagentCalls = st.subagentCalls.filter(c => c.id !== String(ev.toolCallId ?? ""));
          st.toolsDone++;
          if (ev.isError === true) st.toolErrors++;
          st.phase = afterStep();
          break;
        case "compaction_start": st.phase = "compacting"; break;
        case "auto_retry_start": st.phase = "retrying"; st.retry = [Number(ev.attempt) || 0, Number(ev.maxAttempts) || 0]; break;
        case "compaction_end": case "auto_retry_end": st.phase = afterStep(); break;
        case "queue_update":
          st.steering = Array.isArray(ev.steering) ? ev.steering.length : 0;
          st.followUp = Array.isArray(ev.followUp) ? ev.followUp.length : 0;
          break;
        case "agent_settled":
          if (st.active) st.last = { seconds: elapsed, turns: st.turns, tools: st.toolsDone, errors: st.toolErrors, aborted: ev.aborted === true, subagents: st.subagentsStarted };
          st.active = false; st.running = []; st.subagentCalls = []; st.phase = "idle";
          break;
      }
    },
    // Was passiert gerade? Im Ruhezustand der übergebene App-Status („bereit“, „Fehler“ …).
    phaseLabel(idle) {
      switch (st.phase) {
        case "waiting": return "wartet auf Modell";
        case "thinking": return "denkt nach";
        case "writing": return "schreibt Antwort";
        case "toolprep": return "plant Werkzeugaufruf";
        case "tool": {
          const n = st.running.length;
          return n === 0 ? "Werkzeug" : n === 1 ? `Werkzeug: ${st.running[0].name}` : `Werkzeug: ${st.running[n - 1].name} +${n - 1}`;
        }
        case "compacting": return "komprimiert Kontext";
        case "retrying": return `Wiederholung ${st.retry[0]}/${st.retry[1]}`;
        default: return idle;
      }
    },
    // Detail des zuletzt gestarteten, noch laufenden Werkzeugs (leer, wenn keins läuft).
    detailLabel() { return st.running.length ? st.running[st.running.length - 1].detail : ""; },
    // Linke Seite: Zähler während des Laufs, sonst Zusammenfassung des letzten Laufs.
    countsLabel() {
      const parts = [];
      if (st.active) {
        if (st.turns > 0) parts.push(`Runde ${st.turns}`);
        parts.push(plural(st.toolsDone, "Werkzeug", "Werkzeuge"));
        if (st.toolErrors > 0) parts.push(plural(st.toolErrors, "Fehler", "Fehler"));
        if (st.subagentCalls.length) {
          const total = st.subagentCalls.reduce((n, c) => n + c.total, 0), done = st.subagentCalls.reduce((n, c) => n + c.done, 0);
          parts.push(`Subagenten ${done}/${total} fertig`);
        } else if (st.subagentsStarted > 0) parts.push(plural(st.subagentsStarted, "Subagent", "Subagenten"));
      } else if (st.last) {
        const l = st.last;
        parts.push(`Letzter Lauf ${runClock(l.seconds)}`);
        parts.push(plural(l.turns, "Runde", "Runden"));
        parts.push(plural(l.tools, "Werkzeug", "Werkzeuge"));
        if (l.errors > 0) parts.push(plural(l.errors, "Fehler", "Fehler"));
        if (l.subagents > 0) parts.push(plural(l.subagents, "Subagent", "Subagenten"));
        if (l.aborted) parts.push("abgebrochen");
      }
      const queued = st.steering + st.followUp;
      if (queued > 0) parts.push(`Warteschlange ${queued}`);
      return parts.join(" · ");
    },
  };
  return api;
}
if (typeof module !== "undefined") module.exports = { createRunActivity, runClock, runToolDetail };
