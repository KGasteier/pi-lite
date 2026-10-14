//! Laufaktivität für die Statuszeile über der Eingabe (Gegenstück: ui/run-activity.js).
//!
//! Leitet aus dem pi-Ereignisstrom ab, *was* gerade passiert (Phase), und zählt Runden,
//! Werkzeugaufrufe, Fehler und die Warteschlange. Reine Anzeige: nichts davon wird
//! gespeichert oder gesendet; die Prozentschätzung bleibt in `Store::duration_percent`.
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Waiting,
    Thinking,
    Writing,
    ToolPrep,
    Tool,
    Compacting,
    Retrying(u64, u64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LastRun {
    pub seconds: f64,
    pub turns: u32,
    pub tools: u32,
    pub errors: u32,
    pub aborted: bool,
    pub subagents: u32,
}

/// Fortschritt eines laufenden Subagenten-Aufrufs (Extension `subagent`: single/parallel/chain).
#[derive(Clone, Debug, PartialEq)]
pub struct SubagentCall {
    pub id: String,
    pub total: u32,
    pub done: u32,
}

#[derive(Clone, Debug)]
pub struct RunActivity {
    pub active: bool,
    pub phase: Phase,
    pub turns: u32,
    pub tools_done: u32,
    pub tool_errors: u32,
    /// Laufende Werkzeuge (toolCallId, Name, Kurzdetail) in Startreihenfolge.
    pub running: Vec<(String, String, String)>,
    pub steering: usize,
    pub follow_up: usize,
    pub last: Option<LastRun>,
    /// Laufende Subagenten-Aufrufe und die Zahl der in diesem Lauf gestarteten Subagenten.
    pub subagent_calls: Vec<SubagentCall>,
    pub subagents_started: u32,
}

impl Default for RunActivity {
    fn default() -> Self {
        Self { active: false, phase: Phase::Idle, turns: 0, tools_done: 0, tool_errors: 0, running: Vec::new(), steering: 0, follow_up: 0, last: None, subagent_calls: Vec::new(), subagents_started: 0 }
    }
}

/// Kurzdetail eines Werkzeugaufrufs: Pfad, Befehl oder Suchmuster (einzeilig, gekürzt).
pub fn tool_detail(args: &Value) -> String {
    let raw = ["path", "command", "pattern", "url", "query"].iter().find_map(|k| args[*k].as_str())
        .or_else(|| args.as_object().and_then(|o| o.values().find_map(|v| v.as_str())))
        .unwrap_or("");
    let line = raw.lines().next().unwrap_or("").trim();
    let mut out: String = line.chars().take(120).collect();
    if line.chars().count() > 120 || raw.trim().lines().count() > 1 {
        out.push('…');
    }
    out
}

/// Subagenten-Werkzeug? (Extension `subagent`; tolerant für ähnlich benannte Varianten.)
pub fn is_subagent_tool(name: &str) -> bool {
    name.to_ascii_lowercase().contains("subagent")
}

/// Geplante Subagenten eines Aufrufs: `tasks` (parallel), `chain` (Kette) oder einer.
fn subagent_total(args: &Value) -> u32 {
    let n = |k: &str| args[k].as_array().map(|a| a.len() as u32).filter(|n| *n > 0);
    n("tasks").or_else(|| n("chain")).unwrap_or(1)
}

/// Detail für die rechte Seite: Agentname(n) statt roher Argumente.
fn subagent_detail(args: &Value) -> String {
    let names = |k: &str| args[k].as_array().map(|a| a.iter().filter_map(|t| t["agent"].as_str()).collect::<Vec<_>>());
    let joined = if let Some(t) = names("tasks").filter(|v| !v.is_empty()) {
        format!("parallel: {}", t.join(", "))
    } else if let Some(c) = names("chain").filter(|v| !v.is_empty()) {
        format!("Kette: {}", c.join(" → "))
    } else {
        let agent = args["agent"].as_str().unwrap_or("Subagent");
        let task = tool_detail(&serde_json::json!({ "task": args["task"] }));
        if task.is_empty() { agent.to_string() } else { format!("{agent}: {task}") }
    };
    let mut out: String = joined.chars().take(120).collect();
    if joined.chars().count() > 120 {
        out.push('…');
    }
    out
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Sekunden als m:ss.
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl RunActivity {
    /// Neue pi-Instanz/Session: alles vergessen.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// pi beendet/abgestürzt: Lauf ohne Zusammenfassung beenden.
    pub fn halt(&mut self) {
        self.active = false;
        self.running.clear();
        self.subagent_calls.clear();
        self.phase = Phase::Idle;
    }

    fn after_step(&self) -> Phase {
        if !self.running.is_empty() {
            Phase::Tool
        } else if self.active {
            Phase::Waiting
        } else {
            Phase::Idle
        }
    }

    /// Verarbeitet ein pi-Ereignis; `elapsed` = Laufzeit seit Beginn (nur für `agent_settled`).
    pub fn event(&mut self, v: &Value, elapsed: f64) {
        match v["type"].as_str().unwrap_or("") {
            "agent_start" => {
                if !self.active {
                    let (last, steering, follow_up) = (self.last.take(), self.steering, self.follow_up);
                    *self = Self { active: true, phase: Phase::Waiting, last, steering, follow_up, ..Self::default() };
                }
            }
            "turn_start" => {
                self.turns += 1;
                self.phase = Phase::Waiting;
            }
            "message_update" => {
                self.phase = match v["assistantMessageEvent"]["type"].as_str().unwrap_or("") {
                    "thinking_start" | "thinking_delta" => Phase::Thinking,
                    "text_start" | "text_delta" => Phase::Writing,
                    "toolcall_start" | "toolcall_delta" => Phase::ToolPrep,
                    _ => return,
                };
            }
            "message_end" if v["message"]["role"] == "assistant" => self.phase = self.after_step(),
            "tool_execution_start" => {
                let id = v["toolCallId"].as_str().unwrap_or("").to_string();
                let name = v["toolName"].as_str().unwrap_or("?").to_string();
                self.running.retain(|(i, ..)| *i != id);
                let detail = if is_subagent_tool(&name) {
                    let total = subagent_total(&v["args"]);
                    self.subagent_calls.retain(|c| c.id != id);
                    self.subagent_calls.push(SubagentCall { id: id.clone(), total, done: 0 });
                    self.subagents_started += total;
                    subagent_detail(&v["args"])
                } else {
                    tool_detail(&v["args"])
                };
                self.running.push((id, name, detail));
                self.phase = Phase::Tool;
            }
            "tool_execution_update" => {
                // Fortschritt aus partialResult.details: parallel → Ergebnisse mit exitCode ≠ -1,
                // Kette → abgeschlossene Schritte vor dem aktuellen; einzeln bleibt 0 bis zum Ende.
                let id = v["toolCallId"].as_str().unwrap_or("");
                if let Some(call) = self.subagent_calls.iter_mut().find(|c| c.id == id) {
                    let d = &v["partialResult"]["details"];
                    if let Some(results) = d["results"].as_array() {
                        let done = match d["mode"].as_str().unwrap_or("") {
                            "parallel" => results.iter().filter(|r| r["exitCode"].as_i64().is_some_and(|c| c != -1)).count(),
                            "chain" => results.len().saturating_sub(1),
                            _ => 0,
                        } as u32;
                        call.done = done.min(call.total);
                    }
                }
            }
            "tool_execution_end" => {
                let id = v["toolCallId"].as_str().unwrap_or("");
                self.running.retain(|(i, ..)| i != id);
                self.subagent_calls.retain(|c| c.id != id);
                self.tools_done += 1;
                if v["isError"] == true {
                    self.tool_errors += 1;
                }
                self.phase = self.after_step();
            }
            "compaction_start" => self.phase = Phase::Compacting,
            "auto_retry_start" => self.phase = Phase::Retrying(v["attempt"].as_u64().unwrap_or(0), v["maxAttempts"].as_u64().unwrap_or(0)),
            "compaction_end" | "auto_retry_end" => self.phase = self.after_step(),
            "queue_update" => {
                let n = |k: &str| v[k].as_array().map(|a| a.len()).unwrap_or(0);
                self.steering = n("steering");
                self.follow_up = n("followUp");
            }
            "agent_settled" => {
                if self.active {
                    self.last = Some(LastRun { seconds: elapsed, turns: self.turns, tools: self.tools_done, errors: self.tool_errors, aborted: v["aborted"] == true, subagents: self.subagents_started });
                }
                self.active = false;
                self.running.clear();
                self.subagent_calls.clear();
                self.phase = Phase::Idle;
            }
            _ => {}
        }
    }

    /// Was passiert gerade? Im Ruhezustand der übergebene App-Status („bereit“, „Fehler“ …).
    pub fn phase_label(&self, idle: &str) -> String {
        match &self.phase {
            Phase::Idle => idle.to_string(),
            Phase::Waiting => "wartet auf Modell".into(),
            Phase::Thinking => "denkt nach".into(),
            Phase::Writing => "schreibt Antwort".into(),
            Phase::ToolPrep => "plant Werkzeugaufruf".into(),
            Phase::Tool => match self.running.len() {
                0 => "Werkzeug".into(),
                1 => format!("Werkzeug: {}", self.running[0].1),
                n => format!("Werkzeug: {} +{}", self.running[n - 1].1, n - 1),
            },
            Phase::Compacting => "komprimiert Kontext".into(),
            Phase::Retrying(a, m) => format!("Wiederholung {a}/{m}"),
        }
    }

    /// Detail des zuletzt gestarteten, noch laufenden Werkzeugs (leer, wenn keins läuft).
    pub fn detail_label(&self) -> String {
        self.running.last().map(|(_, _, d)| d.clone()).unwrap_or_default()
    }

    /// Linke Seite: Zähler während des Laufs, sonst Zusammenfassung des letzten Laufs.
    pub fn counts_label(&self) -> String {
        let mut parts = Vec::new();
        if self.active {
            if self.turns > 0 {
                parts.push(format!("Runde {}", self.turns));
            }
            parts.push(plural(self.tools_done, "Werkzeug", "Werkzeuge"));
            if self.tool_errors > 0 {
                parts.push(plural(self.tool_errors, "Fehler", "Fehler"));
            }
            if !self.subagent_calls.is_empty() {
                let total: u32 = self.subagent_calls.iter().map(|c| c.total).sum();
                let done: u32 = self.subagent_calls.iter().map(|c| c.done).sum();
                parts.push(format!("Subagenten {done}/{total} fertig"));
            } else if self.subagents_started > 0 {
                parts.push(plural(self.subagents_started, "Subagent", "Subagenten"));
            }
        } else if let Some(l) = &self.last {
            parts.push(format!("Letzter Lauf {}", clock(l.seconds)));
            parts.push(plural(l.turns, "Runde", "Runden"));
            parts.push(plural(l.tools, "Werkzeug", "Werkzeuge"));
            if l.errors > 0 {
                parts.push(plural(l.errors, "Fehler", "Fehler"));
            }
            if l.subagents > 0 {
                parts.push(plural(l.subagents, "Subagent", "Subagenten"));
            }
            if l.aborted {
                parts.push("abgebrochen".into());
            }
        }
        let queued = self.steering + self.follow_up;
        if queued > 0 {
            parts.push(format!("Warteschlange {queued}"));
        }
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn phases_counts_and_last_run() {
        let mut a = RunActivity::default();
        assert_eq!(a.phase_label("bereit"), "bereit");
        assert_eq!(a.counts_label(), "");
        a.event(&json!({"type":"agent_start"}), 0.0);
        a.event(&json!({"type":"turn_start"}), 0.0);
        assert_eq!(a.phase_label("x"), "wartet auf Modell");
        a.event(&json!({"type":"message_update","assistantMessageEvent":{"type":"thinking_delta"}}), 0.0);
        assert_eq!(a.phase_label("x"), "denkt nach");
        a.event(&json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta"}}), 0.0);
        assert_eq!(a.phase_label("x"), "schreibt Antwort");
        a.event(&json!({"type":"tool_execution_start","toolCallId":"1","toolName":"bash","args":{"command":"cargo test\nmehr"}}), 0.0);
        a.event(&json!({"type":"tool_execution_start","toolCallId":"2","toolName":"read","args":{"path":"src/a.rs"}}), 0.0);
        assert_eq!(a.phase_label("x"), "Werkzeug: read +1");
        assert_eq!(a.detail_label(), "src/a.rs");
        a.event(&json!({"type":"tool_execution_end","toolCallId":"2","isError":true}), 0.0);
        assert_eq!(a.phase_label("x"), "Werkzeug: bash");
        assert_eq!(a.detail_label(), "cargo test…");
        a.event(&json!({"type":"tool_execution_end","toolCallId":"1"}), 0.0);
        assert_eq!(a.phase_label("x"), "wartet auf Modell");
        a.event(&json!({"type":"queue_update","steering":["a"],"followUp":["b","c"]}), 0.0);
        assert_eq!(a.counts_label(), "Runde 1 · 2 Werkzeuge · 1 Fehler · Warteschlange 3");
        a.event(&json!({"type":"auto_retry_start","attempt":1,"maxAttempts":3}), 0.0);
        assert_eq!(a.phase_label("x"), "Wiederholung 1/3");
        a.event(&json!({"type":"auto_retry_end"}), 0.0);
        // Zweiter agent_start innerhalb desselben Laufs (Retry) setzt nichts zurück.
        a.event(&json!({"type":"agent_start"}), 0.0);
        assert_eq!(a.tools_done, 2);
        a.event(&json!({"type":"queue_update","steering":[],"followUp":[]}), 0.0);
        a.event(&json!({"type":"agent_settled","aborted":true}), 133.0);
        assert_eq!(a.phase_label("bereit"), "bereit");
        assert_eq!(a.counts_label(), "Letzter Lauf 2:13 · 1 Runde · 2 Werkzeuge · 1 Fehler · abgebrochen");
        a.event(&json!({"type":"agent_start"}), 0.0);
        assert_eq!((a.turns, a.tools_done), (0, 0));
        assert!(a.last.is_some(), "letzter Lauf bleibt bis zum nächsten Abschluss erhalten");
        a.reset();
        assert!(a.last.is_none() && !a.active);
    }

    #[test]
    fn manual_compaction_while_idle_returns_to_idle() {
        let mut a = RunActivity::default();
        a.event(&json!({"type":"compaction_start","reason":"manual"}), 0.0);
        assert_eq!(a.phase_label("bereit"), "komprimiert Kontext");
        a.event(&json!({"type":"compaction_end"}), 0.0);
        assert_eq!(a.phase_label("bereit"), "bereit");
    }

    #[test]
    fn subagent_progress_counts() {
        let mut a = RunActivity::default();
        a.event(&json!({"type":"agent_start"}), 0.0);
        a.event(&json!({"type":"turn_start"}), 0.0);
        a.event(&json!({"type":"tool_execution_start","toolCallId":"p","toolName":"subagent","args":{"tasks":[{"agent":"scout","task":"a"},{"agent":"worker","task":"b"},{"agent":"scout","task":"c"}]}}), 0.0);
        assert_eq!(a.detail_label(), "parallel: scout, worker, scout");
        assert_eq!(a.counts_label(), "Runde 1 · 0 Werkzeuge · Subagenten 0/3 fertig");
        a.event(&json!({"type":"tool_execution_update","toolCallId":"p","toolName":"subagent","partialResult":{"details":{"mode":"parallel","results":[{"exitCode":0},{"exitCode":-1},{"exitCode":1}]}}}), 0.0);
        assert_eq!(a.counts_label(), "Runde 1 · 0 Werkzeuge · Subagenten 2/3 fertig");
        a.event(&json!({"type":"tool_execution_start","toolCallId":"c","toolName":"subagent","args":{"chain":[{"agent":"a","task":"x"},{"agent":"b","task":"{previous}"}]}}), 0.0);
        a.event(&json!({"type":"tool_execution_update","toolCallId":"c","partialResult":{"details":{"mode":"chain","results":[{"exitCode":0},{"exitCode":0}]}}}), 0.0);
        assert_eq!(a.detail_label(), "Kette: a → b");
        assert_eq!(a.counts_label(), "Runde 1 · 0 Werkzeuge · Subagenten 3/5 fertig");
        a.event(&json!({"type":"tool_execution_end","toolCallId":"p"}), 0.0);
        a.event(&json!({"type":"tool_execution_end","toolCallId":"c"}), 0.0);
        assert_eq!(a.counts_label(), "Runde 1 · 2 Werkzeuge · 5 Subagenten");
        a.event(&json!({"type":"tool_execution_start","toolCallId":"s","toolName":"subagent","args":{"agent":"scout","task":"Finde X\nmehr"}}), 0.0);
        assert_eq!(a.detail_label(), "scout: Finde X…");
        a.event(&json!({"type":"agent_settled"}), 61.0);
        assert_eq!(a.counts_label(), "Letzter Lauf 1:01 · 1 Runde · 2 Werkzeuge · 6 Subagenten");
    }
}
