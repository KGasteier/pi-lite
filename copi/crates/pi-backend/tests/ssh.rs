//! SSH-Backend Ende-zu-Ende mit einem Fake-`ssh` (führt den Befehl lokal aus, `-L` über socat).
//! Übersprungen ohne node/socat. Kein echter Server, keine Modellkosten (Mock-Harness).
#![cfg(unix)]
use pi_backend::backends::BackendCfg;
use pi_backend::instances::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

static ENV: Mutex<()> = Mutex::new(());

fn have(bin: &str) -> bool {
    std::process::Command::new("sh").args(["-c", &format!("command -v {bin}")]).output().map(|o| o.status.success()).unwrap_or(false)
}

const FAKE: &str = r#"#!/bin/sh
# Fake-ssh: -L lokal:127.0.0.1:port weiterleiten (-N), sonst letzten Parameter lokal ausführen.
fwd=""; n=0
for a in "$@"; do
  if [ "$prev" = "-L" ]; then fwd="$a"; fi
  prev="$a"; last="$a"
done
case " $* " in *" -N "*)
  lp=$(echo "$fwd" | cut -d: -f2); rp=$(echo "$fwd" | cut -d: -f4)
  exec socat TCP-LISTEN:$lp,bind=127.0.0.1,fork,reuseaddr TCP:127.0.0.1:$rp ;;
esac
exec sh -c "$last"
"#;

#[test]
fn ssh_backend_bootstraps_hub_and_talks_through_tunnel() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    if !have("node") || !have("socat") {
        eprintln!("node/socat fehlt – übersprungen");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("pi-backend-ssh-{}", std::process::id()));
    let home = tmp.join("home");
    std::fs::create_dir_all(home.join("proj")).unwrap();
    let fake = tmp.join("ssh");
    std::fs::write(&fake, FAKE).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::env::set_var("COPI_SSH", &fake);
    std::env::set_var("HOME", &home);
    std::env::remove_var("XDG_CONFIG_HOME");
    std::env::remove_var("HUB_TOKEN");
    let port = 19400 + (std::process::id() % 300) as u16;
    let mut cfg = BackendCfg::ssh("s", "Server", "example.invalid", "tester");
    cfg.hub_port = port;

    let be = cfg.build().unwrap();
    let root = be.files().dirs(None);
    // Erst nach list() ist das Token da (Einrichtung abgeschlossen).
    let listed = be.list();
    let pidfile = home.join(".local/share/pi-hub/pid");
    let cleanup = || {
        if let Ok(p) = std::fs::read_to_string(&pidfile) {
            let _ = std::process::Command::new("kill").arg(p.trim()).status();
        }
    };
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = &root;
        assert_eq!(listed.as_ref().map(|l| l.len()).unwrap_or(99), 0, "{listed:?}");
        let dirs = be.files().dirs(None).unwrap();
        assert!(dirs.dirs.iter().any(|d| d == "proj"), "{dirs:?}");
        let ev: Arc<Mutex<Vec<Tagged>>> = Default::default();
        let e2 = ev.clone();
        let sink: EvSink = Arc::new(move |t| e2.lock().unwrap().push(t));
        let inst = be.create(&Spec { harness: Some("mock".into()), cwd: format!("{}/proj", dirs.path), ..Default::default() }).unwrap();
        let _s = be.subscribe(&inst.id, sink).unwrap();
        for _ in 0..100 {
            if ev.lock().unwrap().iter().any(|t| t.ev == Ev::Live) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        be.harness(&inst.id).unwrap().send(&serde_json::json!({"type":"prompt","message":"Hallo"})).unwrap();
        for _ in 0..100 {
            if ev.lock().unwrap().iter().any(|t| matches!(&t.ev, Ev::Line(l) if l.contains("agent_settled"))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(ev.lock().unwrap().iter().any(|t| matches!(&t.ev, Ev::Line(l) if l.contains("agent_settled"))));
        be.remove(&inst.id).unwrap();
    }));
    drop(be);
    cleanup();
    let _ = std::fs::remove_dir_all(&tmp);
    if let Err(e) = res {
        std::panic::resume_unwind(e);
    }
}

#[test]
fn ssh_failure_is_reported_not_hung() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    // Fake-ssh, das sofort mit Permission denied scheitert.
    let tmp = std::env::temp_dir().join(format!("pi-backend-sshfail-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let fake = tmp.join("ssh");
    std::fs::write(&fake, "#!/bin/sh\necho 'x: Permission denied (publickey).' >&2\nexit 255\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("COPI_SSH", &fake);
    let be = BackendCfg::ssh("s", "S", "h.invalid", "u").build().unwrap();
    let e = be.list().unwrap_err();
    assert!(e.contains("Permission denied") && e.contains("ssh-copy-id"), "{e}");
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Live gegen einen echten Server: `SSH_LIVE=medienhaus@host cargo test -p pi-backend --test ssh -- --ignored live --nocapture`.
/// Nutzt nur den Mock-Harness (keine Modellkosten) und beendet den Hub danach per PID.
#[test]
#[ignore]
fn live_server_bootstrap_sessions_and_mock_chat() {
    let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(t) = std::env::var("SSH_LIVE") else { return };
    let (user, host) = t.split_once('@').unwrap_or(("", &t));
    let mut cfg = BackendCfg::ssh("live", "Live", host, user);
    cfg.hub_port = 19500 + (std::process::id() % 300) as u16;
    let be = cfg.build().unwrap();
    let listed = be.list();
    eprintln!("list: {listed:?}");
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        listed.as_ref().unwrap();
        let dirs = be.files().dirs(None).unwrap();
        eprintln!("Wurzel: {} ({} Ordner)", dirs.path, dirs.dirs.len());
        let sessions = be.files().sessions().unwrap();
        eprintln!("Sessions: {}", sessions.len());
        let ev: Arc<Mutex<Vec<Tagged>>> = Default::default();
        let e2 = ev.clone();
        let sink: EvSink = Arc::new(move |t| e2.lock().unwrap().push(t));
        let inst = be.create(&Spec { harness: Some("mock".into()), cwd: dirs.path.clone(), ..Default::default() }).unwrap();
        let _s = be.subscribe(&inst.id, sink).unwrap();
        be.harness(&inst.id).unwrap().send(&serde_json::json!({"type":"prompt","message":"Hallo"})).unwrap();
        for _ in 0..200 {
            if ev.lock().unwrap().iter().any(|t| matches!(&t.ev, Ev::Line(l) if l.contains("agent_settled"))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(ev.lock().unwrap().iter().any(|t| matches!(&t.ev, Ev::Line(l) if l.contains("agent_settled"))));
        be.remove(&inst.id).unwrap();

        // Echtes pi (RPC-Start + get_state, KEIN Prompt → keine Modellkosten, keine vorhandene Session).
        let ev2: Arc<Mutex<Vec<Tagged>>> = Default::default();
        let e3 = ev2.clone();
        let sink2: EvSink = Arc::new(move |t| e3.lock().unwrap().push(t));
        let pi = be.create(&Spec { harness: Some("pi".into()), cwd: dirs.path.clone(), ..Default::default() }).unwrap();
        let _s2 = be.subscribe(&pi.id, sink2).unwrap();
        be.harness(&pi.id).unwrap().send(&serde_json::json!({"id":"live","type":"get_state"})).unwrap();
        let got = |ev: &Arc<Mutex<Vec<Tagged>>>| ev.lock().unwrap().iter().any(|t| matches!(&t.ev, Ev::Line(l) if l.contains("\"get_state\"") && l.contains("\"success\":true")));
        for _ in 0..300 {
            if got(&ev2) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let info = be.list().unwrap().into_iter().find(|i| i.id == pi.id);
        eprintln!("pi get_state: {} · Instanz {:?}", got(&ev2), info.map(|i| i.state));
        assert!(got(&ev2), "echtes pi antwortet nicht auf get_state");
        be.remove(&pi.id).unwrap();
    }));
    drop(be);
    // Hub auf dem Server per PID beenden (nur unseren, über die pid-Datei).
    let _ = std::process::Command::new("ssh").args(["-o", "BatchMode=yes", &t, "kill $(cat ~/.local/share/pi-hub/pid) 2>/dev/null; true"]).status();
    if let Err(e) = res {
        std::panic::resume_unwind(e);
    }
}
