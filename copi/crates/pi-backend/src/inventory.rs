//! Begrenztes Inventar: nur bekannte Ressourcen-Wurzeln, niemals Session-/Auth-Dateien.
//! Portiert aus `src-tauri/src/inventory.rs`, ohne Tauri (keine `#[tauri::command]`-Makros,
//! kein `tauri::State`; der Lesesperren-Mechanismus ist ein einfacher [`std::sync::Mutex`]).
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, serde::Serialize)]
pub struct Entry {
    pub category: String,
    pub harness: String,
    pub name: String,
    pub path: String,
}

fn add(out: &mut Vec<Entry>, p: &Path, category: &str, harness: &str) {
    if let Ok(p) = p.canonicalize() {
        if out.iter().any(|e| e.path == p.to_string_lossy()) {
            return;
        }
        out.push(Entry {
            category: category.into(),
            harness: harness.into(),
            name: p.file_name().unwrap_or_default().to_string_lossy().into(),
            path: p.to_string_lossy().into(),
        });
    }
}

fn scan(out: &mut Vec<Entry>, root: &Path, category: &str, harness: &str, depth: usize) {
    if depth == 0 {
        return;
    }
    let Ok(files) = std::fs::read_dir(root) else { return };
    for f in files.flatten().take(500) {
        let p = f.path();
        let name = f.file_name().to_string_lossy().into_owned();
        if ["node_modules", "sessions", ".git", "target", "backups", "file-history"].contains(&name.as_str()) {
            continue;
        }
        if p.is_dir() {
            scan(out, &p, category, harness, depth - 1);
        } else {
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("md") || (category == "extensions" && ["ts", "js", "mjs", "cjs"].contains(&ext)) {
                add(out, &p, category, harness);
            }
        }
    }
}

fn home() -> Result<PathBuf, String> {
    crate::home().ok_or_else(|| "HOME/USERPROFILE fehlt".to_string())
}

/// Entdeckt Kontext-/Skill-/Erweiterungsdateien in bekannten Wurzeln (Agentenverzeichnis,
/// Projektordner, Home, Harness-Konventionen). Begrenzte Tiefe/Anzahl; schlie\u00dft
/// Session-Ordner und Auth-/Credential-Dateien grunds\u00e4tzlich aus.
pub fn inventory(cwd: String) -> Result<Vec<Entry>, String> {
    let home = home()?;
    let cwd = if cwd.is_empty() { std::env::current_dir().map_err(|e| e.to_string())? } else { PathBuf::from(cwd) };
    let agent = std::env::var("PI_CODING_AGENT_DIR").map(PathBuf::from).unwrap_or_else(|_| home.join(".pi/agent"));
    let mut out = Vec::new();
    for root in [&agent, &cwd.join(".pi")] {
        for (dir, category) in [("skills", "skills"), ("extensions", "extensions"), ("agents", "agents"), ("prompts", "agents")] {
            scan(&mut out, &root.join(dir), category, "pi", 4);
        }
        for name in ["AGENTS.override.md", "AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD", "SYSTEM.md", "APPEND_SYSTEM.md"] {
            add(&mut out, &root.join(name), "agents", "pi");
        }
        // Paketdeklarationen getrennt von Ressourcen anzeigen: deklariert heißt nicht geladen.
        if let Ok(text) = std::fs::read_to_string(root.join("settings.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(packages) = v["packages"].as_array() {
                    for p in packages {
                        if let Some(s) = p.as_str().or_else(|| p["source"].as_str()) {
                            out.push(Entry {
                                category: "packages".into(),
                                harness: "pi · deklariert".into(),
                                name: s.into(),
                                path: root.join("settings.json").to_string_lossy().into(),
                            });
                        }
                    }
                }
            }
        }
    }
    // Installierte npm-Paket-Markdown-Metadaten (begrenzt; keine Abhängigkeitsrekursion).
    if let Ok(packages) = std::fs::read_dir(agent.join("npm/node_modules")) {
        for package in packages.flatten().take(200) {
            let root = package.path();
            if package.file_name().to_string_lossy().starts_with('@') {
                if let Ok(scoped) = std::fs::read_dir(root) {
                    for p in scoped.flatten().take(200) {
                        add(&mut out, &p.path().join("README.md"), "extensions", "pi · npm installiert");
                    }
                }
            } else {
                add(&mut out, &root.join("README.md"), "extensions", "pi · npm installiert");
            }
        }
    }
    scan(&mut out, &home.join(".agents/skills"), "skills", "Agent Skills", 4);
    for ancestor in cwd.ancestors() {
        for name in ["AGENTS.override.md", "AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"] {
            add(&mut out, &ancestor.join(name), "agents", "Kontext (entdeckt)");
        }
        scan(&mut out, &ancestor.join(".agents/skills"), "skills", "Agent Skills", 3);
    }
    for (dir, harness) in [
        (".claude", "Claude Code"),
        (".opencode", "OpenCode"),
        (".config/opencode", "OpenCode"),
        (".openclaw/workspace", "OpenClaw"),
        (".codex", "Codex"),
        (".gemini", "Gemini"),
    ] {
        let root = home.join(dir);
        for name in ["AGENTS.md", "CLAUDE.md", "GEMINI.md", "SOUL.md", "USER.md", "IDENTITY.md", "TOOLS.md", "HEARTBEAT.md"] {
            add(&mut out, &root.join(name), "harnesses", harness);
        }
        for sub in ["agents", "skills", "rules", "commands"] {
            scan(&mut out, &root.join(sub), "harnesses", harness, 4);
        }
    }
    for (dir, harness) in [(".claude", "Claude Code"), (".opencode", "OpenCode"), (".codex", "Codex"), (".gemini", "Gemini")] {
        scan(&mut out, &cwd.join(dir), "harnesses", harness, 4);
    }
    out.sort_by(|a, b| (&a.category, &a.harness, &a.path).cmp(&(&b.category, &b.harness, &b.path)));
    Ok(out)
}

/// Prüft, dass `path` eine Markdown-Datei *und* Teil des entdeckten Inventars ist, bevor
/// Lese-/Schreibzugriff gewährt wird. Keine beliebigen Dateien editierbar.
fn allowed(cwd: String, path: String) -> Result<PathBuf, String> {
    let p = PathBuf::from(path).canonicalize().map_err(|e| e.to_string())?;
    if !p.extension().and_then(|s| s.to_str()).unwrap_or("").eq_ignore_ascii_case("md") {
        return Err("Nur Markdown-Dateien sind editierbar".into());
    }
    if !inventory(cwd)?.iter().any(|e| Path::new(&e.path) == p) {
        return Err("Datei gehört nicht zum entdeckten Inventar".into());
    }
    Ok(p)
}

pub fn read_config(cwd: String, path: String) -> Result<String, String> {
    let p = allowed(cwd, path)?;
    if p.metadata().map_err(|e| e.to_string())?.len() > 2_000_000 {
        return Err("Datei ist größer als 2 MB".into());
    }
    std::fs::read_to_string(p).map_err(|e| e.to_string())
}

/// Schreibt nur, wenn der aktuelle Dateiinhalt noch mit `original` übereinstimmt (optimistische
/// Konfliktprüfung: verhindert stilles Überschreiben externer Änderungen).
pub fn save_config(cwd: String, path: String, original: String, text: String) -> Result<(), String> {
    let p = allowed(cwd, path)?;
    if std::fs::read_to_string(&p).map_err(|e| e.to_string())? != original {
        return Err("Datei wurde extern verändert. Neu öffnen, bevor du speicherst.".into());
    }
    if text.len() > 2_000_000 {
        return Err("Text ist größer als 2 MB".into());
    }
    std::fs::write(p, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_non_markdown() {
        assert!(allowed(String::new(), std::env::current_exe().unwrap().to_string_lossy().into()).is_err());
    }

    #[test]
    fn editor_saves_and_rejects_external_changes() {
        // `read_config`/`save_config` rufen intern `inventory()` auf, das `HOME` liest.
        let _guard = crate::test_support::EnvGuard::read_only();
        let root = std::env::temp_dir().join(format!("pi-backend-inventory-{}-{}", std::process::id(), fastrand_seed()));
        let dir = root.join(".pi/agents");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("example.md");
        std::fs::write(&path, "before").unwrap();
        let cwd = root.to_string_lossy().into_owned();
        let file = path.to_string_lossy().into_owned();
        assert_eq!(read_config(cwd.clone(), file.clone()).unwrap(), "before");
        save_config(cwd.clone(), file.clone(), "before".into(), "after".into()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "after");
        assert!(save_config(cwd, file, "before".into(), "overwrite".into()).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inventory_is_bounded_and_excludes_credentials() {
        // `inventory()` liest `HOME`/`PI_CODING_AGENT_DIR`; ohne den crate-weiten Lock
        // könnte ein parallel laufender `with_temp_home`-Test (z. B. in `media::`/`preview::`)
        // mitten im Lesen `HOME` auf ein anderes Verzeichnis umsetzen und so flakey
        // falsche/unerwartete Pfade liefern.
        let _guard = crate::test_support::EnvGuard::read_only();
        let entries = inventory(String::new()).unwrap();
        assert!(entries.iter().all(|e| !e.path.ends_with("auth.json") && !e.path.contains("/sessions/")));
    }

    #[test]
    fn save_rejects_oversized_text() {
        // `save_config` ruft intern `inventory()` auf, das `HOME` liest.
        let _guard = crate::test_support::EnvGuard::read_only();
        let root = std::env::temp_dir().join(format!("pi-backend-inventory-big-{}-{}", std::process::id(), fastrand_seed()));
        let dir = root.join(".pi/agents");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.md");
        std::fs::write(&path, "x").unwrap();
        let cwd = root.to_string_lossy().into_owned();
        let file = path.to_string_lossy().into_owned();
        let huge = "y".repeat(2_000_001);
        assert!(save_config(cwd, file, "x".into(), huge).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    fn fastrand_seed() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64
    }
}
