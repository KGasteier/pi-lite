//! Mehrere Backends (lokal + pi-hub-Server) mit je mehreren Instanzen: Modell und Logik.
//!
//! GUI-frei bis auf den `DocCache` für Markdown. Alles Blockierende (list/create/remove/
//! restart/dirs und `Harness::send`, das beim Hub HTTP macht) läuft in Threads; Ergebnisse
//! und Ereignisse kommen über einen mpsc-Kanal zurück und werden in [`NetState::pump`] im
//! UI-Thread verarbeitet. Die Darstellung steht in `net_view.rs`.
use std::sync::{mpsc, Arc};

use md_egui::DocCache;
use pi_backend::backends::{self, BackendCfg};
use pi_backend::instances::{EvSink, Ev, InstanceInfo, Instances, Spec, State, Subscription, Tagged};
use serde_json::{json, Value};

use crate::commands::text_of;

/// (Backend-ID, Instanz-ID)
pub type InstKey = (String, String);

/// Höchstzahl an Verlaufseinträgen je Kachel (ältere fallen weg).
const MAX_ITEMS: usize = 2000;
/// Schonfrist, bevor eine `Info` mit weniger offenen Rückfragen eine Rückfrage entfernt.
const ASK_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// Ein Eintrag im Verlauf einer Instanz.
pub enum NetItem {
    User(String),
    Assistant { text: String, doc: DocCache, done: bool },
    Thinking(String),
    Tool { id: String, name: String, detail: String, done: bool, err: bool, args: String, out: String, open: bool, path: Option<String> },
    Note { text: String, err: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskKind {
    Confirm,
    Select,
    Input,
}

/// Offene Rückfrage des Agenten (inline in der Kachel beantwortet).
#[derive(Clone, Debug, PartialEq)]
pub struct Ask {
    pub id: String,
    pub kind: AskKind,
    pub title: String,
    pub message: String,
    pub options: Vec<String>,
    pub text: String,
    /// Eingang (für den Abgleich mit veralteten `Info`-Meldungen).
    pub since: std::time::Instant,
}

/// Verlauf, Rückfragen und Eingabe einer Instanz.
#[derive(Default)]
pub struct Chat {
    pub items: Vec<NetItem>,
    cur: Option<usize>,
    think: Option<usize>,
    pub asks: Vec<Ask>,
    pub input: String,
    /// Fertige Antwort, die lokal vorgelesen werden soll (nur im Live-Betrieb, nie beim Verlaufs-Nachladen).
    pub speak: Option<String>,
    /// Verlauf ist nachgeliefert (`Ev::Live`).
    pub live: bool,
    /// Modelle und Zustand des pi hinter dieser Instanz (für Modellwahl/Kopfzeile im Remote-Modus).
    pub models: Vec<Value>,
    pub state: Value,
}

impl Chat {
    fn push(&mut self, it: NetItem) -> usize {
        if self.items.len() >= MAX_ITEMS {
            let drop = self.items.len() + 1 - MAX_ITEMS;
            self.items.drain(..drop);
            self.cur = self.cur.and_then(|c| c.checked_sub(drop));
            self.think = self.think.and_then(|c| c.checked_sub(drop));
        }
        self.items.push(it);
        self.items.len() - 1
    }

    pub fn note(&mut self, text: impl Into<String>, err: bool) {
        self.push(NetItem::Note { text: text.into(), err });
    }

    fn finish(&mut self) {
        if let Some(NetItem::Assistant { done, .. }) = self.cur.and_then(|i| self.items.get_mut(i)) {
            *done = true;
        }
        self.cur = None;
        self.think = None;
    }

    /// Verlauf neu aufbauen (vor einem neuen Abonnement, das den Verlauf erneut liefert).
    pub fn reset(&mut self) {
        let input = std::mem::take(&mut self.input);
        *self = Chat { input, ..Default::default() };
    }

    /// Eine JSONL-Zeile des Agenten (pi-RPC-Ereignis) einarbeiten.
    pub fn line(&mut self, line: &str) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        match v["type"].as_str().unwrap_or("") {
            "message_start" => {
                if v["message"]["role"] == "user" {
                    self.push(NetItem::User(text_of(&v["message"]["content"])));
                } else {
                    self.cur = None;
                    self.think = None;
                }
            }
            "message_update" => {
                let e = &v["assistantMessageEvent"];
                let delta = e["delta"].as_str().unwrap_or("");
                match e["type"].as_str().unwrap_or("") {
                    "text_delta" => {
                        let i = match self.cur {
                            Some(i) => i,
                            None => {
                                let i = self.push(NetItem::Assistant { text: String::new(), doc: DocCache::default(), done: false });
                                self.cur = Some(i);
                                i
                            }
                        };
                        if let Some(NetItem::Assistant { text, .. }) = self.items.get_mut(i) {
                            text.push_str(delta);
                        }
                    }
                    "thinking_delta" => {
                        let i = match self.think {
                            Some(i) => i,
                            None => {
                                let i = self.push(NetItem::Thinking(String::new()));
                                self.think = Some(i);
                                i
                            }
                        };
                        if let Some(NetItem::Thinking(t)) = self.items.get_mut(i) {
                            t.push_str(delta);
                        }
                    }
                    _ => {}
                }
            }
            "message_end" if v["message"]["role"] == "assistant" => {
                // Endgültigen Text übernehmen (deckt auch Verläufe ohne Deltas ab).
                let full = text_of(&v["message"]["content"]);
                match self.cur.and_then(|i| self.items.get_mut(i)) {
                    Some(NetItem::Assistant { text, .. }) => {
                        if !full.is_empty() {
                            *text = full;
                        }
                    }
                    _ if !full.trim().is_empty() => {
                        let i = self.push(NetItem::Assistant { text: full, doc: DocCache::default(), done: false });
                        self.cur = Some(i);
                    }
                    _ => {}
                }
                self.finish();
            }
            "tool_execution_start" => {
                self.finish();
                let name = v["toolName"].as_str().unwrap_or("").to_string();
                let a = if v["args"].is_null() { json!({}) } else { v["args"].clone() };
                let args: String = serde_json::to_string(&a).unwrap_or_default().chars().take(160).collect();
                let path = if ["read", "write", "edit"].contains(&name.as_str()) { a["path"].as_str().map(String::from) } else { None };
                self.push(NetItem::Tool { id: v["toolCallId"].as_str().unwrap_or("").into(), detail: tool_detail(&name, &v["args"]), name, done: false, err: false, args, out: String::new(), open: false, path });
            }
            "tool_execution_update" | "tool_execution_end" => {
                let id = v["toolCallId"].as_str().unwrap_or("");
                let is_err = v["isError"] == true;
                let end = v["type"] == "tool_execution_end";
                let r = if v["result"].is_null() { &v["partialResult"] } else { &v["result"] };
                let text = text_of(&r["content"]);
                if let Some(NetItem::Tool { done, err, out, .. }) = self.items.iter_mut().rev().find(|i| matches!(i, NetItem::Tool { id: t, .. } if t == id)) {
                    if !text.is_empty() {
                        *out = text;
                    }
                    *done |= end;
                    *err |= is_err;
                }
            }
            "agent_settled" => {
                self.finish();
                self.asks.clear();
                if self.live && v["aborted"] != true {
                    // Letzte Antwort dieses Laufs (nach der letzten Nutzereingabe) zum lokalen Vorlesen.
                    let last_user = self.items.iter().rposition(|i| matches!(i, NetItem::User(_))).map(|p| p + 1).unwrap_or(0);
                    self.speak = self.items[last_user..].iter().rev().find_map(|i| match i {
                        NetItem::Assistant { text, .. } if !text.trim().is_empty() => Some(text.clone()),
                        _ => None,
                    });
                }
                if v["aborted"] == true {
                    self.note("abgebrochen", false);
                }
            }
            "auto_retry_start" => self.note(format!("Wiederholung {}/{}", v["attempt"], v["maxAttempts"]), true),
            "extension_ui_request" => {
                let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
                let kind = match v["method"].as_str().unwrap_or("") {
                    "confirm" => AskKind::Confirm,
                    "select" => AskKind::Select,
                    "input" | "editor" => AskKind::Input,
                    "notify" => {
                        let m = s("message");
                        if !m.is_empty() {
                            self.note(m, v["notifyType"] == "error");
                        }
                        return;
                    }
                    _ => return,
                };
                let id = s("id");
                if id.is_empty() || self.asks.iter().any(|a| a.id == id) {
                    return;
                }
                let options = v["options"].as_array().map(|a| a.iter().map(|o| o.as_str().map(String::from).unwrap_or_else(|| o.to_string())).collect()).unwrap_or_default();
                self.asks.push(Ask { id, kind, title: s("title"), message: s("message"), options, text: s("prefill"), since: std::time::Instant::now() });
            }
            "response" if v["success"] == true && v["command"] == "get_available_models" => {
                self.models = v["data"]["models"].as_array().cloned().unwrap_or_default();
            }
            "response" if v["success"] == true && v["command"] == "get_state" => {
                self.state = v["data"].clone();
            }
            "response" if v["success"] == false => {
                let c = v["command"].as_str().unwrap_or("");
                self.note(format!("{c}: {}", v["error"].as_str().unwrap_or("Fehler")), true);
            }
            _ => {}
        }
    }

    /// Rückfragen an die vom Backend gemeldete Zahl angleichen: Nach einem erneut gelieferten
    /// Verlauf tauchen auch längst beantwortete Anfragen wieder auf (Antworten sind Befehle,
    /// keine Ereignisse). Es wird nur gekürzt (älteste zuerst), nie ergänzt. `grace`: Rückfragen,
    /// die jünger sind, bleiben stehen – `Info` und Verlauf kommen beim Hub über getrennte
    /// Streams, eine ältere `Info` darf eine frische Rückfrage nicht löschen.
    pub fn reconcile_asks(&mut self, open: usize, grace: std::time::Duration) {
        let mut extra = self.asks.len().saturating_sub(open);
        let now = std::time::Instant::now();
        self.asks.retain(|a| {
            if extra > 0 && now.duration_since(a.since) >= grace {
                extra -= 1;
                false
            } else {
                true
            }
        });
    }
}

/// Kurzer Werkzeug-Hinweis (Pfad bzw. erste Befehlszeile).
fn tool_detail(name: &str, args: &Value) -> String {
    let s = match name {
        "bash" => args["command"].as_str().unwrap_or("").to_string(),
        _ => args["path"].as_str().map(String::from).unwrap_or_else(|| if args.is_null() { String::new() } else { args.to_string() }),
    };
    let first = s.lines().next().unwrap_or("");
    let mut t: String = first.chars().take(80).collect();
    if first.chars().count() > 80 || s.lines().count() > 1 {
        t.push('…');
    }
    t
}

pub struct Inst {
    pub info: InstanceInfo,
    pub chat: Chat,
    sub: Option<Subscription>,
}

impl Inst {
    pub fn new(info: InstanceInfo) -> Self {
        Self { info, chat: Chat::default(), sub: None }
    }
}

pub struct Backend {
    pub cfg: BackendCfg,
    pub api: Option<Arc<dyn Instances>>,
    /// Verbindungsstatus (`None` = noch unbekannt).
    pub link: Option<bool>,
    /// Letzter verständlicher Fehler (Token abgelehnt, nicht erreichbar …).
    pub error: Option<String>,
    pub insts: Vec<Inst>,
    watch: Option<Subscription>,
    /// Zählt Neuverbindungen; Ereignisse älterer Verbindungen werden verworfen.
    gen: u64,
}

impl Backend {
    pub fn new(cfg: BackendCfg) -> Self {
        Self { cfg, api: None, link: None, error: None, insts: Vec::new(), watch: None, gen: 0 }
    }
    pub fn remote(&self) -> bool {
        self.cfg.kind != "local"
    }
    pub fn inst(&self, id: &str) -> Option<&Inst> {
        self.insts.iter().find(|i| i.info.id == id)
    }
    fn inst_mut(&mut self, id: &str) -> Option<&mut Inst> {
        self.insts.iter_mut().find(|i| i.info.id == id)
    }
    /// Harnesses, die beim Anlegen angeboten werden. Der Vertrag meldet (noch) keine
    /// Harness-Liste; der Hub kennt `pi` und `mock`, lokal gibt es nur `pi`.
    pub fn harnesses(&self) -> Vec<&'static str> {
        if self.remote() {
            vec!["pi", "mock"]
        } else {
            vec!["pi"]
        }
    }
}

/// Formular „Backend hinzufügen/bearbeiten“. Das gespeicherte Token wird nie vorbelegt.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BackendForm {
    /// Index des bearbeiteten Backends (`None` = neu).
    pub edit: Option<usize>,
    pub name: String,
    pub url: String,
    pub token: String,
    pub token_env: String,
    /// `true` = SSH-Server (Host/Benutzer/Schlüssel), sonst Hub per URL.
    pub ssh: bool,
    pub host: String,
    pub user: String,
    pub identity_file: String,
    /// Ein Token ist bereits gespeichert (leeres Feld = unverändert).
    pub has_token: bool,
    pub error: Option<String>,
}

/// Formular „Neue Instanz“: Ordnerwahl über `Files::dirs` (im Thread).
#[derive(Clone, Debug, Default)]
pub struct NewInst {
    pub backend: String,
    pub path: String,
    pub parent: Option<String>,
    pub dirs: Vec<String>,
    pub loading: bool,
    pub name: String,
    pub harness: String,
    pub harnesses: Vec<&'static str>,
    pub error: Option<String>,
    pub busy: bool,
}

type Done = Box<dyn FnOnce(&mut NetState) + Send>;

enum Msg {
    Ev(u64, Tagged),
    Done(Done),
}

pub struct NetState {
    pub backends: Vec<Backend>,
    /// Ausgewählte Instanzen in Klickreihenfolge (Raster).
    pub selected: Vec<InstKey>,
    /// Raster statt Einzel-Chat.
    pub grid: bool,
    pub form: Option<BackendForm>,
    pub new_inst: Option<NewInst>,
    /// Meldungen für Toasts (Text, Fehler?) – nie mit Token.
    pub notices: Vec<(String, bool)>,
    pub started: bool,
    /// Sessions der Remote-Backends (für die gemeinsame Sessionliste, `backend` gesetzt).
    pub remote_sessions: Vec<pi_backend::sessions::SessionInfo>,
    /// Ohne Netz/Prozesse (Demo, Screenshots, Tests); `backends.json` wird nicht geschrieben.
    pub demo: bool,
    tx: mpsc::Sender<Msg>,
    rx: mpsc::Receiver<Msg>,
    repaint: Arc<dyn Fn() + Send + Sync>,
}

/// Fehlermeldung verständlich machen und ein evtl. enthaltenes Token schwärzen.
pub fn friendly_err(e: &str, token: &str) -> String {
    let e = if token.len() >= 4 { e.replace(token, "•••") } else { e.to_string() };
    if e.contains("401") {
        "Token abgelehnt (401) – bitte Token bzw. Umgebungsvariable prüfen.".into()
    } else if e.contains("nicht erreichbar") {
        let detail = e.split_once("nicht erreichbar:").map(|(_, d)| d.trim()).unwrap_or("");
        if detail.is_empty() {
            "Server nicht erreichbar – URL, Port und Netzverbindung prüfen.".into()
        } else {
            format!("Server nicht erreichbar – URL, Port und Netzverbindung prüfen ({detail}).")
        }
    } else {
        e
    }
}

/// Token, mit dem ein Backend tatsächlich spricht (nur zum Schwärzen von Fehlertexten).
fn secret(cfg: &BackendCfg) -> String {
    if cfg.token_env.is_empty() {
        cfg.token.clone()
    } else {
        std::env::var(&cfg.token_env).unwrap_or_default()
    }
}

/// Kurzform eines Arbeitsordners: `~` und höchstens die letzten zwei Teile.
pub fn short_cwd(cwd: &str) -> String {
    let t = pi_backend::tilde(cwd);
    let parts: Vec<&str> = t.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 2 {
        return t;
    }
    format!("…/{}", parts[parts.len() - 2..].join("/"))
}

/// Kennung aus dem Namen (klein, a–z/0–9/-), eindeutig gegenüber `taken`.
pub fn slug_id(name: &str, taken: &[&str]) -> String {
    let mut s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-');
    let base = if s.is_empty() { "hub".to_string() } else { format!("hub-{s}") };
    let mut id = base.clone();
    let mut n = 2;
    while taken.contains(&id.as_str()) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// SSH-Variante des Formulars: kein Token (der Hub erzeugt es auf dem Server), nur Zugangsdaten für `ssh`.
fn validate_ssh(f: &BackendForm, list: &[BackendCfg]) -> Result<BackendCfg, String> {
    let (name, host, user, key) = (f.name.trim(), f.host.trim(), f.user.trim(), f.identity_file.trim());
    if name.is_empty() {
        return Err("Bitte einen Namen angeben.".into());
    }
    if host.is_empty() || host.contains(char::is_whitespace) || host.starts_with('-') || host.contains('@') {
        return Err("Bitte einen Host angeben (IP, Name, Tailscale-Name oder Alias aus ~/.ssh/config), ohne Benutzer und Leerzeichen.".into());
    }
    if user.contains(char::is_whitespace) || user.starts_with('-') || user.contains('@') {
        return Err("Der Benutzername ist ungültig.".into());
    }
    if key.starts_with('-') {
        return Err("Der Schlüsselpfad ist ungültig.".into());
    }
    if list.iter().enumerate().any(|(i, b)| Some(i) != f.edit && b.label.trim().eq_ignore_ascii_case(name)) {
        return Err(format!("Es gibt schon ein Backend „{name}“."));
    }
    let old = f.edit.and_then(|i| list.get(i));
    let id = match old {
        Some(o) => o.id.clone(),
        None => {
            let taken: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
            slug_id(name, &taken).replacen("hub-", "ssh-", 1)
        }
    };
    let mut cfg = BackendCfg::ssh(&id, name, host, user);
    cfg.identity_file = key.into();
    if let Some(o) = old {
        cfg.enabled = o.enabled;
        cfg.ssh_port = o.ssh_port;
        cfg.hub_port = o.hub_port;
    }
    Ok(cfg)
}

/// Prüft das Formular und baut die Konfiguration. `list` ist die aktuelle Backendliste.
pub fn validate_form(f: &BackendForm, list: &[BackendCfg]) -> Result<BackendCfg, String> {
    if f.ssh {
        return validate_ssh(f, list);
    }
    let name = f.name.trim();
    let url = f.url.trim().trim_end_matches('/');
    let token = f.token.trim();
    let env = f.token_env.trim();
    if name.is_empty() {
        return Err("Bitte einen Namen angeben.".into());
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) || url.len() < 10 {
        return Err("Die URL muss mit http:// oder https:// beginnen, z. B. http://127.0.0.1:8787.".into());
    }
    if url.contains(char::is_whitespace) {
        return Err("Die URL darf keine Leerzeichen enthalten.".into());
    }
    if !token.is_empty() && !env.is_empty() {
        return Err("Entweder ein Token oder eine Umgebungsvariable angeben, nicht beides.".into());
    }
    if !env.is_empty() && !env.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Der Name der Umgebungsvariable darf nur A–Z, 0–9 und _ enthalten.".into());
    }
    let old = f.edit.and_then(|i| list.get(i));
    if token.is_empty() && env.is_empty() && !old.is_some_and(|o| !o.token.is_empty()) {
        return Err("Bitte ein Token oder den Namen einer Umgebungsvariable angeben.".into());
    }
    if list.iter().enumerate().any(|(i, b)| Some(i) != f.edit && b.label.trim().eq_ignore_ascii_case(name)) {
        return Err(format!("Es gibt schon ein Backend „{name}“."));
    }
    let id = match old {
        Some(o) => o.id.clone(),
        None => {
            let taken: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
            slug_id(name, &taken)
        }
    };
    let mut cfg = BackendCfg::hub(&id, name, url, token);
    if let Some(o) = old {
        cfg.enabled = o.enabled;
        if token.is_empty() && env.is_empty() {
            cfg.token = o.token.clone();
        }
    }
    if !env.is_empty() {
        cfg.token.clear();
        cfg.token_env = env.into();
    }
    Ok(cfg)
}

/// Spalten, Zeilenhöhe und ob gescrollt wird, für `n` Kacheln in einem Bereich `w`×`h`.
/// 1 Kachel: voll; 2: nebeneinander; 3–4: 2×2; mehr: zwei Spalten, scrollbar. Schmale
/// Bereiche (< 640 px) stapeln einspaltig.
pub fn grid_dims(n: usize, w: f32, h: f32) -> (usize, f32, bool) {
    const MIN_H: f32 = 260.0;
    if n == 0 {
        return (1, h, false);
    }
    let narrow = w < 640.0;
    let cols = if narrow || n == 1 { 1 } else { 2 };
    let rows = n.div_ceil(cols);
    let visible = if narrow { rows.min(2) } else { rows.min(2) };
    let tile_h = (h / visible as f32).max(MIN_H.min(h));
    let scroll = tile_h * rows as f32 > h + 0.5;
    (cols, tile_h, scroll)
}

impl NetState {
    pub fn new(repaint: Arc<dyn Fn() + Send + Sync>, demo: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        Self { backends: Vec::new(), selected: Vec::new(), grid: false, form: None, new_inst: None, notices: Vec::new(), started: false, remote_sessions: Vec::new(), demo, tx, rx, repaint }
    }

    fn notice(&mut self, text: impl Into<String>, err: bool) {
        self.notices.push((text.into(), err));
    }

    pub fn backend(&self, id: &str) -> Option<&Backend> {
        self.backends.iter().find(|b| b.cfg.id == id)
    }
    fn backend_mut(&mut self, id: &str) -> Option<&mut Backend> {
        self.backends.iter_mut().find(|b| b.cfg.id == id)
    }
    pub fn inst(&self, k: &InstKey) -> Option<&Inst> {
        self.backend(&k.0)?.inst(&k.1)
    }
    pub fn inst_mut(&mut self, k: &InstKey) -> Option<&mut Inst> {
        self.backend_mut(&k.0)?.inst_mut(&k.1)
    }

    /// Blockierende Arbeit im Thread; `done` läuft danach im UI-Thread (in `pump`).
    fn run<T: Send + 'static>(&self, work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(&mut NetState, T) + Send + 'static) {
        let (tx, repaint) = (self.tx.clone(), self.repaint.clone());
        std::thread::spawn(move || {
            let t = work();
            let _ = tx.send(Msg::Done(Box::new(move |n| done(n, t))));
            repaint();
        });
    }

    fn sink(&self, gen: u64) -> EvSink {
        let (tx, repaint) = (std::sync::Mutex::new(self.tx.clone()), self.repaint.clone());
        Arc::new(move |t| {
            let _ = tx.lock().unwrap().send(Msg::Ev(gen, t));
            repaint();
        })
    }

    /// Backends aus `backends.json` laden und die eingeschalteten verbinden (einmalig, beim
    /// ersten Öffnen der Leiste – der Einzel-Chat bleibt davon unberührt).
    pub fn start(&mut self) {
        self.started = true;
        let list = match backends::load() {
            Ok(l) => l,
            Err(e) => {
                self.notice(e, true);
                vec![BackendCfg::local()]
            }
        };
        self.backends = list.into_iter().map(Backend::new).collect();
        for i in 0..self.backends.len() {
            if self.backends[i].cfg.enabled {
                self.connect(i);
            }
        }
    }

    fn connect(&mut self, i: usize) {
        if self.demo {
            return;
        }
        let gen = {
            let b = &mut self.backends[i];
            b.gen += 1;
            b.link = None;
            b.error = None;
            b.gen
        };
        let api = match self.backends[i].cfg.build() {
            Ok(a) => a,
            Err(e) => {
                self.backends[i].error = Some(e);
                return;
            }
        };
        let sink = self.sink(gen);
        match api.watch(sink) {
            Ok(w) => self.backends[i].watch = Some(w),
            Err(e) => self.backends[i].error = Some(friendly_err(&e, &secret(&self.backends[i].cfg))),
        }
        self.backends[i].api = Some(api.clone());
        // Liste zusätzlich direkt holen: liefert bei Fehlern den Grund (401, nicht erreichbar).
        let (id, tok) = (self.backends[i].cfg.id.clone(), secret(&self.backends[i].cfg));
        self.run(move || api.list(), move |n, r| {
            let Some(b) = n.backend_mut(&id) else { return };
            if b.gen != gen {
                return;
            }
            match r {
                Ok(list) => {
                    b.error = None;
                    for info in list {
                        n.on_ev(Tagged { backend: id.clone(), inst: info.id.clone(), ev: Ev::Info(info) });
                    }
                    n.refresh_remote_sessions(&id);
                }
                Err(e) => b.error = Some(friendly_err(&e, &tok)),
            }
        });
    }

    fn disconnect(&mut self, i: usize) {
        let b = &mut self.backends[i];
        b.gen += 1;
        b.watch = None;
        b.api = None;
        b.link = None;
        b.insts.clear();
        let id = b.cfg.id.clone();
        self.selected.retain(|k| k.0 != id);
        self.remote_sessions.retain(|s| s.backend != id);
    }

    /// Einmal pro Frame. `visible`: Leiste ist offen (startet beim ersten Mal die Backends).
    pub fn pump(&mut self, visible: bool) {
        if visible && !self.started && !self.demo {
            self.start();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(6);
        for _ in 0..512 {
            let Ok(m) = self.rx.try_recv() else { break };
            match m {
                Msg::Ev(gen, t) => {
                    if self.backend(&t.backend).is_some_and(|b| b.gen == gen) {
                        self.on_ev(t);
                    }
                }
                Msg::Done(f) => f(self),
            }
            if std::time::Instant::now() >= deadline {
                (self.repaint)();
                break;
            }
        }
    }

    /// Ein Ereignis eines Backends einarbeiten.
    pub fn on_ev(&mut self, t: Tagged) {
        let key = (t.backend.clone(), t.inst.clone());
        let live = matches!(t.ev, Ev::Live);
        self.on_ev_inner(t);
        if live {
            self.request_meta(&key);
        }
    }

    /// Modelle und Zustand des pi hinter einer Instanz abfragen (kein Modellaufruf).
    pub fn request_meta(&mut self, k: &InstKey) {
        self.send(k, json!({"type": "get_available_models", "id": "copi-models"}));
        self.send(k, json!({"type": "get_state", "id": "copi-state"}));
    }

    /// Instanz, die im Hauptbereich gezeigt wird, wenn sie auf einem Remote-Backend läuft.
    pub fn remote_ctx(&self) -> Option<InstKey> {
        let k = self.selected.first()?;
        if !self.grid || !self.backend(&k.0)?.remote() {
            return None;
        }
        self.inst(k).map(|_| k.clone())
    }

    fn on_ev_inner(&mut self, t: Tagged) {
        let key = (t.backend.clone(), t.inst.clone());
        let Some(b) = self.backend_mut(&t.backend) else { return };
        match t.ev {
            Ev::Link(up) => {
                b.link = Some(up);
                if up {
                    b.error = None;
                }
            }
            Ev::Info(info) => match b.inst_mut(&info.id) {
                Some(i) => {
                    if i.chat.live {
                        i.chat.reconcile_asks(info.asks, ASK_GRACE);
                    }
                    i.info = info;
                }
                None => b.insts.push(Inst::new(info)),
            },
            Ev::Gone => {
                b.insts.retain(|i| i.info.id != t.inst);
                self.selected.retain(|k| *k != key);
            }
            Ev::Line(l) => {
                if let Some(i) = b.inst_mut(&t.inst) {
                    i.chat.line(&l);
                }
            }
            Ev::Exit => {
                if let Some(i) = b.inst_mut(&t.inst) {
                    i.info.state = State::Exited;
                    i.info.asks = 0;
                    i.chat.asks.clear();
                    i.chat.finish();
                    i.chat.note("Prozess beendet.", true);
                }
            }
            Ev::Live => {
                if let Some(i) = b.inst_mut(&t.inst) {
                    i.chat.live = true;
                    let open = i.info.asks;
                    i.chat.reconcile_asks(open, std::time::Duration::ZERO);
                }
            }
            Ev::Stderr(_) => {}
        }
    }

    /// Klick auf eine Instanz: ohne `multi` genau diese wählen, mit `multi` (Strg/Cmd) hinzu-
    /// bzw. wegnehmen. Schaltet auf das Raster und abonniert neu gewählte Instanzen.
    pub fn select(&mut self, k: InstKey, multi: bool) {
        if multi {
            if let Some(p) = self.selected.iter().position(|s| *s == k) {
                self.selected.remove(p);
            } else {
                self.selected.push(k);
            }
        } else {
            self.selected = vec![k];
        }
        if !self.selected.is_empty() {
            self.grid = true;
        }
        self.ensure_subs();
    }

    /// Für alle gewählten Instanzen ein Abonnement (Verlauf + live) halten.
    pub fn ensure_subs(&mut self) {
        let keys = self.selected.clone();
        for k in keys {
            let Some(b) = self.backends.iter().position(|b| b.cfg.id == k.0) else { continue };
            let (Some(api), gen) = (self.backends[b].api.clone(), self.backends[b].gen) else { continue };
            if self.backends[b].inst(&k.1).is_none_or(|i| i.sub.is_some()) {
                continue;
            }
            let sink = self.sink(gen);
            let tok = secret(&self.backends[b].cfg);
            let r = api.subscribe(&k.1, sink);
            let Some(i) = self.backends[b].inst_mut(&k.1) else { continue };
            i.chat.reset();
            match r {
                Ok(s) => i.sub = Some(s),
                Err(e) => i.chat.note(friendly_err(&e, &tok), true),
            }
        }
    }

    /// RPC-Befehl an eine Instanz (im Thread; beim Hub ist das HTTP).
    pub(crate) fn send(&mut self, k: &InstKey, cmd: Value) {
        let Some(b) = self.backend(&k.0) else { return };
        let Some(api) = b.api.clone() else {
            let msg = if self.demo { "Demo: nichts gesendet." } else { "Backend ist nicht verbunden." };
            if let Some(i) = self.inst_mut(k) {
                i.chat.note(msg, false);
            }
            return;
        };
        let (key, tok, id) = (k.clone(), secret(&b.cfg), k.1.clone());
        self.run(move || api.harness(&id).and_then(|h| h.send(&cmd)), move |n, r| {
            if let Err(e) = r {
                let m = friendly_err(&e, &tok);
                if let Some(i) = n.inst_mut(&key) {
                    i.chat.note(m, true);
                }
            }
        });
    }

    /// Eingabe der Kachel als Prompt senden (während der Agent arbeitet: als Steuerung).
    pub fn send_prompt(&mut self, k: &InstKey) {
        let Some(i) = self.inst_mut(k) else { return };
        let msg = i.chat.input.trim().to_string();
        if msg.is_empty() {
            return;
        }
        i.chat.input.clear();
        let mut cmd = json!({"type": "prompt", "message": msg});
        if matches!(i.info.state, State::Busy | State::Waiting) {
            cmd["streamingBehavior"] = json!("steer");
        }
        self.send(k, cmd);
    }

    pub fn abort(&mut self, k: &InstKey) {
        self.send(k, json!({"type": "abort"}));
    }

    /// Rückfrage beantworten; `resp` wie beim Einzel-Chat (`confirmed`/`value`/`cancelled`).
    pub fn answer(&mut self, k: &InstKey, ask_id: &str, mut resp: Value) {
        if let Some(i) = self.inst_mut(k) {
            i.chat.asks.retain(|a| a.id != ask_id);
        }
        resp["type"] = json!("extension_ui_response");
        resp["id"] = json!(ask_id);
        self.send(k, resp);
    }

    /// Stopp = Instanz entfernen.
    pub fn stop(&mut self, k: &InstKey) {
        let Some(api) = self.backend(&k.0).and_then(|b| b.api.clone()) else { return };
        let (key, tok) = (k.clone(), self.backend(&k.0).map(|b| secret(&b.cfg)).unwrap_or_default());
        let rid = key.1.clone();
        self.run(move || api.remove(&rid), move |n, r| match r {
            Ok(()) => n.on_ev(Tagged { backend: key.0.clone(), inst: key.1.clone(), ev: Ev::Gone }),
            Err(e) => n.notice(format!("Stoppen fehlgeschlagen: {}", friendly_err(&e, &tok)), true),
        });
    }

    /// Neustart – nur für beendete Instanzen.
    pub fn restart(&mut self, k: &InstKey) {
        if self.inst(k).is_none_or(|i| i.info.state != State::Exited) {
            return;
        }
        let Some(api) = self.backend(&k.0).and_then(|b| b.api.clone()) else { return };
        let (key, tok) = (k.clone(), self.backend(&k.0).map(|b| secret(&b.cfg)).unwrap_or_default());
        let rid = key.1.clone();
        self.run(move || api.restart(&rid), move |n, r| match r {
            Ok(()) => {
                // Verlauf neu abonnieren (lokal wird er beim Neustart geleert).
                if let Some(i) = n.inst_mut(&key) {
                    i.sub = None;
                    i.info.state = State::Starting;
                }
                n.ensure_subs();
            }
            Err(e) => n.notice(format!("Neustart fehlgeschlagen: {}", friendly_err(&e, &tok)), true),
        });
    }

    /// Ein-/Ausschalten (aus = Verbindung trennen; lokal: Instanzen werden beendet).
    pub fn set_enabled(&mut self, i: usize, on: bool) {
        if i >= self.backends.len() || self.backends[i].cfg.enabled == on {
            return;
        }
        self.backends[i].cfg.enabled = on;
        self.persist();
        if on {
            self.connect(i);
        } else {
            self.disconnect(i);
        }
    }

    fn persist(&mut self) {
        if self.demo {
            return;
        }
        let list: Vec<BackendCfg> = self.backends.iter().map(|b| b.cfg.clone()).collect();
        if let Err(e) = backends::save(&list) {
            self.notice(format!("backends.json nicht gespeichert: {e}"), true);
        }
    }

    pub fn open_form(&mut self, edit: Option<usize>) {
        let f = match edit.and_then(|i| self.backends.get(i)) {
            Some(b) => BackendForm { edit, ssh: b.cfg.kind == "ssh", host: b.cfg.host.clone(), user: b.cfg.user.clone(), identity_file: b.cfg.identity_file.clone(), name: b.cfg.label.clone(), url: b.cfg.url.clone(), token: String::new(), token_env: b.cfg.token_env.clone(), has_token: !b.cfg.token.is_empty(), error: None },
            None => BackendForm { url: "http://".into(), ..Default::default() },
        };
        self.form = Some(f);
    }

    /// Formular übernehmen: prüfen, speichern, (neu) verbinden. `true` = Dialog schließen.
    pub fn submit_form(&mut self) -> bool {
        let Some(f) = self.form.clone() else { return true };
        let list: Vec<BackendCfg> = self.backends.iter().map(|b| b.cfg.clone()).collect();
        let cfg = match validate_form(&f, &list) {
            Ok(c) => c,
            Err(e) => {
                if let Some(f) = &mut self.form {
                    f.error = Some(e);
                }
                return false;
            }
        };
        if !cfg.token_env.is_empty() && std::env::var(&cfg.token_env).is_err() {
            self.notice(format!("Umgebungsvariable {} ist nicht gesetzt.", cfg.token_env), true);
        }
        let label = cfg.label.clone();
        let i = match f.edit {
            Some(i) if i < self.backends.len() => {
                self.disconnect(i);
                self.backends[i].cfg = cfg;
                i
            }
            _ => {
                self.backends.push(Backend::new(cfg));
                self.backends.len() - 1
            }
        };
        self.persist();
        if self.backends[i].cfg.enabled {
            self.connect(i);
        }
        self.notice(format!("Backend „{label}“ gespeichert."), false);
        self.form = None;
        true
    }

    /// Hub-Backend entfernen (das lokale bleibt immer).
    pub fn delete_backend(&mut self, i: usize) {
        if i >= self.backends.len() || !self.backends[i].remote() {
            return;
        }
        self.disconnect(i);
        let b = self.backends.remove(i);
        self.persist();
        self.notice(format!("Backend „{}“ entfernt.", b.cfg.label), false);
    }

    pub fn open_new_inst(&mut self, backend: &str) {
        let Some(b) = self.backend(backend) else { return };
        let harnesses = b.harnesses();
        self.new_inst = Some(NewInst { backend: backend.into(), harness: "pi".into(), harnesses, loading: true, ..Default::default() });
        self.load_dirs(None);
    }

    /// Ordnerliste des Backends laden (im Thread).
    pub fn load_dirs(&mut self, path: Option<String>) {
        let Some(n) = &mut self.new_inst else { return };
        n.loading = true;
        n.error = None;
        let bid = n.backend.clone();
        let Some(b) = self.backend(&bid) else { return };
        let Some(api) = b.api.clone() else {
            if let Some(n) = &mut self.new_inst {
                n.loading = false;
                n.error = Some(if self.demo { "Demo: keine Ordnerliste.".into() } else { "Backend ist nicht verbunden.".into() });
            }
            return;
        };
        let tok = secret(&b.cfg);
        self.run(move || api.files().dirs(path.as_deref()), move |s, r| {
            let Some(n) = s.new_inst.as_mut().filter(|n| n.backend == bid) else { return };
            n.loading = false;
            match r {
                Ok(d) => {
                    n.path = d.path;
                    n.parent = d.parent;
                    n.dirs = d.dirs;
                }
                Err(e) => n.error = Some(friendly_err(&e, &tok)),
            }
        });
    }

    /// Unterordner betreten.
    pub fn enter_dir(&mut self, name: &str) {
        let Some(n) = &self.new_inst else { return };
        let p = if n.path.ends_with('/') { format!("{}{name}", n.path) } else { format!("{}/{name}", n.path) };
        self.load_dirs(Some(p));
    }

    /// Instanz im gewählten Ordner anlegen (im Thread) und danach auswählen.
    pub fn create_inst(&mut self) {
        let Some(n) = &mut self.new_inst else { return };
        if n.path.is_empty() || n.busy {
            return;
        }
        n.busy = true;
        let spec = Spec { name: Some(n.name.trim().to_string()).filter(|s| !s.is_empty()), harness: Some(n.harness.clone()), cwd: n.path.clone(), session: None };
        let bid = n.backend.clone();
        let Some(b) = self.backend(&bid) else { return };
        let Some(api) = b.api.clone() else { return };
        let tok = secret(&b.cfg);
        self.run(move || api.create(&spec), move |s, r| match r {
            Ok(info) => {
                let k = (bid.clone(), info.id.clone());
                s.on_ev(Tagged { backend: bid, inst: info.id.clone(), ev: Ev::Info(info) });
                s.new_inst = None;
                s.select(k, true);
            }
            Err(e) => {
                if let Some(n) = &mut s.new_inst {
                    n.busy = false;
                    n.error = Some(friendly_err(&e, &tok));
                }
            }
        });
    }

    /// Sessionliste eines Remote-Backends neu holen (im Thread) und in `remote_sessions` ersetzen.
    pub fn refresh_remote_sessions(&mut self, id: &str) {
        let Some(b) = self.backend(id).filter(|b| b.remote()) else { return };
        let Some(api) = b.api.clone() else { return };
        let (bid, gen, tok) = (id.to_string(), b.gen, secret(&b.cfg));
        self.run(move || api.files().sessions(), move |n, r| {
            if n.backend(&bid).is_none_or(|b| b.gen != gen) {
                return;
            }
            match r {
                Ok(list) => n.set_remote_sessions(&bid, list),
                Err(e) => n.notice(format!("Remote-Sessions: {}", friendly_err(&e, &tok)), true),
            }
        });
    }

    /// Alle verbundenen Remote-Backends abfragen (z. B. wenn die lokale Liste neu lädt).
    pub fn refresh_all_remote_sessions(&mut self) {
        let ids: Vec<String> = self.backends.iter().filter(|b| b.remote() && b.api.is_some()).map(|b| b.cfg.id.clone()).collect();
        for id in ids {
            self.refresh_remote_sessions(&id);
        }
    }

    pub fn set_remote_sessions(&mut self, backend: &str, list: Vec<pi_backend::instances::SessionRef>) {
        self.remote_sessions.retain(|s| s.backend != backend);
        self.remote_sessions.extend(list.into_iter().map(|r| pi_backend::sessions::SessionInfo {
            path: r.path,
            cwd: r.cwd,
            title: r.title,
            name: String::new(),
            modified: r.modified_ms / 1000,
            backend: backend.into(),
        }));
    }

    /// Anzeigename des Backends einer Remote-Session.
    pub fn backend_label(&self, id: &str) -> String {
        self.backend(id).map(|b| b.cfg.label.clone()).unwrap_or_else(|| id.into())
    }

    /// Instanz, in der die Remote-Session gerade offen ist (nicht beendet).
    pub fn inst_for_session(&self, backend: &str, path: &str) -> Option<InstKey> {
        let b = self.backend(backend)?;
        let i = b.insts.iter().find(|i| i.info.session.as_deref() == Some(path) && i.info.state != State::Exited)?;
        Some((backend.into(), i.info.id.clone()))
    }

    /// Remote-Session öffnen: läuft sie schon in einer Instanz, diese wählen; sonst eine
    /// neue Instanz mit `session` anlegen (der Hub prüft, dass der Pfad im Session-Ordner liegt).
    pub fn open_remote_session(&mut self, s: &pi_backend::sessions::SessionInfo) {
        if let Some(k) = self.inst_for_session(&s.backend, &s.path) {
            self.select(k, false);
            return;
        }
        let Some(b) = self.backend(&s.backend) else {
            self.notice(format!("Backend „{}“ ist nicht konfiguriert.", s.backend), true);
            return;
        };
        let Some(api) = b.api.clone() else {
            self.notice(format!("„{}“ ist nicht verbunden.", b.cfg.label), true);
            return;
        };
        let cwd = if s.cwd.is_empty() { String::new() } else { s.cwd.clone() };
        let spec = Spec { name: None, harness: Some("pi".into()), cwd, session: Some(s.path.clone()) };
        let (bid, tok) = (s.backend.clone(), secret(&b.cfg));
        self.run(move || api.create(&spec), move |n, r| match r {
            Ok(info) => {
                let k = (bid.clone(), info.id.clone());
                n.on_ev(Tagged { backend: bid, inst: info.id.clone(), ev: Ev::Info(info) });
                n.select(k, false);
            }
            Err(e) => n.notice(format!("Remote-Session öffnen: {}", friendly_err(&e, &tok)), true),
        });
    }

    /// Remote-Session auf dem Server löschen (der Hub lehnt offene Sessions mit 409 ab).
    pub fn delete_remote_session(&mut self, s: &pi_backend::sessions::SessionInfo) {
        if self.demo {
            self.remote_sessions.retain(|r| !(r.backend == s.backend && r.path == s.path));
            return;
        }
        let Some(b) = self.backend(&s.backend) else { return };
        let Some(api) = b.api.clone() else {
            self.notice(format!("„{}“ ist nicht verbunden.", b.cfg.label), true);
            return;
        };
        let (bid, path, tok) = (s.backend.clone(), s.path.clone(), secret(&b.cfg));
        self.run(move || api.files().delete_session(&path), move |n, r| {
            if let Err(e) = r {
                let m = if e.contains("409") { "Session ist auf dem Server noch in einer Instanz offen.".into() } else { friendly_err(&e, &tok) };
                n.notice(format!("Löschen fehlgeschlagen: {m}"), true);
            }
            n.refresh_remote_sessions(&bid);
        });
    }

    /// Remote-Session umbenennen (Hub setzt den Namen per pi-RPC; offene Sessions lehnt er mit 409 ab).
    pub fn rename_remote_session(&mut self, s: &pi_backend::sessions::SessionInfo, name: &str) {
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            return;
        }
        if self.demo {
            for r in self.remote_sessions.iter_mut().filter(|r| r.backend == s.backend && r.path == s.path) {
                r.name = name.clone();
            }
            return;
        }
        let Some(b) = self.backend(&s.backend) else { return };
        let Some(api) = b.api.clone() else {
            self.notice(format!("„{}“ ist nicht verbunden.", b.cfg.label), true);
            return;
        };
        let (bid, path, tok) = (s.backend.clone(), s.path.clone(), secret(&b.cfg));
        self.run(move || api.files().rename_session(&path, &name), move |n, r| {
            if let Err(e) = r {
                let m = if e.contains("409") { "Session ist auf dem Server noch in einer Instanz offen – erst die Instanz stoppen.".into() } else { friendly_err(&e, &tok) };
                n.notice(format!("Umbenennen fehlgeschlagen: {m}"), true);
            }
            n.refresh_remote_sessions(&bid);
        });
    }

    /// Zielordner für Abrufe: `~/pi-abruf/<backend>/<ordner>`.
    pub fn fetch_root() -> std::path::PathBuf {
        pi_backend::home().unwrap_or_else(std::env::temp_dir).join("pi-abruf")
    }

    /// Arbeitsordner einer Instanz per rsync holen (ohne Build-Cache, x86-64-Binaries werden verworfen).
    pub fn fetch_instance_dir(&mut self, k: &InstKey) {
        let Some(cwd) = self.inst(k).map(|i| i.info.cwd.clone()) else { return };
        let Some(b) = self.backend(&k.0) else { return };
        let Some(api) = b.api.clone() else {
            self.notice(format!("„{}“ ist nicht verbunden.", b.cfg.label), true);
            return;
        };
        if self.demo {
            self.notice(format!("Demo: {cwd} würde nach {} geholt.", Self::fetch_root().display()), false);
            return;
        }
        let (label, tok) = (b.cfg.label.clone(), secret(&b.cfg));
        self.notice(format!("Hole {cwd} von „{label}“ …"), false);
        self.run(
            move || api.fetch(&cwd, &Self::fetch_root(), pi_backend::ssh::local_machine()),
            move |n, r| match r {
                Ok(rep) => {
                    let mut m = format!("{} Dateien nach {} geholt.", rep.files, rep.dest);
                    if !rep.skipped_arch.is_empty() {
                        m += &format!(" {} x86-64-Binärdatei(en) übersprungen (laufen hier nicht): {}.", rep.skipped_arch.len(), rep.skipped_arch.iter().take(3).cloned().collect::<Vec<_>>().join(", "));
                    }
                    n.notice(m, false);
                }
                Err(e) => n.notice(format!("Abruf fehlgeschlagen: {}", friendly_err(&e, &tok)), true),
            },
        );
    }

    /// Synthetische Szene ohne Netz (`COPI_SCENE=net`): 2 Backends, 4 Instanzen, eine wartet.
    pub fn demo_scene(&mut self) {
        self.started = true;
        let info = |id: &str, name: &str, harness: &str, cwd: &str, state: State, asks: usize, last: &str| InstanceInfo {
            id: id.into(),
            name: name.into(),
            harness: harness.into(),
            cwd: cwd.into(),
            state,
            asks,
            last: last.into(),
            session: None,
        };
        let mut local = Backend::new(BackendCfg::local());
        local.link = Some(true);
        local.insts.push(Inst::new(info("l1", "copi", "pi", "/demo/Projekte/pi-lite/copi", State::Busy, 0, "Ich baue gerade die Tests …")));
        local.insts.push(Inst::new(info("l2", "notizen", "pi", "/demo/Notizen", State::Idle, 0, "Fertig: Zusammenfassung steht in README.md.")));
        let mut hub = Backend::new(BackendCfg::hub("hub-buero", "Server Büro", "http://buero.example:8787", ""));
        hub.link = Some(true);
        hub.insts.push(Inst::new(info("a1b2", "website", "mock", "/srv/demo/website", State::Waiting, 1, "Soll ich build/ löschen?")));
        hub.insts.push(Inst::new(info("c3d4", "alt", "mock", "/srv/demo/alt", State::Exited, 0, "")));
        self.backends = vec![local, hub];
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let refs = [("/srv/sessions/w1.jsonl", "/srv/demo/website", "Build-Pipeline reparieren", 1800), ("/srv/sessions/w2.jsonl", "/srv/demo/alt", "Server-Logs auswerten", 90_000)]
            .map(|(p, c, t, ago)| pi_backend::instances::SessionRef { path: p.into(), cwd: c.into(), title: t.into(), modified_ms: (now - ago) * 1000 });
        self.set_remote_sessions("hub-buero", refs.to_vec());
        let ev = |t: &str| t.to_string();
        let feed: [(&str, &str, Vec<String>); 3] = [
            ("lokal", "l1", vec![
                ev(r#"{"type":"message_start","message":{"role":"user","content":[{"type":"text","text":"Lass bitte die Tests laufen."}]}}"#),
                ev(r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Klar, ich starte **cargo test**."}}"#),
                ev(r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Klar, ich starte **cargo test**."}]}}"#),
                ev(r#"{"type":"tool_execution_start","toolCallId":"t1","toolName":"bash","args":{"command":"cargo test --workspace"}}"#),
                ev(r#"{"type":"tool_execution_end","toolCallId":"t1","isError":false}"#),
                ev(r#"{"type":"tool_execution_start","toolCallId":"t2","toolName":"read","args":{"path":"app/src/net.rs"}}"#),
                ev(r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Alle Tests grün. Jetzt sehe ich mir "}}"#),
            ]),
            ("lokal", "l2", vec![
                ev(r#"{"type":"message_start","message":{"role":"user","content":[{"type":"text","text":"Fasse die Notizen zusammen."}]}}"#),
                ev(r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Fertig: Zusammenfassung steht in `README.md`.\n\n- drei Themen\n- zwei offene Fragen"}]}}"#),
            ]),
            ("hub-buero", "a1b2", vec![
                ev(r#"{"type":"message_start","message":{"role":"user","content":[{"type":"text","text":"bitte lösch das Build-Verzeichnis"}]}}"#),
                ev(r#"{"type":"agent_start"}"#),
                ev(r#"{"type":"extension_ui_request","id":"ask-1","method":"confirm","title":"Dateien löschen?","message":"Der Agent möchte `build/` entfernen."}"#),
            ]),
        ];
        for (b, i, lines) in feed {
            for l in lines {
                self.on_ev(Tagged { backend: b.into(), inst: i.into(), ev: Ev::Line(l) });
            }
            self.on_ev(Tagged { backend: b.into(), inst: i.into(), ev: Ev::Live });
        }
        self.selected = vec![("lokal".into(), "l1".into()), ("hub-buero".into(), "a1b2".into()), ("lokal".into(), "l2".into())];
        self.grid = true;
    }
}
