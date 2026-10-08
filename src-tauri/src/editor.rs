use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn resolve_target(cwd: &str, path: Option<&str>) -> Result<PathBuf, String> {
    let base = Path::new(cwd).canonicalize().map_err(|e| format!("Workspace nicht verfügbar: {e}"))?;
    if !base.is_dir() { return Err("Workspace ist kein Ordner".into()); }
    let target = match path {
        None => base,
        Some(p) if !p.is_empty() && !p.contains('\0') => {
            let p = if let Some(rest) = p.strip_prefix("~/") {
                PathBuf::from(std::env::var("HOME").map_err(|_| "HOME fehlt")?).join(rest)
            } else { PathBuf::from(p) };
            if p.is_absolute() { p } else { base.join(p) }
        }
        _ => return Err("Ungültiger Dateipfad".into()),
    };
    let target = target.canonicalize().map_err(|e| format!("Datei/Ordner nicht verfügbar: {e}"))?;
    if !target.is_file() && !target.is_dir() { return Err("Keine reguläre Datei oder Ordner".into()); }
    Ok(target)
}

/// Prüft, welche Chat-Pfadverweise tatsächlich existieren; nur diese bekommen einen Editor-Button.
#[tauri::command]
pub fn existing_paths(cwd: String, paths: Vec<String>) -> Vec<bool> {
    paths.iter().take(200).map(|p| resolve_target(&cwd, Some(p)).is_ok()).collect()
}

/// Only explicit user clicks invoke this; arguments are passed without a shell.
#[tauri::command]
pub fn open_in_editor(cwd: String, path: Option<String>, line: Option<u32>, column: Option<u32>) -> Result<(), String> {
    let target = resolve_target(&cwd, path.as_deref())?;
    let mut cmd = Command::new("vscodium-electron");
    cmd.arg("--reuse-window");
    if target.is_file() && line.is_some() {
        cmd.arg("--goto").arg(format!("{}:{}:{}", target.display(), line.unwrap().max(1), column.unwrap_or(1).max(1)));
    } else {
        cmd.arg("--").arg(&target);
    }
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|e| format!("VSCodium konnte nicht gestartet werden: {e}"))?;
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_literal_and_must_exist() {
        let base = std::env::temp_dir().join(format!("pi-lite-editor-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let file = base.join("-file with spaces;echo.md");
        std::fs::write(&file, "test").unwrap();
        let cwd = base.to_str().unwrap();
        assert_eq!(resolve_target(cwd, None).unwrap(), base.canonicalize().unwrap());
        assert_eq!(resolve_target(cwd, Some("-file with spaces;echo.md")).unwrap(), file.canonicalize().unwrap());
        assert!(resolve_target(cwd, Some("missing.md")).is_err());
        assert!(resolve_target(cwd, Some("https://example.com/a.md")).is_err());
        assert_eq!(existing_paths(cwd.into(), vec!["-file with spaces;echo.md".into(), "ui/missing.js".into()]), [true, false]);
        std::fs::remove_dir_all(base).unwrap();
    }
}
