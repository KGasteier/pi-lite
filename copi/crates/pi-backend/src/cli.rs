//! `pi <unterbefehl>` aus dem Eingabefeld, Startinfos, Versionsprüfung.
//! Nie über eine Shell; nur eine feste Liste von Unterbefehlen ist erlaubt.
use crate::{Event, Sink};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Nicht-interaktive pi-Unterbefehle. `config` ist eine Terminal-Oberfläche und fehlt bewusst.
pub const SUBCOMMANDS: [&str; 7] = ["update", "install", "remove", "uninstall", "list", "auth", "mcp"];
pub const FLAGS: [&str; 4] = ["--version", "-v", "--help", "-h"];
pub const CHANGELOG_URL: &str = "https://pi.dev/changelog";
const LATEST_URL: &str = "https://pi.dev/api/latest-version";

pub fn allowed(args: &[String]) -> bool {
    args.first().is_some_and(|a| SUBCOMMANDS.contains(&a.as_str()) || FLAGS.contains(&a.as_str()))
}

/// Höchstens ein laufender CLI-Befehl.
#[derive(Clone, Default)]
pub struct Cli {
    child: Arc<Mutex<Option<Child>>>,
}

fn pump<R: Read + Send + 'static>(sink: Sink, id: String, stderr: bool, reader: R) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for chunk in BufReader::new(reader).split(b'\n').map_while(Result::ok) {
            let line = String::from_utf8_lossy(&chunk).trim_end_matches('\r').to_string();
            sink.send(Event::CliLine { id: id.clone(), stderr, line });
        }
    })
}

impl Cli {
    /// Startet `pi <args>` im Hintergrund; Ausgabe kommt als [`Event::CliLine`], Ende als [`Event::CliDone`].
    pub fn run(&self, sink: Sink, id: String, cwd: String, args: Vec<String>) {
        let me = self.clone();
        std::thread::spawn(move || {
            let result = me.run_blocking(sink.clone(), id.clone(), cwd, args);
            sink.send(Event::CliDone { id, result });
        });
    }

    fn run_blocking(&self, sink: Sink, id: String, cwd: String, args: Vec<String>) -> Result<i32, String> {
        if !allowed(&args) {
            return Err("Dieser pi-Befehl ist hier nicht verfügbar (interaktive Oberflächen wie `pi config` bitte im Terminal starten).".into());
        }
        if self.child.lock().unwrap().is_some() {
            return Err("Es läuft bereits ein pi-Befehl.".into());
        }
        let mut cmd = crate::winshim::pi_command(&args);
        cmd.current_dir(if cwd.is_empty() { ".".into() } else { cwd })
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Eigene Prozessgruppe, damit Abbrechen auch den mise-/Node-Unterprozess trifft.
        // Nur auf unix verfügbar; auf anderen Plattformen bleibt `cancel()` auf das
        // direkte Kindelement beschränkt (siehe dort).
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd.spawn().map_err(|e| format!("pi konnte nicht gestartet werden: {e}"))?;
        let out = pump(sink.clone(), id.clone(), false, child.stdout.take().unwrap());
        let err = pump(sink, id, true, child.stderr.take().unwrap());
        *self.child.lock().unwrap() = Some(child);
        let _ = out.join();
        let _ = err.join();
        let mut child = self.child.lock().unwrap().take().ok_or("pi-Prozess nicht mehr vorhanden")?;
        let status = child.wait().map_err(|e| e.to_string())?;
        Ok(status.code().unwrap_or(-1))
    }

    #[cfg(unix)]
    pub fn cancel(&self) {
        let pid = self.child.lock().unwrap().as_ref().map(Child::id);
        if let Some(pid) = pid {
            let _ = Command::new("kill").args(["-TERM", "--", &format!("-{pid}")]).status();
            let me = self.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                let mut slot = me.child.lock().unwrap();
                if let Some(child) = slot.as_mut().filter(|c| c.id() == pid) {
                    // Prozessgruppe treffen: auch Nachkommen können stdout offen halten.
                    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).status();
                    let _ = child.kill();
                }
            });
        }
    }

    /// Portabler Rückfall ohne Prozessgruppen-Semantik: beendet nur den direkten
    /// pi-Kindprozess (keine POSIX-Prozessgruppe verfügbar). Bekannte Lücke: ein von pi
    /// gestarteter Unterprozess (z. B. mise/Node) kann dadurch weiterlaufen.
    #[cfg(not(unix))]
    pub fn cancel(&self) {
        let mut slot = self.child.lock().unwrap();
        if let Some(child) = slot.as_mut() {
            let _ = child.kill();
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContextFile {
    pub name: String,
    pub path: String,
    pub scope: String,
}

#[derive(Clone, Debug, Default)]
pub struct StartupInfo {
    pub version: String,
    pub context: Vec<ContextFile>,
    pub extensions: Vec<String>,
}

/// Pro Ordner gewinnt AGENTS.override.md; sonst die erste vorhandene der übrigen Kontextdateien.
fn context_in(dir: &Path, scope: &str, out: &mut Vec<ContextFile>) {
    let pick = if dir.join("AGENTS.override.md").is_file() {
        Some("AGENTS.override.md")
    } else {
        ["AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"].into_iter().find(|n| dir.join(n).is_file())
    };
    if let Some(name) = pick {
        let path = dir.join(name).to_string_lossy().into_owned();
        if !out.iter().any(|f| f.path == path) {
            out.push(ContextFile { name: name.into(), path, scope: scope.into() });
        }
    }
}

pub fn context_files(agent: Option<&Path>, cwd: &Path) -> Vec<ContextFile> {
    let mut out = Vec::new();
    if let Some(a) = agent {
        context_in(a, "user", &mut out);
    }
    let mut chain: Vec<&Path> = cwd.ancestors().collect();
    chain.reverse();
    for dir in chain {
        context_in(dir, "project", &mut out);
    }
    out
}

fn extension_names(root: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(root.join("extensions")) {
        for e in entries.flatten().take(200) {
            let p = e.path();
            let name = if p.is_dir() {
                e.file_name().to_string_lossy().into_owned()
            } else if matches!(p.extension().and_then(|s| s.to_str()), Some("ts" | "js" | "mjs" | "cjs")) {
                p.file_stem().unwrap_or_default().to_string_lossy().into_owned()
            } else {
                continue;
            };
            if !name.starts_with('.') {
                out.push(name);
            }
        }
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

pub fn extensions(agent: Option<&Path>, cwd: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(a) = agent {
        extension_names(a, &mut out);
    }
    extension_names(&cwd.join(".pi"), &mut out);
    out.sort();
    out.dedup();
    out
}

static VERSION: OnceLock<String> = OnceLock::new();
pub fn pi_version() -> String {
    VERSION
        .get_or_init(|| {
            crate::winshim::pi_command(&["--version".to_string()])
                .stdin(Stdio::null())
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().last().unwrap_or("").to_string())
                .unwrap_or_default()
        })
        .clone()
}

/// Version, Kontextdateien und Extensions für die Startkarte (blockiert – im Thread aufrufen).
pub fn startup_info(cwd: &str) -> StartupInfo {
    let cwd = if cwd.is_empty() { std::env::current_dir().unwrap_or_default() } else { PathBuf::from(cwd) };
    let agent = crate::agent_dir();
    StartupInfo { version: pi_version(), context: context_files(agent.as_deref(), &cwd), extensions: extensions(agent.as_deref(), &cwd) }
}

fn flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| !v.is_empty() && !["0", "false", "no"].contains(&v.to_lowercase().as_str()))
}

/// Dieselbe Abfrage wie pi selbst; respektiert PI_SKIP_VERSION_CHECK/PI_OFFLINE (blockiert).
pub fn latest_version() -> Option<String> {
    if flag("PI_SKIP_VERSION_CHECK") || flag("PI_OFFLINE") {
        return None;
    }
    let out = Command::new("curl").args(["-fsS", "-m", "6", LATEST_URL]).stdin(Stdio::null()).output().ok()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v["version"].as_str().map(String::from)
}

/// Ist `latest` neuer als `current`? (semver-artig, Vorabversionen zählen als älter)
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    fn parse(v: &str) -> ([u64; 3], bool) {
        let v = v.trim_start_matches('v');
        let (core, pre) = match v.split_once('-') {
            Some((c, _)) => (c, true),
            None => (v, false),
        };
        let mut n = [0u64; 3];
        for (i, p) in core.split('.').take(3).enumerate() {
            n[i] = p.parse().unwrap_or(0);
        }
        (n, pre)
    }
    let (a, apre) = parse(latest);
    let (b, bpre) = parse(current);
    if a != b {
        return a > b;
    }
    bpre && !apre
}

/// Betriebssystem-Befehl, der eine Adresse im Standardbrowser öffnet (ohne Shell).
#[cfg(target_os = "macos")]
fn open_command(url: &str) -> Command {
    let mut c = Command::new("open");
    c.arg(url);
    c
}
#[cfg(target_os = "windows")]
fn open_command(url: &str) -> Command {
    // Bewusst `explorer.exe` statt `cmd /c start`: `cmd.exe` interpretiert seine
    // Kommandozeile selbst noch einmal (Metazeichen wie `&`, `|`, `^`, Umleitungen) –
    // auch wenn die URL hier als einzelnes `Command`-Argument übergeben wird und nie durch
    // eine Shell *dieses* Prozesses läuft, liest `cmd /c` die volle Zeile erneut als
    // Batch-Befehl ein. `explorer.exe <url>` übergibt die URL dagegen nur an den
    // registrierten Standardbrowser, ohne sie als Befehlszeile zu interpretieren.
    // **Dieser Zweig ist nicht in dieser Linux-Umgebung lauffähig getestet**, siehe
    // `docs/portability.md`. Scheme-Prüfung erfolgt vorab in [`open_url`], da `explorer.exe`
    // bei einem lokalen Pfad statt einer URL die Datei direkt ausführen/öffnen würde.
    let mut c = Command::new("explorer.exe");
    c.arg(url);
    c
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_command(url: &str) -> Command {
    let mut c = Command::new("xdg-open");
    c.arg(url);
    c
}

/// Nur `http://`/`https://`-Adressen dürfen geöffnet werden. Das ist auf allen Plattformen
/// relevant, aber besonders unter Windows kritisch: `explorer.exe <pfad>` öffnet bei einem
/// lokalen Dateipfad (statt einer URL) direkt diese Datei bzw. führt sie aus, statt einen
/// Fehler zu liefern – ohne diese Prüfung könnte ein von außen beeinflussbarer "Link"
/// (z. B. aus KI-generiertem Inhalt) beliebige lokale Dateien öffnen/ausführen.
fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Öffnet eine Adresse im Standardbrowser (ohne Shell). Lehnt alles außer `http(s)://` ab.
pub fn open_url(url: &str) -> Result<(), String> {
    if !is_http_url(url) {
        return Err("nur http(s)-Adressen dürfen geöffnet werden".into());
    }
    open_command(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|mut c| {
            std::thread::spawn(move || {
                let _ = c.wait();
            });
        })
        .map_err(|e| format!("Standardbrowser konnte nicht geöffnet werden: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    #[test]
    fn only_known_subcommands_are_allowed() {
        assert!(allowed(&s(&["update"])));
        assert!(allowed(&s(&["--version"])));
        assert!(!allowed(&s(&["config"])));
        assert!(!allowed(&[]));
    }
    #[test]
    fn version_compare() {
        assert!(is_newer_version("1.0.5", "1.0.4"));
        assert!(!is_newer_version("1.0.4", "v1.0.4"));
        assert!(is_newer_version("1.0.4", "1.0.4-beta"));
        assert!(!is_newer_version("1.0.3", "1.0.4"));
    }

    #[test]
    fn open_url_rejects_non_http_schemes() {
        assert!(is_http_url("https://pi.dev/changelog"));
        assert!(is_http_url("http://example.com"));
        assert!(!is_http_url("file:///etc/passwd"));
        assert!(!is_http_url("javascript:alert(1)"));
        assert!(!is_http_url("C:\\Windows\\System32\\evil.exe"));
        assert!(!is_http_url("/etc/passwd"));
        assert!(!is_http_url(""));

        assert!(open_url("file:///etc/passwd").is_err());
        assert!(open_url("javascript:alert(1)").is_err());
    }
}
