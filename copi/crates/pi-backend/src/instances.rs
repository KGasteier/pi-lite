//! Mehrere pi-Instanzen hinter einem gemeinsamen Modell – lokal (Kindprozess) oder auf einem
//! entfernten `pi-hub` (HTTP + SSE). GUI-frei; die Oberflächen (copi, pi-lite, cocopi) sprechen
//! nur gegen [`Instances`] und [`Files`] und sehen keinen Unterschied zwischen den Transporten.
//!
//! Ereignisse laufen als [`Tagged`] (Backend, Instanz, [`Ev`]) über einen [`Sink`](crate::Sink)-
//! ähnlichen Rückruf [`EvSink`]. Der Hub-Transport nimmt abgerissene Streams mit `Last-Event-ID`
//! wieder auf, ohne Ereignisse doppelt zu liefern.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Zustand einer Instanz (identisch zu den Hub-Zuständen).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Starting,
    Idle,
    Busy,
    Waiting,
    Exited,
}

impl State {
    pub fn parse(s: &str) -> State {
        match s {
            "starting" => State::Starting,
            "busy" => State::Busy,
            "waiting" => State::Waiting,
            "exited" => State::Exited,
            _ => State::Idle,
        }
    }
}

/// Kurzbeschreibung einer Instanz für Listen, Pillen und Raster.
#[derive(Clone, Debug, PartialEq)]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub harness: String,
    pub cwd: String,
    pub state: State,
    /// Offene Rückfragen (Badge).
    pub asks: usize,
    /// Letzte Assistententext-Zeile (Vorschau).
    pub last: String,
    pub session: Option<String>,
}

impl InstanceInfo {
    fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            id: v["id"].as_str()?.to_string(),
            name: v["name"].as_str().unwrap_or("").to_string(),
            harness: v["harness"].as_str().unwrap_or("pi").to_string(),
            cwd: v["cwd"].as_str().unwrap_or("").to_string(),
            state: State::parse(v["state"].as_str().unwrap_or("idle")),
            asks: v["asks"].as_u64().unwrap_or(0) as usize,
            last: v["last"].as_str().unwrap_or("").to_string(),
            session: v["session"].as_str().map(String::from),
        })
    }
}

/// Wunsch für eine neue Instanz.
#[derive(Clone, Debug, Default)]
pub struct Spec {
    pub name: Option<String>,
    pub harness: Option<String>,
    pub cwd: String,
    pub session: Option<String>,
}

/// Was eine Instanz meldet.
#[derive(Clone, Debug, PartialEq)]
pub enum Ev {
    /// JSONL-Zeile des Agenten (pi-RPC-Ereignis oder -Antwort).
    Line(String),
    Stderr(String),
    /// Der Agentenprozess ist beendet.
    Exit,
    /// Instanz neu gemeldet oder verändert (Zustand, Vorschau, Badge).
    Info(InstanceInfo),
    /// Instanz wurde entfernt.
    Gone,
    /// Verlauf bis hierher ist nachgeliefert; ab jetzt live.
    Live,
    /// Verbindung zum Backend steht (`true`) bzw. ist weg (`false`, Wiederaufbau läuft).
    Link(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tagged {
    pub backend: String,
    /// Leer bei Backend-weiten Ereignissen (`Link`).
    pub inst: String,
    pub ev: Ev,
}

/// Rückruf für Ereignisse aus Hintergrund-Threads (muss selbst thread-sicher sein).
pub type EvSink = Arc<dyn Fn(Tagged) + Send + Sync>;

/// Haltegriff auf ein Abonnement; beim Löschen endet der Hintergrund-Thread.
pub struct Subscription {
    stop: Arc<AtomicBool>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Steuerung einer einzelnen laufenden Instanz.
pub trait Harness: Send + Sync {
    /// RPC-Befehl (pi-JSON) an den Agenten.
    fn send(&self, cmd: &Value) -> Result<(), String>;
    fn abort(&self) -> Result<(), String> {
        self.send(&json!({"type": "abort"}))
    }
}

/// Eintrag in der Sessionliste eines Backends.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRef {
    pub path: String,
    pub modified_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirList {
    pub path: String,
    pub parent: Option<String>,
    pub dirs: Vec<String>,
}

/// Dateisystem-Sicht des Backends (lokal oder auf dem Server).
pub trait Files: Send + Sync {
    fn dirs(&self, path: Option<&str>) -> Result<DirList, String>;
    fn sessions(&self) -> Result<Vec<SessionRef>, String>;
}

/// Ein Backend mit beliebig vielen Instanzen.
pub trait Instances: Send + Sync {
    /// Stabile Kennung innerhalb der App (z. B. "lokal", "hub-server").
    fn id(&self) -> &str;
    fn label(&self) -> String;
    fn is_remote(&self) -> bool;
    fn list(&self) -> Result<Vec<InstanceInfo>, String>;
    fn create(&self, spec: &Spec) -> Result<InstanceInfo, String>;
    fn remove(&self, id: &str) -> Result<(), String>;
    /// Nur für beendete Instanzen.
    fn restart(&self, id: &str) -> Result<(), String>;
    /// Verlauf und Live-Ereignisse einer Instanz; Verlauf zuerst, dann `Ev::Live`.
    fn subscribe(&self, id: &str, sink: EvSink) -> Result<Subscription, String>;
    /// Backend-weite Ereignisse (`Info`/`Gone`/`Link`) für Leiste und Badges.
    fn watch(&self, sink: EvSink) -> Result<Subscription, String>;
    fn harness(&self, id: &str) -> Result<Arc<dyn Harness>, String>;
    fn files(&self) -> &dyn Files;
}

// ───────────────────────────── lokal ─────────────────────────────

struct LocalInst {
    info: Mutex<InstanceInfo>,
    pi: crate::rpc::Pi,
    subs: Mutex<Vec<(u64, EvSink)>>,
    /// Verlauf der Sitzung im Speicher (begrenzt), für späte Abonnenten.
    hist: Mutex<std::collections::VecDeque<Ev>>,
}

const LOCAL_HIST: usize = 5000;

pub struct LocalInstances {
    id: String,
    inner: Arc<LocalInner>,
}

struct LocalInner {
    id: String,
    map: Mutex<HashMap<String, Arc<LocalInst>>>,
    watchers: Mutex<Vec<(u64, EvSink)>>,
    seq: AtomicU64,
}

impl LocalInner {
    fn tell_watchers(&self, inst: &str, ev: Ev) {
        let ws: Vec<EvSink> = self.watchers.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
        for w in ws {
            w(Tagged { backend: self.id.clone(), inst: inst.into(), ev: ev.clone() });
        }
    }
}

/// Leitet den Zustand aus pi-Ereignissen ab (spiegelt die Regeln des Hubs).
pub fn track_state(info: &mut InstanceInfo, asks: &mut std::collections::HashSet<String>, line: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return false };
    let before = (info.state, info.asks, info.last.clone());
    match v["type"].as_str().unwrap_or("") {
        "agent_start" => info.state = State::Busy,
        "agent_settled" => {
            asks.clear();
            info.state = State::Idle;
        }
        "message_end" if v["message"]["role"] == "assistant" => {
            let c = &v["message"]["content"];
            let t = c.as_str().map(String::from).or_else(|| {
                Some(c.as_array()?.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
            });
            if let Some(t) = t.map(|t| t.trim().chars().take(200).collect::<String>()).filter(|t| !t.is_empty()) {
                info.last = t;
            }
        }
        "extension_ui_request" if matches!(v["method"].as_str(), Some("select" | "confirm" | "input" | "editor")) => {
            if let Some(id) = v["id"].as_str() {
                asks.insert(id.into());
                info.state = State::Waiting;
            }
        }
        _ => {}
    }
    info.asks = asks.len();
    before != (info.state, info.asks, info.last.clone())
}

impl LocalInstances {
    pub fn new(id: &str) -> Self {
        Self { id: id.into(), inner: Arc::new(LocalInner { id: id.into(), map: Default::default(), watchers: Default::default(), seq: AtomicU64::new(1) }) }
    }

    fn get(&self, id: &str) -> Result<Arc<LocalInst>, String> {
        self.inner.map.lock().unwrap().get(id).cloned().ok_or_else(|| "unbekannte Instanz".to_string())
    }

    fn launch(&self, li: &Arc<LocalInst>) -> Result<(), String> {
        let (cwd, session) = {
            let i = li.info.lock().unwrap();
            (i.cwd.clone(), i.session.clone())
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let sink = crate::Sink::new(tx, Arc::new(|| {}));
        let args: Vec<String> = session.map(|s| vec!["--session".into(), s]).unwrap_or_default();
        li.pi.start(sink, &cwd, &args)?;
        li.info.lock().unwrap().state = State::Idle;
        let (li2, inner) = (li.clone(), self.inner.clone());
        std::thread::spawn(move || {
            let mut asks = std::collections::HashSet::new();
            while let Ok(ev) = rx.recv() {
                let (out, exit) = match ev {
                    crate::Event::Line(_, l) => (Ev::Line(l), false),
                    crate::Event::Stderr(_, l) => (Ev::Stderr(l), false),
                    crate::Event::Exit(_) => (Ev::Exit, true),
                    _ => continue,
                };
                let changed = match &out {
                    Ev::Line(l) => track_state(&mut li2.info.lock().unwrap(), &mut asks, l),
                    Ev::Exit => {
                        let mut i = li2.info.lock().unwrap();
                        i.state = State::Exited;
                        i.asks = 0;
                        true
                    }
                    _ => false,
                };
                {
                    let mut h = li2.hist.lock().unwrap();
                    h.push_back(out.clone());
                    if h.len() > LOCAL_HIST {
                        h.pop_front();
                    }
                }
                let id = li2.info.lock().unwrap().id.clone();
                let subs: Vec<EvSink> = li2.subs.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
                for s in subs {
                    s(Tagged { backend: inner.id.clone(), inst: id.clone(), ev: out.clone() });
                }
                if changed {
                    let info = li2.info.lock().unwrap().clone();
                    inner.tell_watchers(&id, Ev::Info(info));
                }
                if exit {
                    break;
                }
            }
        });
        Ok(())
    }
}

struct LocalHarness(Arc<LocalInst>);
impl Harness for LocalHarness {
    fn send(&self, cmd: &Value) -> Result<(), String> {
        self.0.pi.send(cmd)
    }
}

struct LocalFiles;
impl Files for LocalFiles {
    fn dirs(&self, path: Option<&str>) -> Result<DirList, String> {
        let start = path.filter(|p| !p.is_empty()).map(String::from).or_else(|| crate::home().map(|h| h.to_string_lossy().into())).ok_or("kein Home")?;
        let (cur, parent, locs) = crate::folders::list_folders(&start)?;
        Ok(DirList { path: cur, parent, dirs: locs.into_iter().map(|l| l.name).collect() })
    }
    fn sessions(&self) -> Result<Vec<SessionRef>, String> {
        let mut c = crate::sessions::InfoCache::default();
        Ok(c.list().into_iter().map(|s| SessionRef { path: s.path, modified_ms: s.modified * 1000 }).collect())
    }
}

impl Instances for LocalInstances {
    fn id(&self) -> &str {
        &self.id
    }
    fn label(&self) -> String {
        "Dieser Rechner".into()
    }
    fn is_remote(&self) -> bool {
        false
    }
    fn list(&self) -> Result<Vec<InstanceInfo>, String> {
        let mut v: Vec<_> = self.inner.map.lock().unwrap().values().map(|l| l.info.lock().unwrap().clone()).collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(v)
    }
    fn create(&self, spec: &Spec) -> Result<InstanceInfo, String> {
        let n = self.inner.seq.fetch_add(1, Ordering::SeqCst);
        let id = format!("l{n}");
        let name = spec.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| {
            std::path::Path::new(&spec.cwd).file_name().map(|f| f.to_string_lossy().into()).unwrap_or_else(|| id.clone())
        });
        let info = InstanceInfo { id: id.clone(), name, harness: "pi".into(), cwd: spec.cwd.clone(), state: State::Starting, asks: 0, last: String::new(), session: spec.session.clone() };
        let li = Arc::new(LocalInst { info: Mutex::new(info), pi: Default::default(), subs: Default::default(), hist: Default::default() });
        self.launch(&li)?;
        self.inner.map.lock().unwrap().insert(id.clone(), li.clone());
        let info = li.info.lock().unwrap().clone();
        self.inner.tell_watchers(&id, Ev::Info(info.clone()));
        Ok(info)
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        let li = self.inner.map.lock().unwrap().remove(id).ok_or("unbekannte Instanz")?;
        li.pi.stop();
        self.inner.tell_watchers(id, Ev::Gone);
        Ok(())
    }
    fn restart(&self, id: &str) -> Result<(), String> {
        let li = self.get(id)?;
        if li.info.lock().unwrap().state != State::Exited {
            return Err("Instanz läuft noch".into());
        }
        li.hist.lock().unwrap().clear();
        self.launch(&li)?;
        let info = li.info.lock().unwrap().clone();
        self.inner.tell_watchers(id, Ev::Info(info));
        Ok(())
    }
    fn subscribe(&self, id: &str, sink: EvSink) -> Result<Subscription, String> {
        let li = self.get(id)?;
        let stop = Arc::new(AtomicBool::new(false));
        let key = self.inner.seq.fetch_add(1, Ordering::SeqCst);
        let tag = |ev| Tagged { backend: self.id.clone(), inst: id.to_string(), ev };
        // Verlauf und Registrierung unter einer Sperre, damit nichts doppelt oder verloren geht.
        {
            let hist = li.hist.lock().unwrap();
            for ev in hist.iter() {
                sink(tag(ev.clone()));
            }
            sink(tag(Ev::Live));
            li.subs.lock().unwrap().push((key, sink));
        }
        let (li2, stop2) = (li.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
            li2.subs.lock().unwrap().retain(|(k, _)| *k != key);
        });
        Ok(Subscription { stop })
    }
    fn watch(&self, sink: EvSink) -> Result<Subscription, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let key = self.inner.seq.fetch_add(1, Ordering::SeqCst);
        sink(Tagged { backend: self.id.clone(), inst: String::new(), ev: Ev::Link(true) });
        self.inner.watchers.lock().unwrap().push((key, sink));
        let (inner, stop2) = (self.inner.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
            inner.watchers.lock().unwrap().retain(|(k, _)| *k != key);
        });
        Ok(Subscription { stop })
    }
    fn harness(&self, id: &str) -> Result<Arc<dyn Harness>, String> {
        Ok(Arc::new(LocalHarness(self.get(id)?)))
    }
    fn files(&self) -> &dyn Files {
        &LocalFiles
    }
}

impl Drop for LocalInstances {
    fn drop(&mut self) {
        for li in self.inner.map.lock().unwrap().values() {
            li.pi.stop();
        }
    }
}

// ───────────────────────────── Hub ─────────────────────────────

/// Zeilenweiser SSE-Parser: liefert (id, data) je vollständigem Ereignis.
#[derive(Default)]
pub struct SseParser {
    id: Option<String>,
    data: Vec<String>,
}

impl SseParser {
    /// Eine Zeile ohne Zeilenende. Gibt bei Leerzeile ein fertiges Ereignis zurück.
    pub fn line(&mut self, l: &str) -> Option<(Option<String>, String)> {
        if l.is_empty() {
            if self.data.is_empty() {
                self.id = None;
                return None;
            }
            return Some((self.id.take(), std::mem::take(&mut self.data).join("\n")));
        }
        if l.starts_with(':') {
            return None;
        }
        let (k, v) = l.split_once(':').unwrap_or((l, ""));
        let v = v.strip_prefix(' ').unwrap_or(v);
        match k {
            "id" => self.id = Some(v.to_string()),
            "data" => self.data.push(v.to_string()),
            _ => {}
        }
        None
    }
}

fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// Zugangstoken, das sich nachträglich setzen lässt (SSH-Backends kennen es erst nach dem Aufbau).
#[derive(Clone, Default)]
pub struct Tok(Arc<Mutex<String>>);
impl Tok {
    pub fn new(s: &str) -> Self {
        Tok(Arc::new(Mutex::new(s.into())))
    }
    pub fn get(&self) -> String {
        self.0.lock().unwrap().clone()
    }
    pub fn set(&self, s: &str) {
        *self.0.lock().unwrap() = s.into();
    }
}

pub struct HubInstances {
    id: String,
    label: String,
    base: String,
    token: Tok,
    files: HubFiles,
}

struct HubFiles {
    base: String,
    token: Tok,
}

fn http_agent(streaming: bool) -> ureq::Agent {
    let b = ureq::Agent::config_builder().max_redirects(0).timeout_connect(Some(Duration::from_secs(10)));
    // Streams laufen unbegrenzt (Heartbeat alle 20 s hält sie offen); Einzelaufrufe sind kurz.
    let b = if streaming { b.timeout_global(None) } else { b.timeout_global(Some(Duration::from_secs(30))) };
    b.build().into()
}

fn get_json(base: &str, token: &str, path: &str) -> Result<Value, String> {
    let mut r = http_agent(false).get(format!("{base}{path}")).header("authorization", format!("Bearer {token}")).call().map_err(hub_err)?;
    parse(r.body_mut().read_to_string())
}

fn call_json(method: &str, base: &str, token: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
    let url = format!("{base}{path}");
    let a = http_agent(false);
    let auth = format!("Bearer {token}");
    let res = match (method, body) {
        ("DELETE", _) => a.delete(&url).header("authorization", auth).call(),
        (_, Some(b)) => a.post(&url).header("authorization", auth).header("content-type", "application/json").send(b.to_string()),
        _ => a.post(&url).header("authorization", auth).send(""),
    };
    parse(res.map_err(hub_err)?.body_mut().read_to_string())
}

fn parse(t: Result<String, ureq::Error>) -> Result<Value, String> {
    let t = t.map_err(|e| e.to_string())?;
    serde_json::from_str(&t).map_err(|e| format!("Hub: ungültige Antwort: {e}"))
}

fn hub_err(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(401) => "Hub: Token abgelehnt (401)".into(),
        ureq::Error::StatusCode(c) => format!("Hub antwortet mit Status {c}"),
        e => format!("Hub nicht erreichbar: {e}"),
    }
}

impl HubInstances {
    /// `base`: z. B. `http://127.0.0.1:8787` (ohne Schrägstrich am Ende).
    pub fn new(id: &str, label: &str, base: &str, token: &str) -> Self {
        Self::with_tok(id, label, base, Tok::new(token))
    }

    pub fn with_tok(id: &str, label: &str, base: &str, token: Tok) -> Self {
        let base = base.trim_end_matches('/').to_string();
        Self { id: id.into(), label: label.into(), base: base.clone(), token: token.clone(), files: HubFiles { base, token } }
    }

    /// Liest einen SSE-Stream bis zum Ende; `on` bekommt (id, JSON). Fehler = Verbindung weg.
    fn read_stream(base: &str, token: &str, path: &str, last: Option<&str>, stop: &AtomicBool, mut on: impl FnMut(Option<String>, Value)) -> Result<(), String> {
        let mut req = http_agent(true).get(format!("{base}{path}")).header("authorization", format!("Bearer {token}"));
        if let Some(l) = last {
            req = req.header("last-event-id", l);
        }
        let resp = req.call().map_err(hub_err)?;
        let mut rd = BufReader::new(resp.into_body().into_reader());
        let mut p = SseParser::default();
        let mut line = String::new();
        loop {
            if stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            line.clear();
            if rd.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("Stream beendet".into());
            }
            if let Some((id, data)) = p.line(line.trim_end_matches(['\n', '\r'])) {
                if let Ok(v) = serde_json::from_str::<Value>(&data) {
                    on(id, v);
                }
            }
        }
    }

    /// Hält einen Stream mit Wiederaufbau am Leben (Backoff 0,5 s … 5 s), bis `stop` gesetzt ist.
    fn keep(&self, path: String, inst: String, sink: EvSink, stop: Arc<AtomicBool>, instance_stream: bool) {
        let (base, tok, backend) = (self.base.clone(), self.token.clone(), self.id.clone());
        std::thread::spawn(move || {
            let mut last: Option<String> = None;
            let mut delay = 500u64;
            let tag = |inst: &str, ev| Tagged { backend: backend.clone(), inst: inst.to_string(), ev };
            let mut up = false;
            while !stop.load(Ordering::SeqCst) {
                let token = tok.get();
                // Stand des Hubs vor dem Aufbau: bis dahin ist es Verlauf, danach live.
                let target = if instance_stream {
                    get_json(&base, &token, "/api/instances").ok().and_then(|l| l.as_array()?.iter().find(|i| i["id"] == inst.as_str())?["seq"].as_u64())
                } else {
                    None
                };
                let have = last.as_deref().and_then(|l| l.parse::<u64>().ok()).unwrap_or(0);
                let mut live = !instance_stream;
                if instance_stream && target.map_or(false, |t| have >= t) {
                    live = true;
                    sink(tag(&inst, Ev::Live));
                }
                let resume = last.clone();
                let r = Self::read_stream(&base, &token, &path, resume.as_deref(), &stop, |id, v| {
                    if !up {
                        up = true;
                        delay = 500;
                        sink(tag("", Ev::Link(true)));
                    }
                    if let Some(i) = id {
                        last = Some(i);
                    }
                    if instance_stream {
                        let ev = &v["ev"];
                        let seq = v["seq"].as_u64();
                        match ev["type"].as_str() {
                            Some("hub_exit") => {
                                sink(tag(&inst, Ev::Exit));
                            }
                            Some("hub_stderr") => sink(tag(&inst, Ev::Stderr(ev["text"].as_str().unwrap_or("").to_string()))),
                            Some("hub_restart") => {}
                            _ => sink(tag(&inst, Ev::Line(ev.to_string()))),
                        }
                        if !live && target.map_or(true, |t| seq.map_or(false, |s| s >= t)) {
                            live = true;
                            sink(tag(&inst, Ev::Live));
                        }
                    } else {
                        match v["type"].as_str() {
                            Some("list") => {
                                for i in v["instances"].as_array().into_iter().flatten().filter_map(InstanceInfo::from_json) {
                                    sink(tag(&i.id.clone(), Ev::Info(i)));
                                }
                            }
                            Some("inst") => {
                                if let Some(i) = InstanceInfo::from_json(&v["inst"]) {
                                    sink(tag(&i.id.clone(), Ev::Info(i)));
                                }
                            }
                            Some("gone") => sink(tag(v["id"].as_str().unwrap_or(""), Ev::Gone)),
                            _ => {}
                        }
                    }
                });
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                // Instanz ohne Ereignisse seit Aufbau: trotzdem live melden.
                if instance_stream && !live && up {
                    sink(tag(&inst, Ev::Live));
                }
                if up {
                    sink(tag("", Ev::Link(false)));
                    up = false;
                } else if r.is_err() {
                    sink(tag("", Ev::Link(false)));
                }
                let mut waited = 0;
                while waited < delay && !stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                    waited += 50;
                }
                delay = (delay * 2).min(5000);
            }
        });
    }
}

struct HubHarness {
    base: String,
    token: Tok,
    id: String,
}
impl Harness for HubHarness {
    fn send(&self, cmd: &Value) -> Result<(), String> {
        call_json("POST", &self.base, &self.token.get(), &format!("/api/instances/{}/rpc", enc(&self.id)), Some(cmd)).map(|_| ())
    }
}

impl Files for HubFiles {
    fn dirs(&self, path: Option<&str>) -> Result<DirList, String> {
        let q = path.map(|p| format!("?path={}", enc(p))).unwrap_or_default();
        let v = get_json(&self.base, &self.token.get(), &format!("/api/dirs{q}"))?;
        Ok(DirList {
            path: v["path"].as_str().unwrap_or("").into(),
            parent: v["parent"].as_str().map(String::from),
            dirs: v["dirs"].as_array().into_iter().flatten().filter_map(|d| d.as_str().map(String::from)).collect(),
        })
    }
    fn sessions(&self) -> Result<Vec<SessionRef>, String> {
        let v = get_json(&self.base, &self.token.get(), "/api/sessions")?;
        Ok(v.as_array().into_iter().flatten().filter_map(|s| Some(SessionRef { path: s["path"].as_str()?.into(), modified_ms: s["modified"].as_f64()? as u64 })).collect())
    }
}

impl Instances for HubInstances {
    fn id(&self) -> &str {
        &self.id
    }
    fn label(&self) -> String {
        self.label.clone()
    }
    fn is_remote(&self) -> bool {
        true
    }
    fn list(&self) -> Result<Vec<InstanceInfo>, String> {
        let v = get_json(&self.base, &self.token.get(), "/api/instances")?;
        Ok(v.as_array().into_iter().flatten().filter_map(InstanceInfo::from_json).collect())
    }
    fn create(&self, spec: &Spec) -> Result<InstanceInfo, String> {
        let b = json!({"name": spec.name, "harness": spec.harness, "cwd": spec.cwd, "session": spec.session});
        let v = call_json("POST", &self.base, &self.token.get(), "/api/instances", Some(&b))?;
        InstanceInfo::from_json(&v).ok_or_else(|| "Hub: ungültige Antwort".into())
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        call_json("DELETE", &self.base, &self.token.get(), &format!("/api/instances/{}", enc(id)), None).map(|_| ())
    }
    fn restart(&self, id: &str) -> Result<(), String> {
        call_json("POST", &self.base, &self.token.get(), &format!("/api/instances/{}/restart", enc(id)), None).map(|_| ())
    }
    fn subscribe(&self, id: &str, sink: EvSink) -> Result<Subscription, String> {
        let stop = Arc::new(AtomicBool::new(false));
        self.keep(format!("/api/instances/{}/events", enc(id)), id.into(), sink, stop.clone(), true);
        Ok(Subscription { stop })
    }
    fn watch(&self, sink: EvSink) -> Result<Subscription, String> {
        let stop = Arc::new(AtomicBool::new(false));
        self.keep("/api/events".into(), String::new(), sink, stop.clone(), false);
        Ok(Subscription { stop })
    }
    fn harness(&self, id: &str) -> Result<Arc<dyn Harness>, String> {
        Ok(Arc::new(HubHarness { base: self.base.clone(), token: self.token.clone(), id: id.into() }))
    }
    fn files(&self) -> &dyn Files {
        &self.files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parser_events_comments_multiline() {
        let mut p = SseParser::default();
        assert_eq!(p.line("retry: 2000"), None);
        assert_eq!(p.line(""), None);
        assert_eq!(p.line(": hb"), None);
        assert_eq!(p.line("id: 7"), None);
        assert_eq!(p.line("data: {\"a\":1}"), None);
        assert_eq!(p.line(""), Some((Some("7".into()), "{\"a\":1}".into())));
        p.line("data: a");
        p.line("data: b");
        assert_eq!(p.line(""), Some((None, "a\nb".into())));
    }

    #[test]
    fn state_machine_matches_hub_rules() {
        let mut i = InstanceInfo { id: "x".into(), name: "x".into(), harness: "pi".into(), cwd: ".".into(), state: State::Idle, asks: 0, last: String::new(), session: None };
        let mut asks = Default::default();
        assert!(track_state(&mut i, &mut asks, r#"{"type":"agent_start"}"#));
        assert_eq!(i.state, State::Busy);
        assert!(track_state(&mut i, &mut asks, r#"{"type":"extension_ui_request","id":"q1","method":"confirm"}"#));
        assert_eq!((i.state, i.asks), (State::Waiting, 1));
        // Nicht-Dialog-Methoden (notify) ändern nichts.
        assert!(!track_state(&mut i, &mut asks, r#"{"type":"extension_ui_request","id":"n","method":"notify"}"#));
        assert!(track_state(&mut i, &mut asks, r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":" fertig "}]}}"#));
        assert_eq!(i.last, "fertig");
        assert!(track_state(&mut i, &mut asks, r#"{"type":"agent_settled"}"#));
        assert_eq!((i.state, i.asks), (State::Idle, 0));
        assert!(!track_state(&mut i, &mut asks, "kein json"));
    }

    #[test]
    fn enc_escapes_reserved() {
        assert_eq!(enc("a b/ü?&"), "a%20b/%C3%BC%3F%26");
    }

    fn collect() -> (EvSink, Arc<Mutex<Vec<Tagged>>>) {
        let v = Arc::new(Mutex::new(Vec::new()));
        let v2 = v.clone();
        (Arc::new(move |t| v2.lock().unwrap().push(t)), v)
    }

    fn wait_for(v: &Arc<Mutex<Vec<Tagged>>>, f: impl Fn(&Tagged) -> bool) -> bool {
        for _ in 0..100 {
            if v.lock().unwrap().iter().any(&f) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[cfg(unix)]
    #[test]
    fn local_two_instances_independent_with_history_and_restart() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("pi-inst-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pi = dir.join("pi");
        // Fake-pi: meldet agent_start/settled pro Eingabezeile; "quit" beendet.
        std::fs::write(&pi, "#!/bin/sh\nwhile IFS= read -r l; do case \"$l\" in *quit*) exit 0;; esac; echo '{\"type\":\"agent_start\"}'; echo '{\"type\":\"agent_settled\"}'; done\n").unwrap();
        std::fs::set_permissions(&pi, std::fs::Permissions::from_mode(0o755)).unwrap();
        let old = std::env::var("PATH").unwrap_or_default();
        let _g = crate::test_support::EnvGuard::new(&[("PATH", Some(&format!("{}:{old}", dir.display())))]);

        let be = LocalInstances::new("lokal");
        let (wsink, wev) = collect();
        let _w = be.watch(wsink).unwrap();
        let a = be.create(&Spec { cwd: dir.to_string_lossy().into(), ..Default::default() }).unwrap();
        let b = be.create(&Spec { cwd: dir.to_string_lossy().into(), name: Some("zwei".into()), ..Default::default() }).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(be.list().unwrap().len(), 2);

        be.harness(&a.id).unwrap().send(&json!({"type":"prompt","message":"x"})).unwrap();
        assert!(wait_for(&wev, |t| t.inst == a.id && matches!(&t.ev, Ev::Info(i) if i.state == State::Busy)));
        // b blieb unberührt.
        assert!(!wev.lock().unwrap().iter().any(|t| t.inst == b.id && matches!(&t.ev, Ev::Info(i) if i.state == State::Busy)));

        // Später Abonnent bekommt Verlauf, dann Live.
        std::thread::sleep(Duration::from_millis(300));
        let (s, ev) = collect();
        let _sub = be.subscribe(&a.id, s).unwrap();
        let got = ev.lock().unwrap().clone();
        let live = got.iter().position(|t| t.ev == Ev::Live).expect("Live-Marker");
        assert!(got[..live].iter().any(|t| matches!(&t.ev, Ev::Line(l) if l.contains("agent_settled"))));

        // Beenden → Exited → Neustart nur dann erlaubt.
        assert!(be.restart(&a.id).is_err());
        be.harness(&a.id).unwrap().send(&json!({"type":"quit"})).unwrap();
        assert!(wait_for(&wev, |t| t.inst == a.id && matches!(&t.ev, Ev::Info(i) if i.state == State::Exited)));
        be.restart(&a.id).unwrap();
        assert_eq!(be.list().unwrap().iter().find(|i| i.id == a.id).unwrap().state, State::Idle);

        be.remove(&a.id).unwrap();
        assert!(wait_for(&wev, |t| t.inst == a.id && t.ev == Ev::Gone));
        assert!(be.harness(&a.id).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
