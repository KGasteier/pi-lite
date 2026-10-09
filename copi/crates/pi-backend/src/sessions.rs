//! Sessiondateien von pi auflisten und löschen.
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct SessionInfo {
    pub path: String,
    pub cwd: String,
    pub title: String,
    /// Von Nutzer oder Extension vergebener Name (letzter `session_info`-Eintrag).
    pub name: String,
    /// Änderungszeit in Sekunden seit Epoch.
    pub modified: u64,
}

pub fn sessions_root() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("PI_CODING_AGENT_SESSION_DIR") {
        return Some(d.into());
    }
    crate::agent_dir().map(|a| a.join("sessions"))
}

/// Liest Kopfzeile, erste Nutzer-Nachricht und `session_info`-Zeilen.
pub fn read_info(path: &Path, modified: u64) -> Option<SessionInfo> {
    let mut lines = BufReader::new(std::fs::File::open(path).ok()?).lines();
    let head: serde_json::Value = serde_json::from_str(&lines.next()?.ok()?).ok()?;
    let cwd = head["cwd"].as_str().unwrap_or("").to_string();
    let (mut title, mut name) = (String::new(), String::new());
    for l in lines.map_while(Result::ok) {
        if l.contains("\"type\":\"session_info\"") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) {
                if let Some(n) = v["name"].as_str() {
                    name = n.trim().to_string();
                }
            }
        } else if title.is_empty() && l.contains("\"role\":\"user\"") {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) else { continue };
            let c = &v["message"]["content"];
            let t = c.as_str().map(String::from).or_else(|| c.as_array()?.iter().find_map(|p| p["text"].as_str().map(String::from)));
            if let Some(t) = t {
                title = t.chars().take(90).collect();
            }
        }
    }
    Some(SessionInfo { path: path.to_string_lossy().into(), cwd, title, name, modified })
}

/// Cache mit präziser Änderungszeit und Dateigröße (mehrere Änderungen pro Sekunde).
#[derive(Default)]
pub struct InfoCache(HashMap<String, (SessionInfo, std::time::SystemTime, u64)>);

impl InfoCache {
    /// Neueste 200 Sessions, absteigend nach Änderungszeit.
    pub fn list(&mut self) -> Vec<SessionInfo> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let Some(root) = sessions_root() else { return out };
        let Ok(dirs) = std::fs::read_dir(root) else { return out };
        for d in dirs.flatten() {
            let Ok(files) = std::fs::read_dir(d.path()) else { continue };
            for f in files.flatten() {
                let p = f.path();
                if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Ok(meta) = p.metadata() else { continue };
                let Ok(stamp) = meta.modified() else { continue };
                let Ok(age) = stamp.duration_since(std::time::UNIX_EPOCH) else { continue };
                let modified = age.as_secs();
                let key = p.to_string_lossy().to_string();
                seen.insert(key.clone());
                if let Some((i, _, _)) = self.0.get(&key).filter(|(_, t, len)| *t == stamp && *len == meta.len()) {
                    out.push(i.clone());
                } else if let Some(i) = read_info(&p, modified) {
                    self.0.insert(key, (i.clone(), stamp, meta.len()));
                    out.push(i);
                }
            }
        }
        self.0.retain(|key, _| seen.contains(key));
        out.sort_by(|a, b| b.modified.cmp(&a.modified));
        out.truncate(200);
        out
    }

    /// Löscht nur `*.jsonl` unterhalb des Sessions-Ordners.
    pub fn delete(&mut self, path: &str) -> Result<(), String> {
        let root = sessions_root().ok_or("kein Sessions-Ordner")?.canonicalize().map_err(|e| e.to_string())?;
        let p = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
        if !p.starts_with(&root) || p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            return Err("Pfad liegt nicht im Sessions-Ordner".into());
        }
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
        self.0.remove(path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_title_and_last_name() {
        let dir = std::env::temp_dir().join(format!("copi-sess-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.jsonl");
        std::fs::write(&f, concat!(
            "{\"type\":\"session\",\"cwd\":\"/tmp/x\"}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"Hallo Welt\"}]}}\n",
            "{\"type\":\"session_info\",\"name\":\"Erst\"}\n",
            "{\"type\":\"session_info\",\"name\":\" Zweit \"}\n",
        )).unwrap();
        let i = read_info(&f, 5).unwrap();
        assert_eq!((i.cwd.as_str(), i.title.as_str(), i.name.as_str(), i.modified), ("/tmp/x", "Hallo Welt", "Zweit", 5));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
