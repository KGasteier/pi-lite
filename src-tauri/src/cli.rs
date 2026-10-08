//! pi-CLI-Unterbefehle (`pi update` usw.) aus dem Eingabefeld, Startinfos und Versionsprüfung.
//! Es wird nie eine Shell benutzt; nur eine feste Liste von Unterbefehlen ist erlaubt.
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Nicht-interaktive pi-Unterbefehle. `config` ist eine Terminal-Oberfläche und fehlt bewusst.
const SUBCOMMANDS: [&str; 7] = ["update", "install", "remove", "uninstall", "list", "auth", "mcp"];
const FLAGS: [&str; 4] = ["--version", "-v", "--help", "-h"];
const CHANGELOG_URL: &str = "https://pi.dev/changelog";
const LATEST_URL: &str = "https://pi.dev/api/latest-version";

#[derive(Default)]
pub struct Cli { child: Mutex<Option<Child>> }

fn allowed(args: &[String]) -> bool {
    args.first().is_some_and(|a| SUBCOMMANDS.contains(&a.as_str()) || FLAGS.contains(&a.as_str()))
}

#[derive(Serialize, Clone)]
struct CliLine { id: String, stderr: bool, line: String }

fn pump<R: Read + Send + 'static>(app: AppHandle, id: String, stderr: bool, reader: R) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for chunk in BufReader::new(reader).split(b'\n').map_while(Result::ok) {
            let line = String::from_utf8_lossy(&chunk).trim_end_matches('\r').to_string();
            let _ = app.emit("pi-cli", CliLine { id: id.clone(), stderr, line });
        }
    })
}

fn run(app: AppHandle, id: String, cwd: String, args: Vec<String>) -> Result<i32, String> {
    let state = app.state::<Cli>();
    if state.child.lock().unwrap().is_some() { return Err("Es läuft bereits ein pi-Befehl.".into()); }
    let mut child = Command::new("pi").args(&args)
        .current_dir(if cwd.is_empty() { ".".into() } else { cwd })
        .env("NO_COLOR", "1")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        // Eigene Prozessgruppe, damit Abbrechen auch den mise-/Node-Unterprozess trifft.
        .process_group(0)
        .spawn().map_err(|e| format!("pi konnte nicht gestartet werden: {e}"))?;
    let out = pump(app.clone(), id.clone(), false, child.stdout.take().unwrap());
    let err = pump(app.clone(), id, true, child.stderr.take().unwrap());
    *state.child.lock().unwrap() = Some(child);
    let _ = out.join(); let _ = err.join();
    let mut child = state.child.lock().unwrap().take().ok_or("pi-Prozess nicht mehr vorhanden")?;
    let status = child.wait().map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(-1))
}

/// Führt `pi <args>` aus; Ausgabe kommt zeilenweise als Event `pi-cli`. Rückgabe: Exit-Code.
#[tauri::command]
pub async fn pi_cli(app: AppHandle, id: String, cwd: String, args: Vec<String>) -> Result<i32, String> {
    if !allowed(&args) { return Err("Dieser pi-Befehl ist hier nicht verfügbar (interaktive Oberflächen wie `pi config` bitte im Terminal starten).".into()); }
    tauri::async_runtime::spawn_blocking(move || run(app, id, cwd, args)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn pi_cli_cancel(state: tauri::State<Cli>) {
    if let Some(child) = state.child.lock().unwrap().as_mut() {
        let _ = Command::new("kill").args(["-TERM", "--", &format!("-{}", child.id())]).status();
        let _ = child.kill();
    }
}

#[derive(Serialize)]
pub struct ContextFile { name: String, path: String, scope: String }
#[derive(Serialize)]
pub struct StartupInfo { version: String, context: Vec<ContextFile>, extensions: Vec<String> }

fn agent_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("PI_CODING_AGENT_DIR") { return Some(d.into()); }
    std::env::var("HOME").ok().map(|h| Path::new(&h).join(".pi/agent"))
}

/// Pro Ordner gewinnt AGENTS.override.md; sonst die erste vorhandene der übrigen Kontextdateien.
fn context_in(dir: &Path, scope: &str, out: &mut Vec<ContextFile>) {
    let pick = if dir.join("AGENTS.override.md").is_file() { Some("AGENTS.override.md") }
        else { ["AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"].into_iter().find(|n| dir.join(n).is_file()) };
    if let Some(name) = pick {
        let path = dir.join(name).to_string_lossy().into_owned();
        if !out.iter().any(|f| f.path == path) { out.push(ContextFile { name: name.into(), path, scope: scope.into() }); }
    }
}

fn context_files(agent: Option<&Path>, cwd: &Path) -> Vec<ContextFile> {
    let mut out = Vec::new();
    if let Some(a) = agent { context_in(a, "user", &mut out); }
    let mut chain: Vec<&Path> = cwd.ancestors().collect();
    chain.reverse();
    for dir in chain { context_in(dir, "project", &mut out); }
    out
}

fn extension_names(root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root.join("extensions")) else { return };
    for e in entries.flatten().take(200) {
        let p = e.path();
        let name = if p.is_dir() { e.file_name().to_string_lossy().into_owned() }
            else if matches!(p.extension().and_then(|s| s.to_str()), Some("ts" | "js" | "mjs" | "cjs")) { p.file_stem().unwrap_or_default().to_string_lossy().into_owned() }
            else { continue };
        if !name.starts_with('.') { out.push(name); }
    }
    if let Ok(text) = std::fs::read_to_string(root.join("settings.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            for p in v["packages"].as_array().into_iter().flatten() {
                if let Some(s) = p.as_str().or_else(|| p["source"].as_str()) {
                    out.push(s.trim_start_matches("npm:").trim_start_matches("git:").to_string());
                }
            }
        }
    }
}

fn extensions(agent: Option<&Path>, cwd: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(a) = agent { extension_names(a, &mut out); }
    extension_names(&cwd.join(".pi"), &mut out);
    out.sort(); out.dedup();
    out
}

static VERSION: OnceLock<String> = OnceLock::new();
fn pi_version() -> String {
    VERSION.get_or_init(|| {
        Command::new("pi").arg("--version").stdin(Stdio::null()).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().last().unwrap_or("").to_string())
            .unwrap_or_default()
    }).clone()
}

/// Version, geladene Kontextdateien und Extensions für die Startkarte (Skills/Prompts kommen aus `get_commands`).
#[tauri::command]
pub async fn startup_info(cwd: String) -> Result<StartupInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cwd = if cwd.is_empty() { std::env::current_dir().map_err(|e| e.to_string())? } else { PathBuf::from(cwd) };
        let agent = agent_dir();
        Ok(StartupInfo { version: pi_version(), context: context_files(agent.as_deref(), &cwd), extensions: extensions(agent.as_deref(), &cwd) })
    }).await.map_err(|e| e.to_string())?
}

fn flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| !v.is_empty() && !["0", "false", "no"].contains(&v.to_lowercase().as_str()))
}

/// Dieselbe Abfrage wie pi selbst (pi.dev/api/latest-version); respektiert PI_SKIP_VERSION_CHECK/PI_OFFLINE.
#[tauri::command]
pub async fn latest_version() -> Option<String> {
    if flag("PI_SKIP_VERSION_CHECK") || flag("PI_OFFLINE") { return None; }
    tauri::async_runtime::spawn_blocking(|| {
        let out = Command::new("curl").args(["-fsS", "-m", "6", LATEST_URL]).stdin(Stdio::null()).output().ok()?;
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        v["version"].as_str().map(String::from)
    }).await.ok().flatten()
}

/// Öffnet ausschließlich die feste Changelog-Adresse im Standardbrowser.
#[tauri::command]
pub fn open_changelog() -> Result<(), String> {
    Command::new("xdg-open").arg(CHANGELOG_URL).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map(|_| ()).map_err(|e| format!("xdg-open fehlgeschlagen: {e}"))
}

#[tauri::command]
pub fn quit_app(app: AppHandle) { app.exit(0); }

#[cfg(test)]
mod tests {
    use super::*;
    fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }
    #[test] fn only_known_subcommands_are_allowed() {
        assert!(allowed(&s(&["update"])) && allowed(&s(&["list"])) && allowed(&s(&["--version"])));
        assert!(!allowed(&s(&["config"])) && !allowed(&s(&["rm", "-rf"])) && !allowed(&[]) && !allowed(&s(&["--mode", "rpc"])));
    }
    #[test] fn override_replaces_other_context_files_in_same_dir() {
        let root = std::env::temp_dir().join(format!("pi-lite-ctx-{}", std::process::id()));
        let cwd = root.join("a/b"); std::fs::create_dir_all(&cwd).unwrap();
        std::fs::write(root.join("a/AGENTS.md"), "x").unwrap();
        std::fs::write(root.join("a/AGENTS.override.md"), "x").unwrap();
        std::fs::write(cwd.join("CLAUDE.md"), "x").unwrap();
        let names: Vec<String> = context_files(None, &cwd).into_iter().filter(|f| f.path.starts_with(root.to_str().unwrap())).map(|f| f.name).collect();
        assert_eq!(names, ["AGENTS.override.md", "CLAUDE.md"]);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn extensions_include_directories_files_and_packages() {
        let root = std::env::temp_dir().join(format!("pi-lite-ext-{}", std::process::id()));
        std::fs::create_dir_all(root.join("extensions/dir-ext")).unwrap();
        std::fs::write(root.join("extensions/one.ts"), "").unwrap();
        std::fs::write(root.join("extensions/notes.md"), "").unwrap();
        std::fs::write(root.join("settings.json"), r#"{"packages":["npm:pi-custom-endpoint"]}"#).unwrap();
        assert_eq!(extensions(Some(&root), Path::new("/nonexistent")), ["dir-ext", "one", "pi-custom-endpoint"]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
