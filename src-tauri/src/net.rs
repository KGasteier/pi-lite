//! Multi-Backend-Ansicht für pi-lite: hält die konfigurierten Backends (lokal + pi-hub), je einen
//! Beobachter (`watch`) und die Abos der im Raster sichtbaren Instanzen.
//!
//! Bewusst frei von Tauri: Ereignisse gehen über einen Rückruf [`Emit`] als JSON hinaus; `main.rs`
//! leitet sie als Tauri-Event `net-ev` an die Oberfläche weiter. So lässt sich der Kern ohne
//! WebKit testen. Alle Methoden können blockieren (HTTP) und gehören in einen Hintergrund-Thread.
//!
//! Ereignisform (eine Zeile pro [`Tagged`]):
//! `{"backend","inst","src":"watch"|"sub","tag"?,"kind":"line"|"stderr"|"exit"|"info"|"gone"|"live"|"link", ...}`
//! Tokens verlassen diesen Kern nie (weder in Ereignissen noch in `backends()`).
use pi_backend::backends::{self, BackendCfg};
use pi_backend::instances::{Ev, EvSink, InstanceInfo, Instances, Spec, Subscription, Tagged};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Rückruf für fertige JSON-Ereignisse (aus beliebigen Threads).
pub type Emit = Arc<dyn Fn(Value) + Send + Sync>;

struct Live {
    inst: Arc<dyn Instances>,
    _watch: Option<Subscription>,
    /// Ereignisse alter Beobachter nach Neuaufbau verwerfen.
    alive: Arc<AtomicBool>,
    link: Arc<Mutex<Option<bool>>>,
}

struct Sub {
    _sub: Subscription,
    alive: Arc<AtomicBool>,
}

pub struct Net {
    emit: Emit,
    /// `backends.json` schreiben (aus in Tests).
    persist: bool,
    cfgs: Mutex<Vec<BackendCfg>>,
    live: Mutex<HashMap<String, Live>>,
    errors: Mutex<HashMap<String, String>>,
    subs: Mutex<HashMap<(String, String), Sub>>,
    started: std::sync::Once,
}

pub fn info_json(i: &InstanceInfo) -> Value {
    json!({
        "id": i.id, "name": i.name, "harness": i.harness, "cwd": i.cwd,
        "state": serde_json::to_value(i.state).unwrap_or(Value::Null),
        "asks": i.asks, "last": i.last, "session": i.session,
    })
}

/// Ein [`Tagged`] als JSON für die Oberfläche.
pub fn tagged_json(t: &Tagged, src: &str, tag: Option<&str>) -> Value {
    let mut v = json!({"backend": t.backend, "inst": t.inst, "src": src});
    if let Some(tag) = tag {
        v["tag"] = json!(tag);
    }
    let (kind, extra) = match &t.ev {
        // pi-Zeilen bereits geparst weiterreichen; Nicht-JSON als Text.
        Ev::Line(l) => ("line", serde_json::from_str::<Value>(l).map(|e| json!({"ev": e})).unwrap_or_else(|_| json!({"text": l}))),
        Ev::Stderr(s) => ("stderr", json!({"text": s})),
        Ev::Exit => ("exit", json!({})),
        Ev::Info(i) => ("info", json!({"info": info_json(i)})),
        Ev::Gone => ("gone", json!({})),
        Ev::Live => ("live", json!({})),
        Ev::Link(up) => ("link", json!({"up": up})),
    };
    v["kind"] = json!(kind);
    if let (Value::Object(m), Value::Object(e)) = (&mut v, extra) {
        m.extend(e);
    }
    v
}

/// Kurzname für ein neues Backend aus dem Anzeigenamen; eindeutig gegenüber `taken`.
pub fn slug(label: &str, taken: &[String]) -> String {
    let mut s: String = label.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() || s == "lokal" {
        s = "hub".into();
    }
    let (base, mut n, mut out) = (s.clone(), 2, s);
    while taken.contains(&out) {
        out = format!("{base}-{n}");
        n += 1;
    }
    out
}

/// Eingabe aus dem Dialog „Backend hinzufügen/bearbeiten“.
#[derive(Debug, Default, serde::Deserialize)]
pub struct BackendForm {
    #[serde(default)]
    pub id: Option<String>,
    pub label: String,
    #[serde(default)]
    pub url: String,
    /// Leer = bisheriges Token behalten.
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub token_env: String,
    #[serde(default)]
    pub clear_token: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
}
fn yes() -> bool {
    true
}

impl Net {
    pub fn new(emit: Emit, persist: bool) -> Self {
        Self { emit, persist, cfgs: Default::default(), live: Default::default(), errors: Default::default(), subs: Default::default(), started: std::sync::Once::new() }
    }

    /// Erst beim ersten Öffnen der Rasteransicht verbinden: Der Einzel-Chat baut so keine
    /// Netzverbindungen zu Hub-Servern auf.
    pub fn ensure_started(&self) {
        self.started.call_once(|| self.start_from_config());
    }

    /// Konfiguration übernehmen und alle aktiven Backends verbinden.
    pub fn start(&self, cfgs: Vec<BackendCfg>) {
        for c in &cfgs {
            self.connect(c);
        }
        *self.cfgs.lock().unwrap() = cfgs;
    }

    /// Lädt `backends.json` (fehlt sie: nur lokal) und verbindet.
    pub fn start_from_config(&self) {
        match backends::load() {
            Ok(c) => self.start(c),
            Err(e) => {
                self.errors.lock().unwrap().insert("lokal".into(), e);
                self.start(vec![BackendCfg::local()]);
            }
        }
    }

    fn connect(&self, c: &BackendCfg) {
        self.disconnect(&c.id);
        self.errors.lock().unwrap().remove(&c.id);
        if !c.enabled {
            return;
        }
        let inst = match c.build() {
            Ok(i) => i,
            Err(e) => {
                self.errors.lock().unwrap().insert(c.id.clone(), e);
                return;
            }
        };
        let alive = Arc::new(AtomicBool::new(true));
        let link = Arc::new(Mutex::new(None));
        let (emit, a2, l2) = (self.emit.clone(), alive.clone(), link.clone());
        let sink: EvSink = Arc::new(move |t: Tagged| {
            if !a2.load(Ordering::SeqCst) {
                return;
            }
            if let Ev::Link(up) = t.ev {
                *l2.lock().unwrap() = Some(up);
            }
            emit(tagged_json(&t, "watch", None));
        });
        let watch = match inst.watch(sink) {
            Ok(w) => Some(w),
            Err(e) => {
                self.errors.lock().unwrap().insert(c.id.clone(), e);
                None
            }
        };
        self.live.lock().unwrap().insert(c.id.clone(), Live { inst, _watch: watch, alive, link });
    }

    fn disconnect(&self, id: &str) {
        self.subs.lock().unwrap().retain(|(b, _), s| {
            if b == id {
                s.alive.store(false, Ordering::SeqCst);
            }
            b != id
        });
        // Außerhalb der Sperre fallen lassen: LocalInstances beendet dabei seine Prozesse.
        let old = self.live.lock().unwrap().remove(id);
        if let Some(l) = old {
            l.alive.store(false, Ordering::SeqCst);
        }
    }

    /// Alles trennen (App-Ende): lokale pi-Prozesse werden beendet.
    pub fn shutdown(&self) {
        let ids: Vec<String> = self.live.lock().unwrap().keys().cloned().collect();
        for id in ids {
            self.disconnect(&id);
        }
    }

    fn get(&self, backend: &str) -> Result<Arc<dyn Instances>, String> {
        if let Some(l) = self.live.lock().unwrap().get(backend) {
            return Ok(l.inst.clone());
        }
        Err(self.errors.lock().unwrap().get(backend).cloned().unwrap_or_else(|| format!("Backend „{backend}“ ist nicht verbunden")))
    }

    /// Liste für die Leiste – ohne Tokens.
    pub fn backends(&self) -> Value {
        let cfgs = self.cfgs.lock().unwrap().clone();
        let live = self.live.lock().unwrap();
        let errors = self.errors.lock().unwrap();
        Value::Array(
            cfgs.iter()
                .map(|c| {
                    let l = live.get(&c.id);
                    json!({
                        "id": c.id, "label": c.label, "kind": c.kind, "url": c.url,
                        "remote": l.map(|l| l.inst.is_remote()).unwrap_or(c.kind != "local"),
                        "enabled": c.enabled, "connected": l.is_some(),
                        "link": l.and_then(|l| *l.link.lock().unwrap()),
                        "error": errors.get(&c.id),
                        "token_set": !c.token.is_empty(), "token_env": c.token_env,
                    })
                })
                .collect(),
        )
    }

    /// Backend anlegen oder ändern; verbindet neu, wenn sich Verbindungsdaten ändern.
    pub fn save_backend(&self, f: BackendForm) -> Result<Value, String> {
        let label = f.label.trim().to_string();
        if label.is_empty() {
            return Err("Name fehlt".into());
        }
        let mut cfgs = self.cfgs.lock().unwrap().clone();
        let ids: Vec<String> = cfgs.iter().map(|c| c.id.clone()).collect();
        let idx = f.id.as_ref().and_then(|id| cfgs.iter().position(|c| &c.id == id));
        let old = idx.map(|i| cfgs[i].clone());
        let mut c = match &old {
            Some(o) => o.clone(),
            None => BackendCfg::hub(&slug(&label, &ids), "", "", ""),
        };
        c.label = label;
        c.enabled = f.enabled;
        if c.kind != "local" {
            let url = f.url.trim().trim_end_matches('/').to_string();
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("URL muss mit http:// oder https:// beginnen".into());
            }
            c.url = url;
            c.token_env = f.token_env.trim().to_string();
            if f.clear_token {
                c.token.clear();
            }
            if !f.token.trim().is_empty() {
                c.token = f.token.trim().to_string();
            }
        }
        // Probebau vor dem Speichern, damit keine kaputte Konfiguration auf Platte landet.
        c.build()?;
        match idx {
            Some(i) => cfgs[i] = c.clone(),
            None => cfgs.push(c.clone()),
        }
        if self.persist {
            backends::save(&cfgs)?;
        }
        *self.cfgs.lock().unwrap() = cfgs;
        let reconnect = match &old {
            None => true,
            // Lokal nur bei An/Aus neu aufbauen – sonst stürben laufende lokale Instanzen.
            Some(o) if o.kind == "local" => o.enabled != c.enabled,
            Some(o) => (&o.url, &o.token, &o.token_env, o.enabled) != (&c.url, &c.token, &c.token_env, c.enabled),
        };
        if reconnect {
            self.connect(&c);
        }
        Ok(json!({"id": c.id, "reconnected": reconnect}))
    }

    /// Hub-Backend entfernen (das lokale bleibt immer).
    pub fn delete_backend(&self, id: &str) -> Result<(), String> {
        let mut cfgs = self.cfgs.lock().unwrap().clone();
        let i = cfgs.iter().position(|c| c.id == id).ok_or("unbekanntes Backend")?;
        if cfgs[i].kind == "local" {
            return Err("Das lokale Backend lässt sich nur abschalten".into());
        }
        cfgs.remove(i);
        if self.persist {
            backends::save(&cfgs)?;
        }
        *self.cfgs.lock().unwrap() = cfgs;
        self.disconnect(id);
        Ok(())
    }

    pub fn list(&self, backend: &str) -> Result<Value, String> {
        Ok(Value::Array(self.get(backend)?.list()?.iter().map(info_json).collect()))
    }

    pub fn create(&self, backend: &str, spec: &Value) -> Result<Value, String> {
        let s = |k: &str| spec[k].as_str().map(str::trim).filter(|v| !v.is_empty()).map(String::from);
        let spec = Spec { name: s("name"), harness: s("harness"), cwd: s("cwd").unwrap_or_default(), session: s("session") };
        Ok(info_json(&self.get(backend)?.create(&spec)?))
    }

    pub fn remove(&self, backend: &str, inst: &str) -> Result<(), String> {
        self.unsubscribe(backend, inst);
        self.get(backend)?.remove(inst)
    }

    pub fn restart(&self, backend: &str, inst: &str) -> Result<(), String> {
        self.get(backend)?.restart(inst)
    }

    pub fn send(&self, backend: &str, inst: &str, cmd: &Value) -> Result<(), String> {
        if !cmd["type"].is_string() {
            return Err("Befehl ohne type".into());
        }
        self.get(backend)?.harness(inst)?.send(cmd)
    }

    pub fn dirs(&self, backend: &str, path: Option<&str>) -> Result<Value, String> {
        let d = self.get(backend)?.files().dirs(path.filter(|p| !p.is_empty()))?;
        Ok(json!({"path": d.path, "parent": d.parent, "dirs": d.dirs}))
    }

    /// Verlauf + Live-Ereignisse einer Instanz; ersetzt ein bestehendes Abo derselben Instanz.
    /// `tag` stammt von der Oberfläche und steht in jedem Ereignis (alte Abos bleiben so erkennbar).
    pub fn subscribe(&self, backend: &str, inst: &str, tag: &str) -> Result<(), String> {
        let b = self.get(backend)?;
        self.unsubscribe(backend, inst);
        let alive = Arc::new(AtomicBool::new(true));
        let (emit, a2, tag) = (self.emit.clone(), alive.clone(), tag.to_string());
        let sink: EvSink = Arc::new(move |t: Tagged| {
            // Verbindungsstatus kommt aus dem Beobachter; hier nur Instanzereignisse.
            if a2.load(Ordering::SeqCst) && !matches!(t.ev, Ev::Link(_)) {
                emit(tagged_json(&t, "sub", Some(&tag)));
            }
        });
        let sub = b.subscribe(inst, sink)?;
        self.subs.lock().unwrap().insert((backend.into(), inst.into()), Sub { _sub: sub, alive });
        Ok(())
    }

    pub fn unsubscribe(&self, backend: &str, inst: &str) {
        let old = self.subs.lock().unwrap().remove(&(backend.to_string(), inst.to_string()));
        if let Some(s) = old {
            s.alive.store(false, Ordering::SeqCst);
        }
    }

    #[cfg(test)]
    fn sub_count(&self) -> usize {
        self.subs.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_backend::instances::State;

    fn collect() -> (Emit, Arc<Mutex<Vec<Value>>>) {
        let v = Arc::new(Mutex::new(Vec::new()));
        let v2 = v.clone();
        (Arc::new(move |e| v2.lock().unwrap().push(e)), v)
    }

    #[test]
    fn tagged_json_shapes() {
        let t = |ev| Tagged { backend: "b".into(), inst: "i".into(), ev };
        let v = tagged_json(&t(Ev::Line(r#"{"type":"agent_start"}"#.into())), "sub", Some("7"));
        assert_eq!(v, json!({"backend":"b","inst":"i","src":"sub","tag":"7","kind":"line","ev":{"type":"agent_start"}}));
        assert_eq!(tagged_json(&t(Ev::Line("kein json".into())), "sub", None)["text"], "kein json");
        assert_eq!(tagged_json(&t(Ev::Link(false)), "watch", None)["up"], false);
        let info = InstanceInfo { id: "i".into(), name: "n".into(), harness: "mock".into(), cwd: "/x".into(), state: State::Waiting, asks: 2, last: "l".into(), session: None };
        let v = tagged_json(&t(Ev::Info(info)), "watch", None);
        assert_eq!((v["kind"].as_str(), v["info"]["state"].as_str(), v["info"]["asks"].as_u64()), (Some("info"), Some("waiting"), Some(2)));
        assert!(v.get("tag").is_none());
    }

    #[test]
    fn slug_unique_and_never_lokal() {
        let taken = vec!["lokal".to_string(), "mein-server".to_string()];
        assert_eq!(slug("Mein Server!", &taken), "mein-server-2");
        assert_eq!(slug("Lokal", &taken), "hub");
        assert_eq!(slug("ÄÖ", &taken), "hub");
    }

    #[test]
    fn backends_never_expose_token_and_keep_it_on_empty_edit() {
        let (e, _) = collect();
        let net = Net::new(e, false);
        net.start(vec![BackendCfg::local(), BackendCfg::hub("s", "Server", "http://127.0.0.1:9", "geheim-123")]);
        let txt = net.backends().to_string();
        assert!(!txt.contains("geheim-123"), "Token in der Liste: {txt}");
        assert_eq!(net.backends()[1]["token_set"], true);
        // Bearbeiten ohne Token behält das alte, URL-Änderung verbindet neu.
        let r = net.save_backend(BackendForm { id: Some("s".into()), label: "Server".into(), url: "http://127.0.0.1:10/".into(), enabled: true, ..Default::default() }).unwrap();
        assert_eq!(r["reconnected"], true);
        assert_eq!(net.cfgs.lock().unwrap()[1].token, "geheim-123");
        assert_eq!(net.cfgs.lock().unwrap()[1].url, "http://127.0.0.1:10");
        // Nur Name geändert: keine Neuverbindung.
        let r = net.save_backend(BackendForm { id: Some("s".into()), label: "Neu".into(), url: "http://127.0.0.1:10".into(), enabled: true, ..Default::default() }).unwrap();
        assert_eq!(r["reconnected"], false);
        assert!(net.save_backend(BackendForm { label: "X".into(), url: "ftp://x".into(), ..Default::default() }).is_err());
        assert!(net.save_backend(BackendForm { label: " ".into(), url: "http://x".into(), ..Default::default() }).is_err());
        let r = net.save_backend(BackendForm { label: "Zweiter".into(), url: "http://h:1".into(), token: "t2".into(), enabled: true, ..Default::default() }).unwrap();
        assert_eq!(r["id"], "zweiter");
        assert!(net.delete_backend("lokal").is_err());
        net.delete_backend("zweiter").unwrap();
        assert!(net.list("zweiter").is_err());
        net.shutdown();
    }

    #[test]
    fn local_edit_does_not_kill_and_disabled_reports_error() {
        let (e, ev) = collect();
        let net = Net::new(e, false);
        net.start(vec![BackendCfg::local()]);
        // Beobachter meldet Link(true) sofort (lokal).
        assert!(ev.lock().unwrap().iter().any(|v| v["kind"] == "link" && v["up"] == true && v["src"] == "watch"));
        assert_eq!(net.backends()[0]["link"], true);
        let r = net.save_backend(BackendForm { id: Some("lokal".into()), label: "Hier".into(), enabled: true, ..Default::default() }).unwrap();
        assert_eq!(r["reconnected"], false);
        net.save_backend(BackendForm { id: Some("lokal".into()), label: "Hier".into(), enabled: false, ..Default::default() }).unwrap();
        assert!(net.list("lokal").unwrap_err().contains("nicht verbunden"));
        assert!(net.send("lokal", "x", &json!({})).is_err());
    }

    /// Live gegen einen laufenden Mock-Hub: `NET_TEST_HUB=http://127.0.0.1:PORT NET_TEST_TOKEN=… cargo test -- --ignored`.
    /// Schreibt die empfangenen Ereignisse optional nach `NET_TEST_DUMP` (für UI-Wiedergabe).
    #[test]
    #[ignore]
    fn live_mock_hub_roundtrip() {
        let url = std::env::var("NET_TEST_HUB").expect("NET_TEST_HUB");
        let token = std::env::var("NET_TEST_TOKEN").unwrap_or_default();
        let cwd = std::env::var("NET_TEST_CWD").unwrap_or_default();
        let (e, ev) = collect();
        let net = Net::new(e, false);
        net.start(vec![BackendCfg::local(), BackendCfg::hub("hub", "Mock-Hub", &url, &token)]);
        let wait = |f: &dyn Fn(&Value) -> bool| {
            for _ in 0..200 {
                if ev.lock().unwrap().iter().any(f) {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            false
        };
        assert!(wait(&|v| v["backend"] == "hub" && v["kind"] == "link" && v["up"] == true), "kein Link");
        assert_eq!(net.backends()[1]["link"], true);
        let d = net.dirs("hub", None).unwrap();
        assert!(d["path"].is_string());
        let a = net.create("hub", &json!({"name":"eins","harness":"mock","cwd":cwd})).unwrap();
        let b = net.create("hub", &json!({"name":"zwei","harness":"mock","cwd":cwd})).unwrap();
        let (ia, ib) = (a["id"].as_str().unwrap().to_string(), b["id"].as_str().unwrap().to_string());
        net.subscribe("hub", &ia, "A1").unwrap();
        net.subscribe("hub", &ib, "B1").unwrap();
        assert!(wait(&|v| v["tag"] == "A1" && v["kind"] == "live"));
        net.send("hub", &ia, &json!({"type":"prompt","message":"Hallo Raster"})).unwrap();
        net.send("hub", &ib, &json!({"type":"prompt","message":"Bitte lösch build"})).unwrap();
        assert!(wait(&|v| v["tag"] == "A1" && v["ev"]["type"] == "agent_settled"), "A nicht fertig");
        assert!(wait(&|v| v["tag"] == "B1" && v["ev"]["type"] == "extension_ui_request"), "B keine Rückfrage");
        assert!(wait(&|v| v["src"] == "watch" && v["inst"] == ib.as_str() && v["info"]["state"] == "waiting" && v["info"]["asks"] == 1));
        let ask = ev.lock().unwrap().iter().find(|v| v["tag"] == "B1" && v["ev"]["type"] == "extension_ui_request").unwrap()["ev"]["id"].clone();
        // Neues Abo ersetzt das alte; alte Kennung liefert danach nichts mehr.
        net.subscribe("hub", &ib, "B2").unwrap();
        assert!(wait(&|v| v["tag"] == "B2" && v["kind"] == "live"));
        let n_b1 = ev.lock().unwrap().iter().filter(|v| v["tag"] == "B1").count();
        net.send("hub", &ib, &json!({"type":"extension_ui_response","id":ask,"confirmed":true})).unwrap();
        assert!(wait(&|v| v["tag"] == "B2" && v["ev"]["type"] == "agent_settled"), "B nicht fertig");
        assert_eq!(ev.lock().unwrap().iter().filter(|v| v["tag"] == "B1").count(), n_b1, "altes Abo liefert noch");
        assert_eq!(net.sub_count(), 2);
        let l = net.list("hub").unwrap();
        assert!(l.as_array().unwrap().iter().any(|i| i["id"] == ia.as_str() && i["last"].as_str().unwrap_or("").contains("Hallo Raster")));
        if let Ok(p) = std::env::var("NET_TEST_DUMP") {
            let lines: Vec<String> = ev.lock().unwrap().iter().map(|v| v.to_string()).collect();
            std::fs::write(p, lines.join("\n") + "\n").unwrap();
        }
        // Token taucht in keinem Ereignis auf.
        if !token.is_empty() {
            assert!(!ev.lock().unwrap().iter().any(|v| v.to_string().contains(&token)));
        }
        net.remove("hub", &ia).unwrap();
        assert!(wait(&|v| v["kind"] == "gone" && v["inst"] == ia.as_str()));
        assert_eq!(net.sub_count(), 1);
        net.remove("hub", &ib).unwrap();
        net.shutdown();
    }
}
