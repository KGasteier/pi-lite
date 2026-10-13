//! Dauerhafte Einstellungen in `<copi_config_dir>/state.json` (plattformkonventioneller Pfad
//! via `pi_backend::resolve_config_file`/der `dirs`-Hilfsbibliothek, z. B. `~/.config/copi`
//! unter Linux, `~/Library/Application Support/copi` unter macOS, `%APPDATA%\copi` unter
//! Windows), mit lesendem Fallback auf den alten `pi-lite`-Pfad. Kein eigenes XDG-Handling
//! hier mehr; das Backend ist die einzige Quelle für Portabilitätslogik.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Store {
    pub output_size: f32,
    pub bookmarks: Vec<String>,
    /// Bookmark-Farbindex (0..=7, siehe `crate::bookmark_palette::PALETTE`) je Session-Pfad.
    /// Nur Einträge mit einem Pfad in `bookmarks` sind bedeutsam; ein fehlender Eintrag für
    /// einen markierten Pfad bedeutet Index 0 (Vorgabefarbe). Einträge für inzwischen nicht
    /// mehr markierte Pfade werden beim nächsten `toggle_bookmark`-Entfernen aufgeräumt, sind
    /// aber auch sonst harmlos (werden einfach nie gelesen).
    pub bookmark_colors: HashMap<String, u8>,
    /// "time" oder "bookmarks"
    pub sort: String,
    pub only_bookmarks: bool,
    pub colored_bookmarks: bool,
    pub category_names: Vec<String>,
    pub duration_history: Vec<f64>,
    pub side_visible: bool,
    pub helper_provider: String,
    pub helper_model: String,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            output_size: 15.0,
            bookmarks: Vec::new(),
            bookmark_colors: HashMap::new(),
            sort: "time".into(),
            only_bookmarks: false,
            colored_bookmarks: false,
            category_names: Vec::new(),
            duration_history: Vec::new(),
            side_visible: true,
            helper_provider: String::new(),
            helper_model: String::new(),
        }
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
        // Fremd editierte/veraltete state.json kann Farbindizes >= Palettenlänge enthalten
        // (z. B. nach einer künftig verkleinerten Palette); auf Index 0 normalisieren statt
        // mit einem ungültigen Index weiterzuarbeiten.
        for v in s.bookmark_colors.values_mut() {
            *v = crate::bookmark_palette::normalize(*v);
        }
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
    pub fn category_name(&self, i: u8) -> String {
        self.category_names.get(i as usize).filter(|n| !n.trim().is_empty()).cloned().unwrap_or_else(|| crate::bookmark_palette::name(i).into())
    }
    pub fn set_category_name(&mut self, i: u8, name: &str) {
        self.category_names.resize(8, String::new());
        self.category_names[crate::bookmark_palette::normalize(i) as usize] = name.trim().chars().take(48).collect();
    }
    pub fn record_duration(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.duration_history.push(seconds);
            if self.duration_history.len() > 30 { self.duration_history.remove(0); }
        }
    }
    pub fn duration_estimate(&self) -> f64 {
        let mut h: Vec<_> = self.duration_history.iter().copied().filter(|n| n.is_finite() && *n > 0.0).collect();
        if h.len() < 3 { return 180.0; }
        h.sort_by(f64::total_cmp);
        let m = h.len()/2;
        if h.len()%2 == 0 { (h[m-1]+h[m])/2.0 } else { h[m] }
    }
    pub fn is_bookmark(&self, path: &str) -> bool {
        self.bookmarks.iter().any(|b| b == path)
    }
    pub fn toggle_bookmark(&mut self, path: &str) {
        if let Some(i) = self.bookmarks.iter().position(|b| b == path) {
            self.bookmarks.remove(i);
            // Farbzuordnung einer entfernten Markierung aufräumen statt verwaist stehen zu
            // lassen (sonst täuscht state.json dauerhaft eine Farbe für einen nicht mehr
            // markierten Pfad vor).
            self.bookmark_colors.remove(path);
        } else {
            self.bookmarks.push(path.into());
        }
    }
    /// Farbindex eines Bookmarks (0..=7, normalisiert); 0 (Vorgabe) auch für nicht markierte
    /// Pfade oder Pfade ohne eigenen Eintrag.
    ///
    /// Verwendet von Session-Karten und Farbwähler.
    #[allow(dead_code)]
    pub fn bookmark_color(&self, path: &str) -> u8 {
        self.bookmark_colors.get(path).copied().map(crate::bookmark_palette::normalize).unwrap_or(0)
    }
    /// Setzt die Bookmark-Farbe eines **markierten** Pfads; ohne Wirkung, falls `path` kein
    /// Bookmark ist (keine verwaisten Farbeinträge für nicht markierte Sessions).
    /// `#[allow(dead_code)]`: siehe [`Store::bookmark_color`].
    #[allow(dead_code)]
    pub fn set_bookmark_color(&mut self, path: &str, index: u8) {
        if !self.is_bookmark(path) {
            return;
        }
        let index = crate::bookmark_palette::normalize(index);
        if index == 0 {
            self.bookmark_colors.remove(path);
        } else {
            self.bookmark_colors.insert(path.into(), index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_store_has_no_bookmarks_or_colors() {
        let s = Store::default();
        assert!(s.bookmarks.is_empty());
        assert!(s.bookmark_colors.is_empty());
    }

    #[test]
    fn toggle_bookmark_adds_and_removes_path() {
        let mut s = Store::default();
        assert!(!s.is_bookmark("/a"));
        s.toggle_bookmark("/a");
        assert!(s.is_bookmark("/a"));
        s.toggle_bookmark("/a");
        assert!(!s.is_bookmark("/a"));
    }

    #[test]
    fn bookmark_color_defaults_to_zero() {
        let mut s = Store::default();
        s.toggle_bookmark("/a");
        assert_eq!(s.bookmark_color("/a"), 0);
        assert_eq!(s.bookmark_color("/not-a-bookmark"), 0);
    }

    #[test]
    fn set_bookmark_color_only_affects_existing_bookmarks() {
        let mut s = Store::default();
        // Kein Bookmark -> keine Wirkung.
        s.set_bookmark_color("/a", 3);
        assert_eq!(s.bookmark_color("/a"), 0);
        assert!(s.bookmark_colors.is_empty());

        s.toggle_bookmark("/a");
        s.set_bookmark_color("/a", 3);
        assert_eq!(s.bookmark_color("/a"), 3);
    }

    #[test]
    fn set_bookmark_color_to_zero_removes_explicit_entry() {
        let mut s = Store::default();
        s.toggle_bookmark("/a");
        s.set_bookmark_color("/a", 5);
        assert_eq!(s.bookmark_colors.len(), 1);
        s.set_bookmark_color("/a", 0);
        assert_eq!(s.bookmark_color("/a"), 0);
        assert!(s.bookmark_colors.is_empty(), "Index 0 sollte keinen expliziten Eintrag brauchen");
    }

    #[test]
    fn set_bookmark_color_normalizes_out_of_range_index() {
        let mut s = Store::default();
        s.toggle_bookmark("/a");
        s.set_bookmark_color("/a", 200);
        assert_eq!(s.bookmark_color("/a"), 0);
    }

    #[test]
    fn toggle_off_removes_orphaned_color_entry() {
        let mut s = Store::default();
        s.toggle_bookmark("/a");
        s.set_bookmark_color("/a", 2);
        s.toggle_bookmark("/a"); // entfernt das Bookmark wieder
        assert!(s.bookmark_colors.is_empty());
        // Erneutes Markieren beginnt wieder bei der Vorgabefarbe, kein Geisterzustand.
        s.toggle_bookmark("/a");
        assert_eq!(s.bookmark_color("/a"), 0);
    }

    #[test]
    fn save_and_load_round_trip_bookmark_colors() {
        // `path()`/echtes Dateisystem wird hier bewusst nicht berührt (kein HOME-Override
        // in diesem Modul) – Serialisierung wird direkt über serde_json geprüft, was dem von
        // `save`/`load` verwendeten Format entspricht.
        let mut s = Store::default();
        s.toggle_bookmark("/a");
        s.set_bookmark_color("/a", 6);
        let bytes = serde_json::to_vec(&s).unwrap();
        let restored: Store = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.bookmark_color("/a"), 6);
    }

    #[test]
    fn load_from_disk_normalizes_out_of_range_stored_color_index() {
        // Simuliert eine fremd editierte/veraltete state.json mit einem Farbindex, der die
        // aktuelle Palettenlänge überschreitet.
        let mut s = Store::default();
        s.bookmarks.push("/a".into());
        s.bookmark_colors.insert("/a".into(), 250);
        let bytes = serde_json::to_vec(&s).unwrap();
        let mut restored: Store = serde_json::from_slice(&bytes).unwrap();
        for v in restored.bookmark_colors.values_mut() {
            *v = crate::bookmark_palette::normalize(*v);
        }
        assert_eq!(restored.bookmark_color("/a"), 0);
    }
}
