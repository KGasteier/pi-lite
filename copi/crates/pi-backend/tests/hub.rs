//! Integrationstest des Hub-Transports gegen einen echten `pi-hub` (Mock-Harness).
//! Übersprungen, wenn Node oder der Hub fehlen (`PI_HUB_DIR`, Standard `~/Work/pi-hub`).
use pi_backend::instances::*;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Hub(Child);
impl Drop for Hub {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(port: u16, state: &std::path::Path, root: &std::path::Path) -> Option<Hub> {
    let dir = std::env::var("PI_HUB_DIR").ok().or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/Work/pi-hub")))?;
    if !std::path::Path::new(&dir).join("server.mjs").exists() {
        return None;
    }
    let c = Command::new("node")
        .arg("server.mjs")
        .current_dir(&dir)
        .env("HUB_PORT", port.to_string())
        .env("HUB_TOKEN", "t0k")
        .env("HUB_ROOTS", root)
        .env("HUB_STATE", state)
        .env("PI_SESSIONS_DIR", root.join("sess"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let h = Hub(c);
    for _ in 0..60 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Some(h);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

fn wait(v: &Arc<Mutex<Vec<Tagged>>>, f: impl Fn(&Tagged) -> bool) -> bool {
    for _ in 0..120 {
        if v.lock().unwrap().iter().any(&f) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn hub_transport_end_to_end() {
    let tmp = std::env::temp_dir().join(format!("pi-backend-hub-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("proj")).unwrap();
    let port = 18900 + (std::process::id() % 500) as u16;
    let Some(hub) = start(port, &tmp.join("state.json"), &tmp) else {
        eprintln!("pi-hub nicht verfügbar – übersprungen");
        return;
    };
    let url = format!("http://127.0.0.1:{port}");

    // Falsches Token wird klar gemeldet.
    let bad = HubInstances::new("h", "Hub", &url, "falsch");
    assert!(bad.list().unwrap_err().contains("401"), "Token-Fehler muss sichtbar sein");

    let be = HubInstances::new("h", "Hub", &url, "t0k");
    assert!(be.is_remote());
    let root = be.files().dirs(None).unwrap();
    assert!(root.dirs.iter().any(|d| d == "proj"));
    assert!(be.files().dirs(Some("/")).is_err(), "Sandbox muss greifen");

    // Remote-Sessions: Liste mit cwd/Titel, Löschen nur im Session-Ordner.
    let sdir = tmp.join("sess").join("p");
    std::fs::create_dir_all(&sdir).unwrap();
    let sf = sdir.join("a.jsonl");
    let cwd = std::fs::canonicalize(tmp.join("proj")).unwrap().to_string_lossy().to_string();
    std::fs::write(&sf, format!("{{\"type\":\"session\",\"cwd\":\"{cwd}\"}}\n{{\"type\":\"message\",\"message\":{{\"role\":\"user\",\"content\":\"Remote-Titel\"}}}}\n")).unwrap();
    let ss = be.files().sessions().unwrap();
    assert_eq!(ss.len(), 1);
    assert_eq!((ss[0].cwd.as_str(), ss[0].title.as_str()), (cwd.as_str(), "Remote-Titel"));
    assert!(be.files().delete_session("/etc/passwd").unwrap_err().contains("403"));
    assert!(bad.files().delete_session(&ss[0].path).unwrap_err().contains("401"));
    be.files().delete_session(&ss[0].path).unwrap();
    assert!(!sf.exists() && be.files().sessions().unwrap().is_empty());

    // Remote-Session öffnen (copi/pi-lite: create mit `session`): außerhalb → 403;
    // offen in einer Instanz → Löschen 409; nach dem Entfernen der Instanz löschbar.
    let outside = Spec { harness: Some("mock".into()), cwd: cwd.clone(), session: Some("/etc/passwd".into()), ..Default::default() };
    assert!(be.create(&outside).unwrap_err().contains("403"));
    std::fs::write(&sf, "{\"type\":\"session\"}\n").unwrap();
    let sp = std::fs::canonicalize(&sf).unwrap().to_string_lossy().to_string();
    let open = be.create(&Spec { harness: Some("mock".into()), cwd: cwd.clone(), session: Some(sp.clone()), ..Default::default() }).unwrap();
    assert_eq!(open.session.as_deref(), Some(sp.as_str()));
    assert!(be.files().delete_session(&sp).unwrap_err().contains("409"));
    be.remove(&open.id).unwrap();
    be.files().delete_session(&sp).unwrap();

    let ev: Arc<Mutex<Vec<Tagged>>> = Default::default();
    let e2 = ev.clone();
    let sink: EvSink = Arc::new(move |t| e2.lock().unwrap().push(t));
    let _w = be.watch(sink.clone()).unwrap();
    let inst = be.create(&Spec { harness: Some("mock".into()), cwd: root.path.clone() + "/proj", ..Default::default() }).unwrap();
    assert!(wait(&ev, |t| t.inst == inst.id && matches!(t.ev, pi_backend::instances::Ev::Info(_))));

    let _sub = be.subscribe(&inst.id, sink.clone()).unwrap();
    assert!(wait(&ev, |t| t.inst == inst.id && t.ev == Ev::Live));
    be.harness(&inst.id).unwrap().send(&serde_json::json!({"type":"prompt","message":"Hallo"})).unwrap();
    assert!(wait(&ev, |t| t.inst == inst.id && matches!(&t.ev, Ev::Line(l) if l.contains("agent_settled"))));
    let settled = ev.lock().unwrap().iter().filter(|t| matches!(&t.ev, Ev::Line(l) if l.contains("agent_settled"))).count();
    assert_eq!(settled, 1);
    assert!(be.restart(&inst.id).is_err(), "laufende Instanz nicht neu startbar");

    // Zweites Abonnement = später Teilnehmer: bekommt Verlauf, dann Live; keine Doppelungen im ersten.
    let ev2: Arc<Mutex<Vec<Tagged>>> = Default::default();
    let e3 = ev2.clone();
    let _s2 = be.subscribe(&inst.id, Arc::new(move |t| e3.lock().unwrap().push(t))).unwrap();
    assert!(wait(&ev2, |t| t.ev == Ev::Live));
    let got = ev2.lock().unwrap().clone();
    let live = got.iter().position(|t| t.ev == Ev::Live).unwrap();
    assert!(got[..live].iter().any(|t| matches!(&t.ev, Ev::Line(l) if l.contains("agent_settled"))), "Verlauf vor Live");

    // Hub-Neustart: Registry bringt die Instanz zurück, Stream baut sich selbst wieder auf.
    drop(hub);
    assert!(wait(&ev, |t| t.ev == Ev::Link(false)), "Verbindungsabbruch wird gemeldet");
    let _hub2 = start(port, &tmp.join("state.json"), &tmp).expect("Hub-Neustart");
    assert_eq!(be.list().unwrap().len(), 1);
    assert_eq!(be.list().unwrap()[0].id, inst.id);
    ev.lock().unwrap().clear();
    assert!(wait(&ev, |t| t.ev == Ev::Link(true)), "Wiederaufbau");

    be.remove(&inst.id).unwrap();
    assert!(wait(&ev, |t| t.inst == inst.id && t.ev == Ev::Gone));
    assert!(be.list().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&tmp);
}
