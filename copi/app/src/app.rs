//! Zustand und Logik von copi (entspricht ui/app.js in pi-lite).
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Color32, Rect};
use md_egui::DocCache;
use pi_backend::cli::{self, Cli, StartupInfo};
use pi_backend::folders::Location;
use pi_backend::rpc::Pi;
use pi_backend::sessions::{InfoCache, SessionInfo};
use pi_backend::{Event, Sink};
use serde_json::{json, Value};

use crate::commands::*;
use crate::store::Store;
use crate::theme;

/// Fortsetzung einer asynchronen Kette (wie `await` in pi-lite).
pub type Cont = Option<Box<dyn FnOnce(&mut App)>>;
/// Rückruf für eine RPC-Antwort (Zeitüberschreitung = `success:false`).
pub type Cb = Box<dyn FnOnce(&mut App, Value)>;
type TaskCb = Box<dyn FnOnce(&mut App, Box<dyn Any + Send>)>;

pub fn run(app: &mut App, c: Cont) {
    if let Some(f) = c {
        f(app)
    }
}

fn cont(f: impl FnOnce(&mut App) + 'static) -> Cont {
    Some(Box::new(f))
}

pub fn ok(r: &Value) -> bool {
    r["success"] == true
}

pub fn err_or(r: &Value, d: &str) -> String {
    r["error"].as_str().filter(|s| !s.is_empty()).unwrap_or(d).to_string()
}

pub const THINKING: [&str; 7] = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
pub const SIDE_VIEWS: [&str; 5] = ["Sessions", "Extensions / Plugins", "Skills", "pi · Agent- und Kontextdateien", "Andere Agent Harnesses"];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PickerKind {
    Workspace,
    Model,
    Thinking,
    SideView,
    Sort,
}

pub struct Picker {
    pub kind: PickerKind,
    pub cursor: usize,
    pub scroll: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ToastKind {
    Info,
    Warn,
    Error,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub expiry: Instant,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Job {
    Cli(String),
    Bash(String),
}

pub enum ChipAction {
    None,
    Insert(String),
    Open(String),
}

pub struct Chip {
    pub label: String,
    pub title: String,
    pub action: ChipAction,
}

pub struct StartCard {
    pub seq: u64,
    pub version: String,
    pub groups: Vec<(String, Color32, Vec<Chip>)>,
    pub open: bool,
    pub update: Option<String>,
}

pub enum Item {
    User(String),
    Assistant { doc: DocCache, text: String, done: bool },
    Thinking(String),
    Note { text: String, err: bool, block: bool },
    Tool { id: String, name: String, args: String, out: String, open: bool, err: bool, path: Option<String> },
    Shell { title: String, out: String, state: String, open: bool, err: bool },
    StartCard(StartCard),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DialogKind {
    Select,
    Confirm,
    Input,
    Editor,
}

pub enum Origin {
    Ext(String),
    Local(Box<dyn FnOnce(&mut App, Option<Value>)>),
}

pub struct Dialog {
    pub kind: DialogKind,
    pub title: String,
    pub message: String,
    pub options: Vec<String>,
    pub text: String,
    pub placeholder: String,
    pub cursor: usize,
    pub deadline: Option<Instant>,
    pub origin: Origin,
    pub focus: bool,
    pub scroll: bool,
}

struct Pending {
    deadline: Option<Instant>,
    ty: String,
    cb: Cb,
}

pub struct App {
    pub(crate) ctx: egui::Context,
    pub(crate) demo: bool,
    pub(crate) store: Store,
    pi: Pi,
    cli: Cli,
    sink: Sink,
    rx: mpsc::Receiver<Event>,
    generation: u64,
    pending: HashMap<String, Pending>,
    task_tx: mpsc::Sender<(u64, Box<dyn Any + Send>)>,
    task_rx: mpsc::Receiver<(u64, Box<dyn Any + Send>)>,
    task_cbs: HashMap<u64, TaskCb>,
    task_seq: u64,
    pub(crate) deferred: Vec<Box<dyn FnOnce(&mut App)>>,

    // Chatverlauf
    pub(crate) items: Vec<Item>,
    cur: Option<usize>,
    cur_think: Option<usize>,
    pub(crate) active_job: Option<Job>,
    pub(crate) log_epoch: u64,
    shells: HashMap<String, usize>,
    cli_jobs: HashMap<String, (usize, String)>,
    pub(crate) force_scroll: bool,

    // Daten von pi
    pub(crate) models: Vec<Value>,
    pub(crate) state: Value,
    pub(crate) commands: Vec<Cmd>,
    remote_commands: Vec<Cmd>,
    pub(crate) sessions: Vec<SessionInfo>,
    info_cache: Arc<Mutex<InfoCache>>,
    pub(crate) cwd: String,
    session_cwd: String,
    pub(crate) locations: Vec<Location>,

    // Status
    pub(crate) busy: bool,
    pub(crate) status: String,
    pub(crate) usage_text: String,
    pub(crate) speech_available: bool,
    pub(crate) speech_on: bool,
    pub(crate) ext_status: Vec<(String, String)>,
    pub(crate) widgets: Vec<(String, Vec<String>, String)>,

    // Eingabe
    pub(crate) prompt: String,
    pub(crate) history: InputHistory,
    pub(crate) slash_cursor: usize,
    pub(crate) slash_dismissed: Option<String>,
    pub(crate) slash_scroll: bool,
    pub(crate) pending_cursor: Option<usize>,
    pub(crate) prompt_rect: Rect,
    pub(crate) slash_rect: Rect,
    pub(crate) picker_rect: Rect,
    pub(crate) focus_prompt: bool,
    pub(crate) prompt_cursor: usize,
    pub(crate) prompt_focused: bool,

    // Überlagerungen
    pub(crate) dialogs: VecDeque<Dialog>,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) picker: Option<Picker>,
    pub(crate) anchors: HashMap<PickerKind, Rect>,
    pub(crate) keyhelp: bool,
    pub(crate) editor_open: bool,
    pub(crate) editor_text: String,
    pub(crate) editor_focus: bool,

    pub(crate) title: String,
    pub(crate) sent_title: String,
    start_seq: u64,
    card_seq: u64,
    usage_seq: u64,
    cli_seq: u64,
    req_seq: u64,
}

impl Drop for App {
    fn drop(&mut self) {
        if !self.demo {
            self.pi.stop();
            self.cli.cancel();
        }
    }
}

impl App {
    pub fn new(ctx: egui::Context, demo: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let c2 = ctx.clone();
        let sink = Sink::new(tx, Arc::new(move || c2.request_repaint()));
        let (task_tx, task_rx) = mpsc::channel();
        let locations = if demo {
            vec![
                Location { name: "Standard (Startordner)".into(), path: "~/Projekte/demo".into() },
                Location { name: "Home".into(), path: "~".into() },
            ]
        } else {
            pi_backend::folders::work_locations()
        };
        let cwd = locations.first().map(|l| l.path.clone()).unwrap_or_default();
        let mut app = Self {
            ctx,
            demo,
            store: Store::load(!demo),
            pi: Pi::default(),
            cli: Cli::default(),
            sink,
            rx,
            generation: 0,
            pending: HashMap::new(),
            task_tx,
            task_rx,
            task_cbs: HashMap::new(),
            task_seq: 0,
            deferred: Vec::new(),
            items: Vec::new(),
            cur: None,
            cur_think: None,
            active_job: None,
            log_epoch: 0,
            shells: HashMap::new(),
            cli_jobs: HashMap::new(),
            force_scroll: false,
            models: Vec::new(),
            state: Value::Null,
            commands: local_commands(),
            remote_commands: Vec::new(),
            sessions: Vec::new(),
            info_cache: Arc::new(Mutex::new(InfoCache::default())),
            cwd,
            session_cwd: String::new(),
            locations,
            busy: false,
            status: "startet…".into(),
            usage_text: "Tokenverbrauch: —".into(),
            speech_available: false,
            speech_on: false,
            ext_status: Vec::new(),
            widgets: Vec::new(),
            prompt: String::new(),
            history: InputHistory::default(),
            slash_cursor: 0,
            slash_dismissed: None,
            slash_scroll: false,
            pending_cursor: None,
            prompt_rect: Rect::NOTHING,
            slash_rect: Rect::NOTHING,
            picker_rect: Rect::NOTHING,
            focus_prompt: true,
            prompt_cursor: 0,
            prompt_focused: false,
            dialogs: VecDeque::new(),
            toasts: Vec::new(),
            picker: None,
            anchors: HashMap::new(),
            keyhelp: false,
            editor_open: false,
            editor_text: String::new(),
            editor_focus: false,
            title: "copi".into(),
            sent_title: String::new(),
            start_seq: 0,
            card_seq: 0,
            usage_seq: 0,
            cli_seq: 0,
            req_seq: 0,
        };
        let resume = if demo { None } else { pi_backend::folders::resume_session() };
        app.start(resume, None);
        app
    }

    // ---------- Infrastruktur ----------

    pub fn request(&mut self, v: Value, timeout_s: Option<u64>, cb: Cb) {
        self.req_seq += 1;
        let id = format!("r{}", self.req_seq);
        self.request_with_id(id, v, timeout_s, cb);
    }

    fn req(&mut self, v: Value, cb: impl FnOnce(&mut App, Value) + 'static) {
        self.request(v, Some(15), Box::new(cb));
    }

    fn request_with_id(&mut self, id: String, mut v: Value, timeout_s: Option<u64>, cb: Cb) {
        let ty = v["type"].as_str().unwrap_or("").to_string();
        v["id"] = json!(id);
        if self.demo {
            let e = format!("{ty}: nicht verfügbar im Demo-Modus");
            self.deferred.push(Box::new(move |a| cb(a, json!({"success": false, "error": e}))));
            return;
        }
        match self.pi.send(&v) {
            Ok(()) => {
                let deadline = timeout_s.map(|s| Instant::now() + Duration::from_secs(s));
                self.pending.insert(id, Pending { deadline, ty, cb });
            }
            Err(e) => self.deferred.push(Box::new(move |a| cb(a, json!({"success": false, "error": e})))),
        }
    }

    pub fn send(&mut self, v: Value) {
        if self.demo {
            return;
        }
        if let Err(e) = self.pi.send(&v) {
            self.note(e, true);
        }
    }

    /// Blockierende Arbeit im Hintergrund; `done` läuft danach im UI-Thread.
    pub fn spawn<T: Send + 'static>(&mut self, work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(&mut App, T) + 'static) {
        let cb: TaskCb = Box::new(move |a, v| {
            if let Ok(t) = v.downcast::<T>() {
                done(a, *t)
            }
        });
        if self.demo {
            // Deterministisch für Screenshots: sofort ausführen.
            let t = work();
            self.deferred.push(Box::new(move |a| cb(a, Box::new(t))));
            return;
        }
        self.task_seq += 1;
        let id = self.task_seq;
        self.task_cbs.insert(id, cb);
        let tx = self.task_tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let r: Box<dyn Any + Send> = Box::new(work());
            let _ = tx.send((id, r));
            ctx.request_repaint();
        });
    }

    pub(crate) fn drain(&mut self) {
        loop {
            let mut any = false;
            while let Ok((id, v)) = self.task_rx.try_recv() {
                any = true;
                if let Some(cb) = self.task_cbs.remove(&id) {
                    cb(self, v);
                }
            }
            let d = std::mem::take(&mut self.deferred);
            if !d.is_empty() {
                any = true;
                for f in d {
                    f(self);
                }
            }
            if !any {
                break;
            }
        }
    }

    /// Einmal pro Frame: Ereignisse, Hintergrundergebnisse, Zeitüberschreitungen.
    pub fn pump(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            self.on_backend(ev);
        }
        self.drain();
        let now = Instant::now();
        let expired: Vec<String> = self.pending.iter().filter(|(_, p)| p.deadline.is_some_and(|d| d <= now)).map(|(k, _)| k.clone()).collect();
        for id in expired {
            if let Some(p) = self.pending.remove(&id) {
                (p.cb)(self, json!({"success": false, "error": format!("{}: Zeitüberschreitung", p.ty)}));
            }
        }
        self.drain();
        self.toasts.retain(|t| t.expiry > now);
        if self.dialogs.front().and_then(|d| d.deadline).is_some_and(|d| d <= now) {
            self.close_dialog(None);
        }
        let next = self
            .pending
            .values()
            .filter_map(|p| p.deadline)
            .chain(self.toasts.iter().map(|t| t.expiry))
            .chain(self.dialogs.front().and_then(|d| d.deadline))
            .min();
        if let Some(n) = next {
            self.ctx.request_repaint_after(n.saturating_duration_since(now) + Duration::from_millis(5));
        }
    }

    fn on_backend(&mut self, ev: Event) {
        match ev {
            Event::Line(g, line) => {
                if g == self.generation {
                    if let Ok(v) = serde_json::from_str::<Value>(&line) {
                        self.handle(v);
                    }
                }
            }
            Event::Stderr(_, l) => eprintln!("pi: {l}"),
            Event::Exit(g) => {
                if g == self.generation {
                    self.drop_dialogs();
                    self.note("pi wurde beendet.", true);
                    self.set_status("beendet", false);
                }
            }
            Event::CliLine { id, line, .. } => {
                if let Some((idx, _)) = self.cli_jobs.get(&id).cloned() {
                    self.shell_append(idx, &(strip_ansi(&line) + "\n"));
                }
            }
            Event::CliDone { id, result } => {
                if self.active_job == Some(Job::Cli(id.clone())) {
                    self.active_job = None;
                }
                if let Some((idx, first)) = self.cli_jobs.remove(&id) {
                    match result {
                        Ok(0) => {
                            self.shell_finish(idx, "fertig", false);
                            if ["update", "install", "remove", "uninstall"].contains(&first.as_str()) {
                                self.note("Änderungen an pi und Erweiterungen wirken in einer neuen Session (Alt+N).", false);
                            }
                        }
                        Ok(n) => self.shell_finish(idx, &format!("Exit {n}"), true),
                        Err(e) => {
                            self.shell_finish(idx, "Fehler", true);
                            self.shell_append(idx, &(e + "\n"));
                        }
                    }
                }
            }
        }
    }

    // ---------- Chatverlauf ----------

    pub fn push(&mut self, item: Item) -> usize {
        self.items.push(item);
        self.items.len() - 1
    }

    pub fn note(&mut self, text: impl Into<String>, err: bool) {
        self.push(Item::Note { text: text.into(), err, block: false });
        self.force_scroll = true;
    }

    fn note_block(&mut self, text: String) {
        self.push(Item::Note { text, err: false, block: true });
        self.force_scroll = true;
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind) {
        let secs = if kind == ToastKind::Error { 9 } else { 5 };
        self.toasts.push(Toast { text: text.into(), kind, expiry: Instant::now() + Duration::from_secs(secs) });
    }

    pub fn set_status(&mut self, s: &str, busy: bool) {
        self.status = s.into();
        self.busy = busy;
    }

    fn clear_log(&mut self) {
        self.items.clear();
        self.cur = None;
        self.cur_think = None;
        self.shells.clear();
        self.cli_jobs.clear();
        self.log_epoch += 1;
    }

    fn finish_cur(&mut self) {
        if let Some(Item::Assistant { done, .. }) = self.cur.and_then(|i| self.items.get_mut(i)) {
            *done = true;
        }
    }

    fn add_tool(&mut self, id: &str, name: &str, args: &Value) {
        let args_v = if args.is_null() { json!({}) } else { args.clone() };
        let s = serde_json::to_string(&args_v).unwrap_or_default();
        let args_s: String = s.chars().take(160).collect();
        let path = if ["read", "write", "edit"].contains(&name) { args_v["path"].as_str().map(String::from) } else { None };
        self.push(Item::Tool { id: id.into(), name: name.into(), args: args_s, out: String::new(), open: false, err: false, path });
    }

    fn tool_mut(&mut self, id: &str) -> Option<&mut Item> {
        self.items.iter_mut().rev().find(|i| matches!(i, Item::Tool { id: t, .. } if t == id))
    }

    fn shell_append(&mut self, idx: usize, text: &str) {
        if let Some(Item::Shell { out, .. }) = self.items.get_mut(idx) {
            out.push_str(text);
        }
    }

    fn shell_finish(&mut self, idx: usize, label: &str, e: bool) {
        if let Some(Item::Shell { state, err, .. }) = self.items.get_mut(idx) {
            *state = label.into();
            *err = e;
        }
    }

    fn add_shell(&mut self, title: String) -> usize {
        self.force_scroll = true;
        self.push(Item::Shell { title, out: String::new(), state: "läuft …".into(), open: true, err: false })
    }

    pub fn show_history(&mut self, messages: &Value) {
        self.clear_log();
        for m in messages.as_array().into_iter().flatten() {
            match m["role"].as_str().unwrap_or("") {
                "user" => {
                    self.push(Item::User(text_of(&m["content"])));
                }
                "assistant" => {
                    for p in m["content"].as_array().into_iter().flatten() {
                        match p["type"].as_str().unwrap_or("") {
                            "text" => {
                                if let Some(t) = p["text"].as_str().filter(|t| !t.is_empty()) {
                                    self.push(Item::Assistant { doc: DocCache::default(), text: t.into(), done: true });
                                }
                            }
                            "thinking" => {
                                if let Some(t) = p["thinking"].as_str().filter(|t| !t.is_empty()) {
                                    self.push(Item::Thinking(t.into()));
                                }
                            }
                            "toolCall" => {
                                let (id, name) = (p["id"].as_str().unwrap_or("").to_string(), p["name"].as_str().unwrap_or("").to_string());
                                self.add_tool(&id, &name, &p["arguments"]);
                            }
                            _ => {}
                        }
                    }
                }
                "bashExecution" => {
                    let pre = if m["excludeFromContext"] == true { "!! " } else { "! " };
                    let idx = self.add_shell(format!("{pre}{}", m["command"].as_str().unwrap_or("")));
                    self.shell_append(idx, &strip_ansi(m["output"].as_str().unwrap_or("")));
                    let cancelled = m["cancelled"] == true;
                    let code = m["exitCode"].as_i64();
                    let label = if cancelled { "abgebrochen".into() } else { format!("Exit {}", code.map(|c| c.to_string()).unwrap_or("?".into())) };
                    self.shell_finish(idx, &label, cancelled || code.unwrap_or(0) > 0);
                }
                "toolResult" => {
                    let id = m["toolCallId"].as_str().unwrap_or("").to_string();
                    let text = text_of(&m["content"]);
                    let is_err = m["isError"] == true;
                    if let Some(Item::Tool { out, err, .. }) = self.tool_mut(&id) {
                        *out = text;
                        *err |= is_err;
                    }
                }
                _ => {}
            }
        }
        self.force_scroll = true;
    }

    fn handle(&mut self, v: Value) {
        match v["type"].as_str().unwrap_or("") {
            "response" => {
                let p = v["id"].as_str().and_then(|id| self.pending.remove(id));
                if let Some(p) = p {
                    (p.cb)(self, v);
                } else if !ok(&v) {
                    let c = v["command"].as_str().unwrap_or("").to_string();
                    self.note(format!("{c}: {}", err_or(&v, "Fehler")), true);
                }
            }
            "bash_execution_update" => {
                if let Some(&idx) = v["id"].as_str().and_then(|i| self.shells.get(i)) {
                    self.shell_append(idx, &strip_ansi(v["delta"].as_str().unwrap_or("")));
                }
            }
            "agent_start" => self.set_status("arbeitet …", true),
            "agent_settled" => {
                self.set_status("bereit", false);
                self.refresh_state(None);
                self.load_sessions();
            }
            "message_start" => {
                if v["message"]["role"] == "user" {
                    self.push(Item::User(text_of(&v["message"]["content"])));
                    self.force_scroll = true;
                } else {
                    self.cur = None;
                    self.cur_think = None;
                }
            }
            "message_update" => {
                let e = &v["assistantMessageEvent"];
                let delta = e["delta"].as_str().unwrap_or("");
                match e["type"].as_str().unwrap_or("") {
                    "text_delta" => {
                        let idx = match self.cur {
                            Some(i) => i,
                            None => {
                                let i = self.push(Item::Assistant { doc: DocCache::default(), text: String::new(), done: false });
                                self.cur = Some(i);
                                i
                            }
                        };
                        if let Some(Item::Assistant { text, .. }) = self.items.get_mut(idx) {
                            text.push_str(delta);
                        }
                    }
                    "thinking_delta" => {
                        let idx = match self.cur_think {
                            Some(i) => i,
                            None => {
                                let i = self.push(Item::Thinking(String::new()));
                                self.cur_think = Some(i);
                                i
                            }
                        };
                        if let Some(Item::Thinking(t)) = self.items.get_mut(idx) {
                            t.push_str(delta);
                        }
                    }
                    _ => {}
                }
            }
            "message_end" => {
                if v["message"]["role"] == "assistant" {
                    self.finish_cur();
                    self.cur = None;
                    self.cur_think = None;
                }
            }
            "tool_execution_start" => {
                self.finish_cur();
                self.cur = None;
                let (id, name) = (v["toolCallId"].as_str().unwrap_or("").to_string(), v["toolName"].as_str().unwrap_or("").to_string());
                self.add_tool(&id, &name, &v["args"]);
            }
            "tool_execution_update" | "tool_execution_end" => {
                let id = v["toolCallId"].as_str().unwrap_or("").to_string();
                let r = if v["result"].is_null() { &v["partialResult"] } else { &v["result"] };
                let text = text_of(&r["content"]);
                let is_err = v["isError"] == true;
                if let Some(Item::Tool { out, err, .. }) = self.tool_mut(&id) {
                    *out = text;
                    *err |= is_err;
                }
            }
            "auto_retry_start" => {
                let t = format!("Wiederholung {}/{}: {}", v["attempt"], v["maxAttempts"], v["errorMessage"].as_str().unwrap_or(""));
                self.note(t, true);
            }
            "compaction_start" => self.note("Kontext wird komprimiert …", false),
            "extension_ui_request" => self.ext_ui(v),
            _ => {}
        }
    }

    // ---------- Extension-UI ----------

    fn ext_ui(&mut self, ev: Value) {
        let s = |k: &str| ev[k].as_str().unwrap_or("").to_string();
        let method = s("method");
        match method.as_str() {
            "select" | "confirm" | "input" | "editor" => {
                let kind = match method.as_str() {
                    "select" => DialogKind::Select,
                    "confirm" => DialogKind::Confirm,
                    "input" => DialogKind::Input,
                    _ => DialogKind::Editor,
                };
                let options = ev["options"].as_array().map(|a| a.iter().map(|o| o.as_str().map(String::from).unwrap_or_else(|| o.to_string())).collect()).unwrap_or_default();
                let deadline = ev["timeout"].as_u64().filter(|t| *t > 0).map(|t| Instant::now() + Duration::from_millis(t));
                self.dialogs.push_back(Dialog {
                    kind,
                    title: s("title"),
                    message: s("message"),
                    options,
                    text: s("prefill"),
                    placeholder: s("placeholder"),
                    cursor: 0,
                    deadline,
                    origin: Origin::Ext(s("id")),
                    focus: true,
                    scroll: true,
                });
            }
            "notify" => {
                let kind = match ev["notifyType"].as_str() {
                    Some("error") => ToastKind::Error,
                    Some("warning") => ToastKind::Warn,
                    _ => ToastKind::Info,
                };
                self.toast(s("message"), kind);
            }
            "setStatus" => {
                let (key, text) = (s("statusKey"), s("statusText"));
                if key == "vorlesen" {
                    self.update_speech(&text);
                }
                if text.is_empty() {
                    self.ext_status.retain(|(k, _)| *k != key);
                } else if let Some(e) = self.ext_status.iter_mut().find(|(k, _)| *k == key) {
                    e.1 = text;
                } else {
                    self.ext_status.push((key, text));
                }
            }
            "setWidget" => {
                let key = s("widgetKey");
                let lines: Vec<String> = ev["widgetLines"].as_array().map(|a| a.iter().map(|l| l.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
                if lines.is_empty() {
                    self.widgets.retain(|(k, _, _)| *k != key);
                } else {
                    let place = ev["widgetPlacement"].as_str().unwrap_or("aboveEditor").to_string();
                    if let Some(w) = self.widgets.iter_mut().find(|(k, _, _)| *k == key) {
                        w.1 = lines;
                        w.2 = place;
                    } else {
                        self.widgets.push((key, lines, place));
                    }
                }
            }
            "setTitle" => self.title = s("title"),
            "set_editor_text" => {
                self.prompt = s("text");
                self.pending_cursor = Some(self.prompt.chars().count());
                self.focus_prompt = true;
            }
            _ => {}
        }
    }

    fn update_speech(&mut self, text: &str) {
        self.speech_on = !text.is_empty() && !text.contains("aus");
    }

    pub fn speech_command(&mut self, c: &str) {
        self.req(json!({"type": "prompt", "message": format!("/vorlesen {c}")}), |a, r| {
            if !ok(&r) {
                a.toast(err_or(&r, "Sprachbefehl fehlgeschlagen"), ToastKind::Error);
            }
        });
    }

    fn drop_dialogs(&mut self) {
        self.dialogs.clear();
        self.ext_status.clear();
        self.widgets.clear();
    }

    /// Antwort auf den vorderen Dialog; `None` = Zeitüberschreitung.
    pub fn close_dialog(&mut self, resp: Option<Value>) {
        let Some(d) = self.dialogs.pop_front() else { return };
        match d.origin {
            Origin::Ext(id) => {
                if let Some(mut r) = resp {
                    r["type"] = json!("extension_ui_response");
                    r["id"] = json!(id);
                    self.send(r);
                }
            }
            Origin::Local(f) => f(self, resp.filter(|r| r["cancelled"] != true)),
        }
        if let Some(n) = self.dialogs.front_mut() {
            n.focus = true;
        } else {
            self.focus_prompt = true;
        }
    }

    /// Lokaler Dialog; `cb` bekommt `None` bei Abbruch/Zeitüberschreitung.
    pub fn ask(&mut self, kind: DialogKind, title: &str, message: &str, options: Vec<String>, prefill: &str, cb: impl FnOnce(&mut App, Option<Value>) + 'static) {
        self.dialogs.push_back(Dialog {
            kind,
            title: title.into(),
            message: message.into(),
            options,
            text: prefill.into(),
            placeholder: String::new(),
            cursor: 0,
            deadline: None,
            origin: Origin::Local(Box::new(cb)),
            focus: true,
            scroll: true,
        });
    }

    // ---------- Start, Zustand, Modelle ----------

    pub fn start(&mut self, session: Option<String>, then: Cont) {
        self.start_seq += 1;
        self.commands = local_commands();
        self.remote_commands.clear();
        self.slash_cursor = 0;
        self.clear_log();
        if matches!(self.active_job, Some(Job::Bash(_))) {
            self.active_job = None;
        }
        self.pending.clear();
        self.drop_dialogs();
        self.usage_seq += 1;
        self.usage_text = "Tokenverbrauch: —".into();
        self.speech_available = false;
        self.set_status("startet…", false);
        if self.demo {
            self.fill_demo();
            run(self, then);
            return;
        }
        let had_session = session.is_some();
        let args: Vec<String> = session.map(|s| vec!["--session".into(), s]).unwrap_or_default();
        let cwd = self.cwd.clone();
        match self.pi.start(self.sink.clone(), &cwd, &args) {
            Err(e) => {
                self.note(e, true);
                self.set_status("Fehler", false);
                self.load_sessions();
                self.focus_prompt = true;
            }
            Ok(g) => {
                self.generation = g;
                self.session_cwd = cwd;
                let fin = move |a: &mut App| {
                    a.set_status("bereit", false);
                    if !had_session {
                        a.render_start_card();
                    }
                    run(a, then);
                    a.load_sessions();
                    a.focus_prompt = true;
                };
                self.load_models(cont(move |a| {
                    a.refresh_state(cont(move |a| {
                        a.detect_speech(cont(move |a| {
                            if had_session {
                                a.req(json!({"type": "get_messages"}), move |a, r| {
                                    if ok(&r) {
                                        a.show_history(&r["data"]["messages"]);
                                    }
                                    fin(a);
                                });
                            } else {
                                fin(a);
                            }
                        }))
                    }))
                }));
            }
        }
    }

    pub fn refresh_state(&mut self, then: Cont) {
        self.req(json!({"type": "get_state"}), move |a, r| {
            if ok(&r) {
                a.state = r["data"].clone();
                a.refresh_usage();
                a.title = match a.state["sessionName"].as_str().filter(|s| !s.is_empty()) {
                    Some(n) => format!("copi – {n}"),
                    None => "copi".into(),
                };
            }
            run(a, then);
        });
    }

    fn refresh_usage(&mut self) {
        self.usage_seq += 1;
        let seq = self.usage_seq;
        self.req(json!({"type": "get_session_stats"}), move |a, r| {
            if seq != a.usage_seq || !ok(&r) {
                return;
            }
            let t = &r["data"]["tokens"];
            let c = &r["data"]["contextUsage"];
            let n = |v: &Value| v.as_f64();
            let total = n(&t["input"]).unwrap_or(0.0) + n(&t["cacheRead"]).unwrap_or(0.0) + n(&t["cacheWrite"]).unwrap_or(0.0);
            let hit = if total > 0.0 { format!("{:.1}%", 100.0 * n(&t["cacheRead"]).unwrap_or(0.0) / total) } else { "—".into() };
            let context = if c.is_object() {
                let p = n(&c["percent"]).map(|p| format!("{p:.1}%")).unwrap_or("?".into());
                format!("{p}/{}", token_short(n(&c["contextWindow"])))
            } else {
                "—".into()
            };
            let auto = if a.state["autoCompactionEnabled"] == true { " (auto)" } else { "" };
            a.usage_text = format!(
                "↑{} ↓{} R{} CH{hit} {context}{auto}",
                token_short(n(&t["input"])),
                token_short(n(&t["output"])),
                token_short(n(&t["cacheRead"]))
            );
        });
    }

    pub fn load_models(&mut self, then: Cont) {
        self.req(json!({"type": "get_available_models"}), move |a, r| {
            if ok(&r) {
                a.models = r["data"]["models"].as_array().cloned().unwrap_or_default();
            }
            run(a, then);
        });
    }

    pub fn detect_speech(&mut self, then: Cont) {
        self.req(json!({"type": "get_commands"}), move |a, r| {
            let remote: Vec<Cmd> = r["data"]["commands"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| Cmd {
                    name: c["name"].as_str().unwrap_or("").into(),
                    description: c["description"].as_str().unwrap_or("").into(),
                    source: c["source"].as_str().unwrap_or("").into(),
                })
                .filter(|c| !c.name.is_empty())
                .collect();
            let mut all = local_commands();
            all.extend(remote.iter().filter(|c| !all.iter().any(|l| l.name == c.name)).cloned().collect::<Vec<_>>());
            a.commands = all;
            a.speech_available = ok(&r) && remote.iter().any(|c| c.name == "vorlesen");
            a.remote_commands = remote;
            run(a, then);
        });
    }

    pub fn model_label(&self) -> String {
        let m = &self.state["model"];
        if !m.is_object() {
            return "Modell auswählen".into();
        }
        let found = self.models.iter().find(|x| x["provider"] == m["provider"] && x["id"] == m["id"]).unwrap_or(m);
        model_text(found)
    }

    pub fn thinking_level(&self) -> String {
        self.state["thinkingLevel"].as_str().unwrap_or("").to_string()
    }

    pub fn session_file(&self) -> String {
        self.state["sessionFile"].as_str().unwrap_or("").to_string()
    }

    // ---------- Startkarte ----------

    fn render_start_card(&mut self) {
        self.card_seq += 1;
        let seq = self.card_seq;
        let demo = self.demo;
        let cwd = self.cwd.clone();
        self.spawn(
            move || {
                if demo {
                    StartupInfo {
                        version: "1.0.0".into(),
                        context: vec![cli::ContextFile { name: "AGENTS.md".into(), path: "~/Projekte/demo/AGENTS.md".into(), scope: "Projekt".into() }],
                        extensions: vec!["beispiel-erweiterung".into(), "vorlesen".into()],
                    }
                } else {
                    cli::startup_info(&cwd)
                }
            },
            move |a, info: StartupInfo| {
                if seq != a.card_seq || a.items.iter().any(|i| !matches!(i, Item::Note { .. })) {
                    return;
                }
                let mut skills: Vec<Chip> = a
                    .remote_commands
                    .iter()
                    .filter(|c| c.source == "skill")
                    .map(|c| Chip { label: c.name.trim_start_matches("skill:").into(), title: "In die Eingabe übernehmen".into(), action: ChipAction::Insert(format!("/{} ", c.name)) })
                    .collect();
                skills.sort_by(|x, y| x.label.to_lowercase().cmp(&y.label.to_lowercase()));
                let mut prompts: Vec<Chip> = a
                    .remote_commands
                    .iter()
                    .filter(|c| c.source == "prompt")
                    .map(|c| Chip { label: format!("/{}", c.name), title: "In die Eingabe übernehmen".into(), action: ChipAction::Insert(format!("/{} ", c.name)) })
                    .collect();
                prompts.sort_by(|x, y| x.label.to_lowercase().cmp(&y.label.to_lowercase()));
                let context = info.context.iter().map(|f| Chip { label: f.name.clone(), title: format!("Im Editor öffnen: {}", f.path), action: ChipAction::Open(f.path.clone()) }).collect();
                let ext = info.extensions.iter().map(|e| Chip { label: e.clone(), title: String::new(), action: ChipAction::None }).collect();
                let groups: Vec<(String, Color32, Vec<Chip>)> = vec![
                    ("Kontext".into(), theme::CAT_AGENTS, context),
                    ("Skills".into(), theme::CAT_SKILLS, skills),
                    ("Prompts".into(), theme::CAT_HARNESS, prompts),
                    ("Erweiterungen".into(), theme::CAT_EXT, ext),
                ]
                .into_iter()
                .filter(|g| !g.2.is_empty())
                .collect();
                let version = info.version.trim_start_matches('v').to_string();
                a.push(Item::StartCard(StartCard { seq, version: version.clone(), groups, open: true, update: None }));
                a.force_scroll = true;
                if a.demo || version.is_empty() {
                    return;
                }
                a.spawn(cli::latest_version, move |a, latest: Option<String>| {
                    let Some(l) = latest else { return };
                    if !cli::is_newer_version(&l, &version) {
                        return;
                    }
                    for it in a.items.iter_mut() {
                        if let Item::StartCard(c) = it {
                            if c.seq == seq {
                                c.update = Some(l.trim_start_matches('v').to_string());
                            }
                        }
                    }
                });
            },
        );
    }

    // ---------- Sessions ----------

    pub fn load_sessions(&mut self) {
        if self.demo {
            return;
        }
        let cache = self.info_cache.clone();
        self.spawn(move || cache.lock().map(|mut c| c.list()).unwrap_or_default(), |a, list| a.sessions = list);
    }

    pub fn visible_sessions(&self) -> Vec<SessionInfo> {
        let bm = self.store.sort == "bookmarks";
        let mut v: Vec<SessionInfo> = self.sessions.iter().filter(|s| !self.store.only_bookmarks || self.store.is_bookmark(&s.path)).cloned().collect();
        v.sort_by(|a, b| {
            let (ka, kb) = (bm && self.store.is_bookmark(&a.path), bm && self.store.is_bookmark(&b.path));
            kb.cmp(&ka).then(b.modified.cmp(&a.modified)).then(a.path.cmp(&b.path))
        });
        v
    }

    pub fn open_session(&mut self, s: &SessionInfo, then: Cont) {
        if self.busy {
            self.note("pi arbeitet noch – erst abbrechen.", true);
            return;
        }
        if self.demo {
            self.toast("Demo-Modus: Sessions werden nicht geöffnet.", ToastKind::Info);
            return;
        }
        self.cwd = s.cwd.clone();
        self.start(Some(s.path.clone()), then);
    }

    pub fn rename_session(&mut self, s: &SessionInfo) {
        if s.path != self.session_file() {
            if self.busy {
                self.note("pi arbeitet noch – erst abbrechen.", true);
                return;
            }
            let p = s.path.clone();
            self.open_session(
                s,
                cont(move |a| {
                    if a.session_file() == p {
                        a.rename_current();
                    }
                }),
            );
            return;
        }
        self.rename_current();
    }

    pub fn rename_current(&mut self) {
        let prefill = self.state["sessionName"].as_str().unwrap_or("").to_string();
        self.ask(DialogKind::Input, "Session umbenennen", "", vec![], &prefill, |a, r| {
            let Some(name) = r.and_then(|r| r["value"].as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()) else { return };
            a.req(json!({"type": "set_session_name", "name": name}), |a, r| {
                if !ok(&r) {
                    a.note(err_or(&r, "Umbenennen fehlgeschlagen"), true);
                    return;
                }
                a.refresh_state(cont(|a| a.load_sessions()));
            });
        });
    }

    pub fn remove_session(&mut self, s: &SessionInfo) {
        let label = [&s.name, &s.title].into_iter().find(|t| !t.is_empty()).cloned().unwrap_or("(leer)".into());
        let path = s.path.clone();
        self.ask(DialogKind::Confirm, "Session löschen?", &format!("{label}\n\nDie Datei wird endgültig gelöscht."), vec![], "", move |a, r| {
            if r.map(|r| r["confirmed"] == true) != Some(true) {
                return;
            }
            if a.demo {
                a.sessions.retain(|s| s.path != path);
                return;
            }
            let cache = a.info_cache.clone();
            a.spawn(
                move || cache.lock().map_err(|e| e.to_string()).and_then(|mut c| c.delete(&path)),
                |a, res: Result<(), String>| {
                    if let Err(e) = res {
                        a.note(e, true);
                    }
                    a.load_sessions();
                },
            );
        });
    }

    pub fn toggle_bookmark(&mut self, path: &str) {
        self.store.toggle_bookmark(path);
        self.save_store();
    }

    pub fn save_store(&mut self) {
        if let Err(e) = self.store.save(!self.demo) {
            self.toast(format!("Einstellungen nicht gespeichert: {e}"), ToastKind::Error);
        }
    }

    pub fn resize_output(&mut self, delta: i32) {
        self.store.output_size = if delta == 0 { 15.0 } else { (self.store.output_size + delta as f32).clamp(10.0, 30.0) };
        self.save_store();
    }

    pub fn new_session(&mut self) {
        self.start(None, None);
    }

    pub fn set_work_folder(&mut self, path: String) {
        self.cwd = path;
        self.toast("Arbeitsordner für die nächste neue Session ausgewählt.", ToastKind::Info);
    }

    pub fn choose_folder(&mut self) {
        let prefill = pi_backend::tilde(&self.cwd);
        self.ask(DialogKind::Input, "Arbeitsordner wählen", "", vec![], &prefill, |a, r| {
            let Some(v) = r.and_then(|r| r["value"].as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()) else { return };
            let p = match v.strip_prefix('~') {
                Some(rest) => pi_backend::home().map(|h| h.to_string_lossy().to_string()).unwrap_or_default() + rest,
                None => v,
            };
            if std::path::Path::new(&p).is_dir() {
                a.set_work_folder(p);
            } else {
                a.toast(format!("Kein Ordner: {p}"), ToastKind::Error);
            }
        });
    }

    // ---------- Picker ----------

    pub fn picker_rows(&self, kind: PickerKind) -> (Vec<String>, usize) {
        match kind {
            PickerKind::Workspace => {
                let mut rows: Vec<String> = self.locations.iter().map(|l| format!("{} · {}", l.name, pi_backend::tilde(&l.path))).collect();
                let cur = self.locations.iter().position(|l| l.path == self.cwd).unwrap_or(0);
                rows.push("Ordner wählen …".into());
                (rows, cur)
            }
            PickerKind::Model => {
                let m = &self.state["model"];
                let cur = self.models.iter().position(|x| x["provider"] == m["provider"] && x["id"] == m["id"]).unwrap_or(0);
                (self.models.iter().map(model_text).collect(), cur)
            }
            PickerKind::Thinking => {
                let t = self.thinking_level();
                (THINKING.iter().map(|l| format!("Denken: {l}")).collect(), THINKING.iter().position(|l| *l == t).unwrap_or(0))
            }
            PickerKind::SideView => (SIDE_VIEWS.iter().map(|s| s.to_string()).collect(), 0),
            PickerKind::Sort => (vec!["Zeit ↓".into(), "Bookmarks".into()], usize::from(self.store.sort == "bookmarks")),
        }
    }

    pub fn open_picker(&mut self, kind: PickerKind) {
        if self.picker.as_ref().is_some_and(|p| p.kind == kind) {
            self.picker = None;
            return;
        }
        let (rows, cur) = self.picker_rows(kind);
        if rows.is_empty() {
            self.toast("Keine Einträge verfügbar.", ToastKind::Info);
            return;
        }
        self.picker = Some(Picker { kind, cursor: cur.min(rows.len() - 1), scroll: true });
    }

    pub fn choose_pick(&mut self, kind: PickerKind, idx: usize) {
        self.picker = None;
        match kind {
            PickerKind::Workspace => {
                if let Some(l) = self.locations.get(idx) {
                    let p = l.path.clone();
                    self.set_work_folder(p);
                } else {
                    self.choose_folder();
                }
            }
            PickerKind::Model => {
                let Some(m) = self.models.get(idx).cloned() else { return };
                self.req(json!({"type": "set_model", "provider": m["provider"], "modelId": m["id"]}), |a, r| {
                    if !ok(&r) {
                        a.note(format!("Modell: {}", err_or(&r, "Fehler")), true);
                    }
                    a.refresh_state(None);
                });
                if self.demo {
                    self.state["model"] = m;
                }
            }
            PickerKind::Thinking => {
                let level = THINKING[idx.min(THINKING.len() - 1)];
                self.req(json!({"type": "set_thinking_level", "level": level}), |a, r| {
                    if !ok(&r) {
                        a.note(format!("Denkstufe: {}", err_or(&r, "Fehler")), true);
                    }
                    a.refresh_state(None);
                });
                if self.demo {
                    self.state["thinkingLevel"] = json!(level);
                }
            }
            PickerKind::SideView => {
                if idx != 0 {
                    self.toast(format!("„{}“ kommt in Stufe 3.", SIDE_VIEWS[idx.min(4)]), ToastKind::Info);
                }
            }
            PickerKind::Sort => {
                self.store.sort = if idx == 1 { "bookmarks".into() } else { "time".into() };
                self.save_store();
            }
        }
    }

    // ---------- Eingabe ----------

    pub fn set_prompt(&mut self, text: String) {
        self.pending_cursor = Some(text.chars().count());
        self.prompt = text;
        self.focus_prompt = true;
    }

    pub fn recall(&mut self, dir: i32) {
        let t = self.history.move_by(dir, &self.prompt);
        self.set_prompt(t);
    }

    pub fn submit(&mut self) {
        let t = self.prompt.trim().to_string();
        if t.is_empty() {
            return;
        }
        let parsed = parse_input(&t);
        self.history.add(&t);
        self.prompt.clear();
        self.slash_dismissed = None;
        self.slash_cursor = 0;
        match parsed {
            Parsed::Prompt => {
                if self.demo {
                    self.push(Item::User(t));
                    self.push(Item::Assistant { doc: DocCache::default(), text: "*Demo-Modus – pi läuft nicht.*".into(), done: true });
                    self.force_scroll = true;
                    return;
                }
                let mut v = json!({"type": "prompt", "message": t});
                if self.busy {
                    v["streamingBehavior"] = json!("steer");
                }
                let slash = t.starts_with('/');
                self.request(
                    v,
                    None,
                    Box::new(move |a, r| {
                        if !ok(&r) {
                            if a.prompt.is_empty() {
                                a.set_prompt(t.clone());
                            }
                            a.toast(err_or(&r, "Eingabe fehlgeschlagen"), ToastKind::Error);
                        }
                        if slash {
                            a.load_models(cont(|a| a.refresh_state(cont(|a| a.detect_speech(None)))));
                        }
                    }),
                );
            }
            Parsed::Bash { command, exclude } => self.run_bash(command, exclude),
            Parsed::Cli(args) => self.run_cli(args),
            Parsed::Local { name, arg } => self.run_local(&name, &arg),
            Parsed::Terminal(name) => self.note(format!("/{name} gibt es nur im pi-Terminal. Dort ausführen oder in copi über die Seitenleiste/Kopfzeile bedienen."), true),
        }
    }

    pub fn abort_current(&mut self) {
        match self.active_job.clone() {
            Some(Job::Cli(_)) => self.cli.cancel(),
            Some(Job::Bash(_)) => self.send(json!({"type": "abort_bash"})),
            None => self.send(json!({"type": "abort"})),
        }
    }

    pub fn run_cli(&mut self, args: Vec<String>) {
        if self.active_job.is_some() {
            self.toast("Es läuft bereits ein pi-Befehl oder eine Shell-Eingabe.", ToastKind::Error);
            return;
        }
        if self.demo {
            self.note("pi-Befehle sind im Demo-Modus nicht verfügbar.", true);
            return;
        }
        self.cli_seq += 1;
        let id = format!("c{}", self.cli_seq);
        let idx = self.add_shell(format!("pi {}", args.join(" ")));
        self.cli_jobs.insert(id.clone(), (idx, args.first().cloned().unwrap_or_default()));
        self.active_job = Some(Job::Cli(id.clone()));
        self.cli.run(self.sink.clone(), id, self.cwd.clone(), args);
    }

    fn run_bash(&mut self, command: String, exclude: bool) {
        if self.busy {
            self.note("pi arbeitet noch – erst abbrechen.", true);
            return;
        }
        if self.active_job.is_some() {
            self.toast("Es läuft bereits ein pi-Befehl oder eine Shell-Eingabe.", ToastKind::Error);
            return;
        }
        self.req_seq += 1;
        let id = format!("b{}", self.req_seq);
        let idx = self.add_shell(format!("{}{command}", if exclude { "!! " } else { "! " }));
        self.shells.insert(id.clone(), idx);
        self.active_job = Some(Job::Bash(id.clone()));
        let epoch = self.log_epoch;
        let id2 = id.clone();
        self.request_with_id(
            id,
            json!({"type": "bash", "command": command, "excludeFromContext": exclude}),
            None,
            Box::new(move |a, r| {
                if a.active_job == Some(Job::Bash(id2.clone())) {
                    a.active_job = None;
                }
                a.shells.remove(&id2);
                if epoch != a.log_epoch {
                    return;
                }
                if !ok(&r) {
                    a.shell_finish(idx, "Fehler", true);
                    a.shell_append(idx, &(err_or(&r, "bash fehlgeschlagen") + "\n"));
                    return;
                }
                let d = &r["data"];
                if matches!(a.items.get(idx), Some(Item::Shell { out, .. }) if out.is_empty()) {
                    a.shell_append(idx, &strip_ansi(d["output"].as_str().unwrap_or("")));
                }
                let cancelled = d["cancelled"] == true;
                let code = d["exitCode"].as_i64();
                let mut label = if cancelled { "abgebrochen".to_string() } else { format!("Exit {}", code.map(|c| c.to_string()).unwrap_or("?".into())) };
                if d["truncated"] == true {
                    label += " · gekürzt";
                }
                if exclude {
                    label += " · nicht im Kontext";
                }
                a.shell_finish(idx, &label, cancelled || code.unwrap_or(0) > 0);
                if let Some(p) = d["fullOutputPath"].as_str() {
                    a.note(format!("Vollständige Ausgabe: {p}"), false);
                }
            }),
        );
    }

    pub fn fork_from_here(&mut self) {
        if self.busy {
            self.note("pi arbeitet noch – erst abbrechen.", true);
            return;
        }
        self.req(json!({"type": "get_fork_messages"}), |a, r| {
            let msgs = r["data"]["messages"].as_array().cloned().unwrap_or_default();
            if msgs.is_empty() {
                a.note("Noch keine Nachricht zum Abzweigen.", true);
                return;
            }
            let labels: Vec<String> = msgs
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let t = m["text"].as_str().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ");
                    format!("{}. {}", i + 1, t.chars().take(80).collect::<String>())
                })
                .collect();
            let l2 = labels.clone();
            a.ask(DialogKind::Select, "Ab welcher Nachricht abzweigen?", "", labels, "", move |a, pick| {
                let Some(pick) = pick else { return };
                let Some(i) = l2.iter().position(|l| pick["value"] == l.as_str()) else { return };
                let m = msgs[i].clone();
                a.req(json!({"type": "fork", "entryId": m["entryId"]}), move |a, f| {
                    if !ok(&f) {
                        a.note(err_or(&f, "Fork fehlgeschlagen"), true);
                        return;
                    }
                    if f["data"]["cancelled"] == true {
                        a.note("Fork wurde von einer Extension abgebrochen.", true);
                        return;
                    }
                    let text = f["data"]["text"].as_str().or(m["text"].as_str()).unwrap_or("").to_string();
                    a.req(json!({"type": "get_messages"}), move |a, h| {
                        if ok(&h) {
                            a.show_history(&h["data"]["messages"]);
                        }
                        a.set_prompt(text);
                        a.refresh_state(cont(|a| a.load_sessions()));
                    });
                });
            });
        });
    }

    fn run_local(&mut self, name: &str, arg: &str) {
        if ["new", "compact", "clone", "fork", "name"].contains(&name) && self.busy {
            self.note("pi arbeitet noch – erst abbrechen.", true);
            return;
        }
        let n = name.to_string();
        let fail = move |a: &mut App, r: &Value| a.note(format!("/{n}: {}", err_or(r, "Fehler")), true);
        let arg = arg.to_string();
        match name {
            "model" => {
                if arg.is_empty() {
                    self.open_picker(PickerKind::Model);
                    return;
                }
                let q = arg.to_lowercase();
                let key = |m: &Value| format!("{}/{}", m["provider"].as_str().unwrap_or(""), m["id"].as_str().unwrap_or("")).to_lowercase();
                let hits: Vec<Value> = self.models.iter().filter(|m| key(m) == q || m["id"].as_str().unwrap_or("").to_lowercase() == q).cloned().collect();
                let found: Vec<Value> = if hits.len() == 1 {
                    hits
                } else {
                    self.models.iter().filter(|m| format!("{} {}", key(m), m["name"].as_str().unwrap_or("").to_lowercase()).contains(&q)).cloned().collect()
                };
                if found.len() != 1 {
                    if found.is_empty() {
                        self.note(format!("/model: Kein Modell „{arg}“."), true);
                    } else {
                        self.note(format!("/model: „{arg}“ ist nicht eindeutig ({} Treffer).", found.len()), true);
                        self.open_picker(PickerKind::Model);
                    }
                    return;
                }
                let m = found[0].clone();
                self.req(json!({"type": "set_model", "provider": m["provider"], "modelId": m["id"]}), move |a, r| {
                    if !ok(&r) {
                        fail(a, &r);
                        return;
                    }
                    a.refresh_state(cont(move |a| a.note(format!("Modell: {}/{}", m["provider"].as_str().unwrap_or(""), m["id"].as_str().unwrap_or("")), false)));
                });
            }
            "thinking" => {
                if arg.is_empty() {
                    let t = self.thinking_level();
                    self.note(format!("Denkstufe: {}", if t.is_empty() { "unbekannt".into() } else { t }), false);
                    return;
                }
                let l = arg.to_lowercase();
                if !THINKING.contains(&l.as_str()) {
                    self.note(format!("/thinking: Stufen sind {}", THINKING.join(", ")), true);
                    return;
                }
                self.req(json!({"type": "set_thinking_level", "level": l}), move |a, r| {
                    if !ok(&r) {
                        fail(a, &r);
                        return;
                    }
                    a.refresh_state(cont(move |a| {
                        let t = a.thinking_level();
                        a.note(format!("Denkstufe: {}", if t.is_empty() { l } else { t }), false)
                    }));
                });
            }
            "new" => self.new_session(),
            "name" => {
                if arg.is_empty() {
                    let n = self.state["sessionName"].as_str().filter(|s| !s.is_empty()).unwrap_or("(keiner)").to_string();
                    self.note(format!("Sessionname: {n}"), false);
                    return;
                }
                self.req(json!({"type": "set_session_name", "name": arg}), move |a, r| {
                    if !ok(&r) {
                        fail(a, &r);
                        return;
                    }
                    a.refresh_state(cont(move |a| {
                        a.load_sessions();
                        a.note(format!("Sessionname: {arg}"), false);
                    }));
                });
            }
            "session" => self.req(json!({"type": "get_session_stats"}), move |a, r| {
                if !ok(&r) {
                    fail(a, &r);
                    return;
                }
                let d = &r["data"];
                let t = &d["tokens"];
                let c = &d["contextUsage"];
                let s = |v: &Value| v.as_str().unwrap_or("").to_string();
                let mut lines = vec![
                    format!("Session {}", s(&d["sessionId"])),
                    s(&d["sessionFile"]),
                    format!("Nachrichten: {} Nutzer · {} Assistent · {} Tool-Aufrufe", d["userMessages"], d["assistantMessages"], d["toolCalls"]),
                    format!(
                        "Tokens: {} (Eingabe {}, Ausgabe {}, Cache {})",
                        token_short(t["total"].as_f64()),
                        token_short(t["input"].as_f64()),
                        token_short(t["output"].as_f64()),
                        token_short(t["cacheRead"].as_f64())
                    ),
                ];
                if let Some(cost) = d["cost"].as_f64() {
                    lines.push(format!("Kosten: ${cost:.4}"));
                }
                if c.is_object() {
                    let p = c["percent"].as_f64().map(|p| format!("{p:.1} %")).unwrap_or("—".into());
                    lines.push(format!("Kontext: {p} von {}", token_short(c["contextWindow"].as_f64())));
                }
                lines.retain(|l| !l.is_empty());
                a.note_block(lines.join("\n"));
            }),
            "compact" => {
                self.note("Kontext wird komprimiert …", false);
                let mut v = json!({"type": "compact"});
                if !arg.is_empty() {
                    v["customInstructions"] = json!(arg);
                }
                self.request(
                    v,
                    None,
                    Box::new(move |a, r| {
                        if !ok(&r) {
                            fail(a, &r);
                        } else {
                            let d = &r["data"];
                            a.note(format!("Komprimiert: {} → ca. {} Tokens.", token_short(d["tokensBefore"].as_f64()), token_short(d["estimatedTokensAfter"].as_f64())), false);
                        }
                        a.refresh_state(None);
                    }),
                );
            }
            "copy" => self.req(json!({"type": "get_last_assistant_text"}), |a, r| match r["data"]["text"].as_str().filter(|t| ok(&r) && !t.is_empty()) {
                Some(t) => {
                    a.ctx.copy_text(t.to_string());
                    a.toast("Letzte Antwort kopiert.", ToastKind::Info);
                }
                None => a.note("/copy: Noch keine Assistentenantwort.", true),
            }),
            "export" => {
                let mut v = json!({"type": "export_html"});
                if !arg.is_empty() {
                    v["outputPath"] = json!(arg);
                }
                self.request(
                    v,
                    Some(60),
                    Box::new(move |a, r| {
                        if !ok(&r) {
                            fail(a, &r);
                        } else {
                            a.note(format!("Exportiert: {}", r["data"]["path"].as_str().unwrap_or("")), false);
                        }
                    }),
                );
            }
            "clone" => self.req(json!({"type": "clone"}), move |a, r| {
                if !ok(&r) {
                    fail(a, &r);
                    return;
                }
                if r["data"]["cancelled"] == true {
                    a.note("Klonen wurde von einer Extension abgebrochen.", true);
                    return;
                }
                a.req(json!({"type": "get_messages"}), |a, h| {
                    if ok(&h) {
                        a.show_history(&h["data"]["messages"]);
                    }
                    a.refresh_state(cont(|a| {
                        a.load_sessions();
                        a.note("Session dupliziert.", false);
                    }));
                });
            }),
            "fork" => self.fork_from_here(),
            "resume" => {
                self.store.side_visible = true;
                self.save_store();
            }
            "hotkeys" => self.keyhelp = !self.keyhelp,
            "changelog" => {
                if let Err(e) = cli::open_url(cli::CHANGELOG_URL) {
                    self.toast(e, ToastKind::Error);
                }
            }
            "quit" => self.ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => {}
        }
    }

    pub fn open_editor(&mut self, path: Option<&str>) {
        let cwd = if self.session_cwd.is_empty() { self.cwd.clone() } else { self.session_cwd.clone() };
        if self.demo {
            self.toast("Demo-Modus: Editor wird nicht geöffnet.", ToastKind::Info);
            return;
        }
        if let Err(e) = pi_backend::editor::open_in_editor(&cwd, path, None) {
            self.toast(e, ToastKind::Error);
        }
    }

    // ---------- Demo ----------

    fn fill_demo(&mut self) {
        self.models = vec![
            json!({"provider": "beispiel", "id": "beispielmodell", "name": "Beispielmodell"}),
            json!({"provider": "beispiel", "id": "beispielmodell-mini", "name": "Beispielmodell Mini"}),
        ];
        self.state = json!({
            "model": {"provider": "beispiel", "id": "beispielmodell", "name": "Beispielmodell"},
            "thinkingLevel": "medium",
            "sessionFile": "/demo/sessions/a.jsonl",
            "sessionName": "Beispiel-Session",
        });
        self.remote_commands = vec![
            Cmd { name: "vorlesen".into(), description: "Antworten vorlesen".into(), source: "extension".into() },
            Cmd { name: "skill:beispiel".into(), description: "Beispiel-Skill".into(), source: "skill".into() },
            Cmd { name: "review".into(), description: "Änderungen prüfen".into(), source: "prompt".into() },
        ];
        let mut all = local_commands();
        all.extend(self.remote_commands.clone());
        self.commands = all;
        self.speech_available = true;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        self.sessions = (0..5)
            .map(|i| SessionInfo {
                path: if i == 0 { "/demo/sessions/a.jsonl".into() } else { format!("/demo/sessions/{i}.jsonl") },
                cwd: "~/Projekte/demo".into(),
                title: ["Beispiel-Session", "Markdown-Test", "Kurze Frage", "Refactoring planen", "Notizen"][i].into(),
                name: if i == 0 { "Beispiel-Session".into() } else { String::new() },
                modified: now - i as u64 * 7200,
            })
            .collect();
        if self.store.bookmarks.is_empty() {
            self.store.bookmarks.push("/demo/sessions/2.jsonl".into());
        }
        self.cwd = "~/Projekte/demo".into();
        self.session_cwd = self.cwd.clone();
        self.title = "copi – Beispiel-Session".into();
        self.push(Item::User("Wie ist das Projekt aufgebaut?".into()));
        self.push(Item::Thinking("Ich sehe mir zuerst die Struktur an …".into()));
        self.add_tool("t1", "read", &json!({"path": "README.md"}));
        if let Some(Item::Tool { out, .. }) = self.tool_mut("t1") {
            *out = "# Demo\nEin Beispielprojekt.".into();
        }
        self.push(Item::Assistant {
            doc: DocCache::default(),
            text: "## Aufbau\n\nDas Projekt hat **drei** Teile:\n\n- `md-core` – Markdown-Parser\n- `md-egui` – Darstellung\n- `app` – die Oberfläche\n\n```rust\nfn main() {\n    println!(\"Hallo\");\n}\n```\n\n| Teil | Zeilen |\n|---|---|\n| md-core | 700 |\n| app | 2000 |\n\nMehr unter [pi.dev](https://example.com).".into(),
            done: true,
        });
        let idx = self.add_shell("! ls".into());
        self.shell_append(idx, "Cargo.toml\napp\ncrates\n");
        self.shell_finish(idx, "Exit 0", false);
        self.note("Demo-Modus: pi läuft nicht.", false);
        self.usage_text = "↑12.4k ↓1.2k R8k CH39.2% 4.1%/200k (auto)".into();
        self.status = "bereit".into();
        self.locations = vec![
            Location { name: "Standard (Startordner)".into(), path: "~/Projekte/demo".into() },
            Location { name: "Home".into(), path: "~".into() },
        ];
    }

    pub fn demo_scene(&mut self, scene: &str) {
        match scene {
            "slash" => {
                self.set_prompt("/".into());
            }
            "picker" => self.open_picker(PickerKind::Model),
            "dialog" => self.ask(DialogKind::Select, "Ab welcher Nachricht abzweigen?", "", vec!["1. Wie ist das Projekt aufgebaut?".into(), "2. Zweite Beispielnachricht".into()], "", |_, _| {}),
            "confirm" => self.ask(DialogKind::Confirm, "Session löschen?", "Beispiel-Session\n\nDie Datei wird endgültig gelöscht.", vec![], "", |_, _| {}),
            "keyhelp" => self.keyhelp = true,
            "start" => {
                self.clear_log();
                self.render_start_card();
            }
            "toast" => self.toast("Arbeitsordner für die nächste neue Session ausgewählt.", ToastKind::Info),
            _ => {}
        }
    }
}

pub fn model_text(m: &Value) -> String {
    let id = m["id"].as_str().unwrap_or("");
    let name = m["name"].as_str().filter(|s| !s.is_empty()).unwrap_or(id);
    format!("{name} ({})", m["provider"].as_str().unwrap_or(""))
}
