//! Dateien/Ordner in VSCodium öffnen – nur auf ausdrücklichen Klick, ohne Shell.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn resolve_target(cwd: &str, path: Option<&str>) -> Result<PathBuf, String> {
    let base = Path::new(cwd).canonicalize().map_err(|e| format!("Workspace nicht verfügbar: {e}"))?;
    if !base.is_dir() {
        return Err("Workspace ist kein Ordner".into());
    }
    let target = match path {
        None => base,
        Some(p) if !p.is_empty() && !p.contains('\0') => {
            let p = if let Some(rest) = p.strip_prefix("~/") {
                crate::home().ok_or("HOME/USERPROFILE fehlt")?.join(rest)
            } else {
                PathBuf::from(p)
            };
            if p.is_absolute() {
                p
            } else {
                base.join(p)
            }
        }
        _ => return Err("Ungültiger Dateipfad".into()),
    };
    let target = target.canonicalize().map_err(|e| format!("Datei/Ordner nicht verfügbar: {e}"))?;
    if !target.is_file() && !target.is_dir() {
        return Err("Keine reguläre Datei oder Ordner".into());
    }
    Ok(target)
}

pub fn path_exists(cwd: &str, path: &str) -> bool {
    resolve_target(cwd, Some(path)).is_ok()
}

/// Editor-Befehl: `COPI_EDITOR_CMD` überschreibt den Plattform-Standard (z. B. `code`
/// unter Windows/macOS, wo `vscodium-electron` normalerweise nicht im PATH liegt).
fn editor_command() -> String {
    std::env::var("COPI_EDITOR_CMD").unwrap_or_else(|_| default_editor_command().to_string())
}

#[cfg(target_os = "windows")]
fn default_editor_command() -> &'static str {
    "code"
}
#[cfg(target_os = "macos")]
fn default_editor_command() -> &'static str {
    "code"
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn default_editor_command() -> &'static str {
    "vscodium-electron"
}

pub fn open_in_editor(cwd: &str, path: Option<&str>, line: Option<u32>) -> Result<(), String> {
    let target = resolve_target(cwd, path)?;
    let editor = editor_command();
    let mut cmd = Command::new(&editor);
    cmd.arg("--reuse-window");
    match line {
        Some(l) if target.is_file() => {
            cmd.arg("--goto").arg(format!("{}:{}:1", target.display(), l.max(1)));
        }
        _ => {
            cmd.arg("--").arg(&target);
        }
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Editor ({editor}) konnte nicht gestartet werden: {e}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
