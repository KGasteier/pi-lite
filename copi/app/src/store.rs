//! Dauerhafte Einstellungen in `<copi_config_dir>/state.json` (plattformkonventioneller Pfad
//! via `pi_backend::resolve_config_file`/der `dirs`-Hilfsbibliothek, z. B. `~/.config/copi`
//! unter Linux, `~/Library/Application Support/copi` unter macOS, `%APPDATA%\copi` unter
//! Windows), mit lesendem Fallback auf den alten `pi-lite`-Pfad. Kein eigenes XDG-Handling
//! hier mehr; das Backend ist die einzige Quelle für Portabilitätslogik.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Store {
    pub output_size: f32,
    pub bookmarks: Vec<String>,
    /// "time" oder "bookmarks"
    pub sort: String,
    pub only_bookmarks: bool,
    pub side_visible: bool,
    pub helper_provider: String,
    pub helper_model: String,
}

impl Default for Store {
    fn default() -> Self {
        Self { output_size: 15.0, bookmarks: Vec::new(), sort: "time".into(), only_bookmarks: false, side_visible: true, helper_provider: String::new(), helper_model: String::new() }
    }
}

fn path() -> Option<PathBuf> {
    pi_backend::resolve_config_file("state.json")
}

impl Store {
    pub fn load(enabled: bool) -> Self {
        if !enabled {
            return Self::default();
        }
        let mut s: Self = path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        s.output_size = s.output_size.clamp(10.0, 30.0);
        s
    }
    pub fn save(&self, enabled: bool) -> Result<(), String> {
        if !enabled {
            return Ok(());
        }
        let p = path().ok_or("Kein Konfigurationsordner")?;
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(tmp, p).map_err(|e| e.to_string())
    }
    pub fn is_bookmark(&self, path: &str) -> bool {
        self.bookmarks.iter().any(|b| b == path)
    }
    pub fn toggle_bookmark(&mut self, path: &str) {
        if let Some(i) = self.bookmarks.iter().position(|b| b == path) {
            self.bookmarks.remove(i);
        } else {
            self.bookmarks.push(path.into());
        }
    }
}
