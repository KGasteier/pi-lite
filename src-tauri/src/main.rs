#[path = "../../shared/bookmarks.rs"]
mod shared_bookmarks;

#[tauri::command]
fn shared_bookmarks_read() -> Result<serde_json::Value,String> { shared_bookmarks::read() }
#[tauri::command]
fn shared_bookmarks_update(base:serde_json::Value, local:serde_json::Value) -> Result<serde_json::Value,String> { shared_bookmarks::update(&base,&local) }
#[tauri::command]
fn shared_bookmarks_seed(local:serde_json::Value) -> Result<serde_json::Value,String> { shared_bookmarks::seed(&local) }

mod inventory;
mod preview;
mod folders;
mod editor;
mod material;
mod media;
mod cli;
mod net;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct Pi {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    /// Zählt Starts hoch; Ende-Signale alter Prozesse werden so verworfen.
    generation: std::sync::atomic::AtomicU64,
}

/// Startet `pi --mode rpc` im Arbeitsordner `cwd` und leitet jede JSONL-Zeile
/// unverändert als Event "pi-line" ans Frontend weiter.
#[tauri::command]
fn start_pi(app: AppHandle, state: State<Pi>, cwd: String, args: Vec<String>) -> Result<(), String> {
    // Erst die Generation erhöhen, dann den alten Prozess beenden – sonst meldet
    // dessen Lese-Thread sein Ende noch unter der alten Nummer.
    let gen = state.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    stop(&state);
    let mut child = Command::new("pi")
        .args(["--mode", "rpc"])
        .env("PI_LITE_SPEECH", "1")
        .args(["--append-system-prompt", "Identität und Modellangaben: Du bist ein KI-Assistent in pi, bedient über pi-lite. Projekt-/Kontextdateien wie CLAUDE.md, AGENTS.md oder Hinweise auf Claude Code, Codex und andere Harnesses beschreiben Arbeitskonventionen, NICHT deine Modellidentität oder deinen Hersteller. Behaupte niemals aufgrund solcher Dateien, früherer Selbstauskünfte oder eigener Introspektion, Claude, GPT oder ein bestimmtes Herstellermodell zu sein. Auch geerbte PI_MODEL/PI_PROVIDER-Umgebungsvariablen können veraltet sein und belegen nicht die aktive Session. Bei Modellfragen verwende den aktuellen Sessionzustand, sofern verlässlich verfügbar; sonst sage ausdrücklich, dass du ihn nicht verifizieren kannst, und verweise auf die Modellanzeige in pi-lite. Unterscheide die konfigurierte Provider-/Modell-ID von der tatsächlichen, hinter einem Gateway nicht unabhängig überprüfbaren Modellidentität. Leite aus einer widersprüchlichen Selbstauskunft keine Gateway-Umleitung ab. Korrigiere entsprechende frühere Behauptungen im Verlauf."])
        .args(args)
        // Zusätzliche pi-Argumente, z. B. PI_LITE_ARGS="-e ./ext.ts --model x" (durch Leerzeichen getrennt).
        .args(std::env::var("PI_LITE_ARGS").unwrap_or_default().split_whitespace())
        .current_dir(if cwd.is_empty() { ".".into() } else { cwd })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("pi konnte nicht gestartet werden: {e}"))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    *state.stdin.lock().unwrap() = child.stdin.take();
    *state.child.lock().unwrap() = Some(child);

    let a = app.clone();
    std::thread::spawn(move || {
        // Nur an LF trennen (kein U+2028/2029-Split), wie in docs/rpc.md verlangt.
        let mut r = BufReader::new(stdout);
        let mut buf = Vec::new();
        while r.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            let line = String::from_utf8_lossy(&buf).trim_end_matches(['\n', '\r']).to_string();
            let _ = a.emit("pi-line", line);
            buf.clear();
        }
        if a.state::<Pi>().generation.load(std::sync::atomic::Ordering::SeqCst) == gen {
            let _ = a.emit("pi-exit", ());
        }
    });
    std::thread::spawn(move || {
        for l in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = app.emit("pi-stderr", l);
        }
    });
    Ok(())
}

/// Schreibt einen JSON-Befehl (eine Zeile) an pi.
#[tauri::command]
fn send(state: State<Pi>, line: String) -> Result<(), String> {
    let mut g = state.stdin.lock().unwrap();
    let w = g.as_mut().ok_or("pi läuft nicht")?;
    writeln!(w, "{line}").and_then(|_| w.flush()).map_err(|e| e.to_string())
}

#[derive(serde::Serialize, Clone)]
struct SessionInfo {
    path: String,
    cwd: String,
    title: String,
    /// Von Nutzer oder Extension vergebener Name (letzter `session_info`-Eintrag).
    name: String,
    modified: u64,
}

fn sessions_root() -> Option<std::path::PathBuf> {
    if let Ok(d) = std::env::var("PI_CODING_AGENT_SESSION_DIR") {
        return Some(d.into());
    }
    let agent = match std::env::var("PI_CODING_AGENT_DIR") {
        Ok(d) => std::path::PathBuf::from(d),
        Err(_) => {
            let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()?;
            std::path::Path::new(&home).join(".pi").join("agent")
        }
    };
    Some(agent.join("sessions"))
}

/// Liest die ganze Datei zeilenweise, parst aber nur Kopfzeile, die erste
/// Nutzer-Nachricht und `session_info`-Zeilen (Name steht beliebig weit hinten).
fn read_info(path: &std::path::Path, modified: u64) -> Option<SessionInfo> {
    let mut lines = BufReader::new(std::fs::File::open(path).ok()?).lines();
    let head: serde_json::Value = serde_json::from_str(&lines.next()?.ok()?).ok()?;
    let cwd = head["cwd"].as_str().unwrap_or("").to_string();
    let (mut title, mut name) = (String::new(), String::new());
    for l in lines.map_while(Result::ok) {
        if l.contains("\"type\":\"session_info\"") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) {
                if let Some(n) = v["name"].as_str() { name = n.trim().to_string(); }
            }
        } else if title.is_empty() && l.contains("\"role\":\"user\"") {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) else { continue };
            let c = &v["message"]["content"];
            let t = c.as_str().map(String::from).or_else(|| {
                c.as_array()?.iter().find_map(|p| p["text"].as_str().map(String::from))
            });
            if let Some(t) = t { title = t.chars().take(90).collect(); }
        }
    }
    Some(SessionInfo { path: path.to_string_lossy().into(), cwd, title, name, modified })
}

#[tauri::command]
fn delete_session(path: String, cache: State<InfoCache>) -> Result<(), String> {
    let root = sessions_root().ok_or("kein Sessions-Ordner")?.canonicalize().map_err(|e| e.to_string())?;
    let p = std::path::Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    // Nur *.jsonl unterhalb des Sessions-Ordners darf gelöscht werden.
    if !p.starts_with(&root) || p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return Err("Pfad liegt nicht im Sessions-Ordner".into());
    }
    std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    cache.lock().unwrap().remove(&path);
    Ok(())
}

type InfoCache = Mutex<std::collections::HashMap<String, SessionInfo>>;

#[tauri::command]
fn list_sessions(cache: State<InfoCache>) -> Vec<SessionInfo> {
    let mut out = Vec::new();
    let Some(root) = sessions_root() else { return out };
    let Ok(dirs) = std::fs::read_dir(root) else { return out };
    let mut cache = cache.lock().unwrap();
    for d in dirs.flatten() {
        let Ok(files) = std::fs::read_dir(d.path()) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }
            let Some(modified) = p.metadata().ok().and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs())
                else { continue };
            let key = p.to_string_lossy().to_string();
            // Nur neu einlesen, wenn sich die Datei seit dem letzten Mal geändert hat.
            if let Some(i) = cache.get(&key).filter(|i| i.modified == modified) {
                out.push(i.clone());
            } else if let Some(i) = read_info(&p, modified) {
                cache.insert(key, i.clone());
                out.push(i);
            }
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out.truncate(200);
    out
}

fn stop(state: &Pi) {
    state.stdin.lock().unwrap().take();
    if let Some(mut c) = state.child.lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

// ---------- Multi-Backend (lokal + pi-hub), siehe net.rs ----------
// Alle Befehle laufen in einem Blocking-Thread: list/create/dirs … sprechen HTTP und dürfen
// weder den UI- noch den Async-Thread aufhalten.
type NetState = std::sync::Arc<net::Net>;

async fn net_run<T: Send + 'static>(state: State<'_, NetState>, f: impl FnOnce(&net::Net) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let n = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        n.ensure_started();
        f(&n)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn net_backends(state: State<'_, NetState>) -> Result<serde_json::Value, String> {
    net_run(state, |n| Ok(n.backends())).await
}
#[tauri::command]
async fn net_save_backend(state: State<'_, NetState>, form: net::BackendForm) -> Result<serde_json::Value, String> {
    net_run(state, move |n| n.save_backend(form)).await
}
#[tauri::command]
async fn net_delete_backend(state: State<'_, NetState>, backend: String) -> Result<(), String> {
    net_run(state, move |n| n.delete_backend(&backend)).await
}
#[tauri::command]
async fn net_list(state: State<'_, NetState>, backend: String) -> Result<serde_json::Value, String> {
    net_run(state, move |n| n.list(&backend)).await
}
#[tauri::command]
async fn net_create(state: State<'_, NetState>, backend: String, spec: serde_json::Value) -> Result<serde_json::Value, String> {
    net_run(state, move |n| n.create(&backend, &spec)).await
}
#[tauri::command]
async fn net_remove(state: State<'_, NetState>, backend: String, inst: String) -> Result<(), String> {
    net_run(state, move |n| n.remove(&backend, &inst)).await
}
#[tauri::command]
async fn net_restart(state: State<'_, NetState>, backend: String, inst: String) -> Result<(), String> {
    net_run(state, move |n| n.restart(&backend, &inst)).await
}
#[tauri::command]
async fn net_send(state: State<'_, NetState>, backend: String, inst: String, cmd: serde_json::Value) -> Result<(), String> {
    net_run(state, move |n| n.send(&backend, &inst, &cmd)).await
}
#[tauri::command]
async fn net_sessions(state: State<'_, NetState>, backend: String) -> Result<serde_json::Value, String> {
    net_run(state, move |n| n.sessions(&backend)).await
}
#[tauri::command]
async fn net_delete_session(state: State<'_, NetState>, backend: String, path: String) -> Result<(), String> {
    net_run(state, move |n| n.delete_session(&backend, &path)).await
}
#[tauri::command]
async fn net_dirs(state: State<'_, NetState>, backend: String, path: Option<String>) -> Result<serde_json::Value, String> {
    net_run(state, move |n| n.dirs(&backend, path.as_deref())).await
}
#[tauri::command]
async fn net_subscribe(state: State<'_, NetState>, backend: String, inst: String, tag: String) -> Result<(), String> {
    net_run(state, move |n| n.subscribe(&backend, &inst, &tag)).await
}
#[tauri::command]
async fn net_unsubscribe(state: State<'_, NetState>, backend: String, inst: String) -> Result<(), String> {
    net_run(state, move |n| {
        n.unsubscribe(&backend, &inst);
        Ok(())
    })
    .await
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Ereignisse aus Hintergrund-Threads als Tauri-Event "net-ev" an die Oberfläche.
            let h = app.handle().clone();
            let emit: net::Emit = std::sync::Arc::new(move |v| {
                let _ = h.emit("net-ev", v);
            });
            app.manage::<NetState>(std::sync::Arc::new(net::Net::new(emit, true)));
            Ok(())
        })
        .manage(Pi::default())
        .manage(preview::CacheLock::default())
        .manage(InfoCache::default())
        .manage(cli::Cli::default())
        .invoke_handler(tauri::generate_handler![shared_bookmarks_read, shared_bookmarks_update, shared_bookmarks_seed, start_pi, send, list_sessions, delete_session, inventory::inventory, inventory::read_config, inventory::save_config, preview::preview, preview::cached_purpose, preview::clear_previews, preview::explain_usage, folders::resume_session, folders::work_locations, folders::list_folders, editor::open_in_editor, editor::existing_paths, material::material_browser, material::material_preview, material::pick_material, material::prepare_material, media::media_routes, media::analyze_media, cli::pi_cli, cli::pi_cli_cancel, cli::startup_info, cli::latest_version, cli::open_changelog, cli::quit_app, net_backends, net_save_backend, net_delete_backend, net_list, net_create, net_remove, net_restart, net_send, net_dirs, net_sessions, net_delete_session, net_subscribe, net_unsubscribe])
        .build(tauri::generate_context!())
        .expect("Tauri-Start fehlgeschlagen")
        .run(|app, ev| {
            if let tauri::RunEvent::Exit = ev {
                stop(&app.state::<Pi>());
                if let Some(n) = app.try_state::<NetState>() {
                    n.shutdown();
                }
            }
        });
}
