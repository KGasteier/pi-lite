//! Dauerhafte Einstellungen in `<copi_config_dir>/state.json` (plattformkonventioneller Pfad
//! via `pi_backend::resolve_config_file`/der `dirs`-Hilfsbibliothek, z. B. `~/.config/copi`
//! unter Linux, `~/Library/Application Support/copi` unter macOS, `%APPDATA%\copi` unter
//! Windows), mit lesendem Fallback auf den alten `pi-lite`-Pfad. Schreibzugriffe gehen
//! immer in den bevorzugten copi-Pfad, mit Dateisperre und feldweiser Delta-Zusammenführung.
//! Das Backend bleibt die einzige Quelle für Portabilitätslogik.
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    /// Rechte Seitenleiste „Netz“ (menschlicher Chat, vorerst Platzhalter); standardmäßig aus.
    pub net_visible: bool,
    pub helper_provider: String,
    pub helper_model: String,
    /// Letzter gelesener/geschriebener Stand für feldweise Mehrinstanz-Zusammenführung.
    #[serde(skip)]
    baseline: serde_json::Value,
    #[serde(skip)]
    last_check: Option<std::time::Instant>,
    #[serde(skip)]
    shared_baseline: Value,
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
            net_visible: false,
            helper_provider: String::new(),
            helper_model: String::new(),
            baseline: serde_json::Value::Null,
            last_check: None,
            shared_baseline: Value::Null,
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
        s.baseline = serde_json::to_value(&s).unwrap_or_default();
        // Migration je App einmalig, sonst könnten gelöschte Bookmarks wieder auftauchen.
        let marker = pi_backend::copi_config_dir().map(|p| p.join("bookmarks-shared-v1"));
        let shared = if marker.as_ref().is_some_and(|p| p.exists()) {
            crate::shared_bookmarks::read()
        } else { crate::shared_bookmarks::seed(&s.shared_value()) };
        if let Ok(v) = shared {
            s.apply_shared(&v); s.shared_baseline = v;
            if let Some(p) = marker { if let Some(d)=p.parent() { let _=std::fs::create_dir_all(d); } let _=std::fs::write(p,b"1"); }
        }
        s
    }
    pub fn save(&mut self, enabled: bool) -> Result<(), String> {
        if !enabled { return Ok(()); }
        // Fallback nur lesen; immer in den bevorzugten copi-Konfigurationsordner schreiben.
        let p = pi_backend::copi_config_dir().ok_or("Kein Konfigurationsordner")?.join("state.json");
        let base = if self.shared_baseline.is_null() { crate::shared_bookmarks::empty() } else { self.shared_baseline.clone() };
        let v = crate::shared_bookmarks::update(&base, &self.shared_value())?;
        self.apply_shared(&v); self.shared_baseline = v.clone();
        self.save_at(&p)?;
        // Der private Store darf gemeinsame Markierungen nicht zurücksetzen.
        self.apply_shared(&v); self.shared_baseline = v;
        Ok(())
    }

    fn shared_value(&self) -> Value {
        serde_json::json!({"bookmarks":self.bookmarks,"bookmark_colors":self.bookmark_colors,"category_names":self.category_names})
    }
    fn apply_shared(&mut self, v:&Value) {
        self.bookmarks = serde_json::from_value(v["bookmarks"].clone()).unwrap_or_default();
        self.bookmark_colors = serde_json::from_value(v["bookmark_colors"].clone()).unwrap_or_default();
        self.category_names = serde_json::from_value(v["category_names"].clone()).unwrap_or_default();
    }

    fn baseline_value(&self) -> Value {
        if self.baseline.is_null() { serde_json::to_value(Self::default()).unwrap() }
        else { self.baseline.clone() }
    }

    fn save_at(&mut self, p: &std::path::Path) -> Result<(), String> {
        if let Some(d) = p.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())?; }
        // Feste Sperrdatei (nie umbenennen): schützt Lesen → Delta → atomaren Austausch.
        let lock = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true)
            .open(p.with_extension("lock")).map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        let disk = if p.exists() { read_state(p)? } else { self.baseline_value() };
        let local = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        let merged = merge_state(&self.baseline_value(), &local, &disk);
        let tmp = p.with_extension(format!("json.{}.tmp", std::process::id()));
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        use std::io::Write;
        f.write_all(&serde_json::to_vec_pretty(&merged).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, p).map_err(|e| e.to_string())?;
        let mut s: Self = serde_json::from_value(merged.clone()).map_err(|e| e.to_string())?;
        s.baseline = merged;
        *self = s;
        Ok(()) // Sperre wird mit dem File-Handle freigegeben.
    }

    /// Fremde Änderungen lesen, eigene noch nicht gespeicherte Eingaben erhalten.
    pub fn refresh(&mut self, enabled: bool) {
        if !enabled { return; }
        if self.last_check.is_some_and(|t| t.elapsed().as_secs_f32() < 1.0) { return; }
        self.last_check = Some(std::time::Instant::now());
        let Some(p) = path() else { return };
        let Ok(disk) = read_state(&p) else { return };
        let shared_base = self.shared_baseline.clone();
        let shared_local = self.shared_value();
        let local = serde_json::to_value(&*self).unwrap_or_default();
        let merged = merge_state(&self.baseline_value(), &local, &disk);
        if let Ok(mut s) = serde_json::from_value::<Self>(merged) {
            s.baseline = disk;
            s.last_check = self.last_check;
            if let Ok(v) = crate::shared_bookmarks::read() {
                let b = if shared_base.is_null() { crate::shared_bookmarks::empty() } else { shared_base };
                s.apply_shared(&crate::shared_bookmarks::merge(&b, &shared_local, &v));
                s.shared_baseline = v;
            }
            *self = s;
        }
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
    /// Lokale Prozentschätzung (gleiche Regeln wie ui/duration-estimate.js): bis zur
    /// erwarteten Dauer linear bis 90 %, danach Median der Restdauer längerer Läufe,
    /// höchstens 99 %. Liefert (Prozent, überzogen, Erwartung in s).
    pub fn duration_percent(&self, elapsed: f64) -> (f64, bool, f64) {
        let est = self.duration_estimate().max(1.0);
        if elapsed <= est {
            return ((elapsed / est * 90.0).clamp(0.0, 90.0), false, est);
        }
        let mut r: Vec<f64> = self.duration_history.iter().copied().filter(|n| *n > elapsed).map(|n| n - elapsed).collect();
        let remaining = if r.is_empty() { est } else {
            r.sort_by(f64::total_cmp);
            let m = r.len() / 2;
            if r.len() % 2 == 0 { (r[m - 1] + r[m]) / 2.0 } else { r[m] }
        };
        ((100.0 * elapsed / (elapsed + remaining.max(1.0))).min(99.0), true, est)
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

/// Nur lokal geänderte Felder überschreiben; Sammlungen pro Pfad/Farbindex zusammenführen.
fn merge_state(base: &Value, local: &Value, disk: &Value) -> Value {
    let mut out = disk.clone();
    for (key, value) in local.as_object().unwrap() {
        if value == &base[key] { continue; }
        match key.as_str() {
            "bookmark_colors" => {
                let mut map = disk[key].as_object().cloned().unwrap_or_default();
                let empty = serde_json::Map::new();
                let old = base[key].as_object().unwrap_or(&empty);
                let new = value.as_object().unwrap_or(&empty);
                for k in old.keys().chain(new.keys()) {
                    if old.get(k) != new.get(k) {
                        if let Some(v) = new.get(k) { map.insert(k.clone(), v.clone()); }
                        else { map.remove(k); }
                    }
                }
                out[key] = Value::Object(map);
            }
            "bookmarks" => {
                let old = base[key].as_array().cloned().unwrap_or_default();
                let new = value.as_array().cloned().unwrap_or_default();
                let mut current = disk[key].as_array().cloned().unwrap_or_default();
                current.retain(|p| !old.contains(p) || new.contains(p));
                for p in new { if !old.contains(&p) && !current.contains(&p) { current.push(p); } }
                out[key] = Value::Array(current);
            }
            "category_names" => {
                let mut names = disk[key].as_array().cloned().unwrap_or_default();
                names.resize(8, Value::String(String::new()));
                for (i, name) in value.as_array().unwrap().iter().enumerate().take(8) {
                    let before = base[key].get(i).and_then(Value::as_str).unwrap_or("");
                    if name.as_str().unwrap_or("") != before { names[i] = name.clone(); }
                }
                out[key] = Value::Array(names);
            }
            _ => { out[key] = value.clone(); }
        }
    }
    out
}

fn read_state(p: &std::path::Path) -> Result<Value, String> {
    match std::fs::read(p) {
        Ok(bytes) => {
            let s: Store = serde_json::from_slice(&bytes).map_err(|e| format!("Ungültige Einstellungsdatei (bleibt unverändert): {e}"))?;
            serde_json::to_value(s).map_err(|e| e.to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::to_value(Store::default()).unwrap()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiple_instances_keep_bookmarks_colors_and_names_on_disk() {
        let dir = std::env::temp_dir().join(format!("copi-store-test-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let p = dir.join("state.json");
        let mut a = Store::default();
        let mut b = Store::default(); // ältere, gleichzeitig geöffnete Instanz
        a.toggle_bookmark("/synthetic/a"); a.set_bookmark_color("/synthetic/a", 6);
        a.set_category_name(6, "Kurs"); a.save_at(&p).unwrap();
        b.toggle_bookmark("/synthetic/b"); b.set_bookmark_color("/synthetic/b", 3);
        b.set_category_name(3, "Projekt"); b.save_at(&p).unwrap();
        // Unabhängige Einstellungsänderung darf fremde Bookmarks nicht löschen.
        a.output_size = 18.0; a.save_at(&p).unwrap();
        let mut restarted: Store = serde_json::from_value(read_state(&p).unwrap()).unwrap();
        assert!(restarted.is_bookmark("/synthetic/a") && restarted.is_bookmark("/synthetic/b"));
        assert_eq!(restarted.bookmark_color("/synthetic/a"), 6);
        assert_eq!(restarted.bookmark_color("/synthetic/b"), 3);
        assert_eq!(restarted.category_name(6), "Kurs"); assert_eq!(restarted.category_name(3), "Projekt");
        restarted.baseline = serde_json::to_value(&restarted).unwrap();
        restarted.toggle_bookmark("/synthetic/a"); restarted.save_at(&p).unwrap();
        b.net_visible = true; b.save_at(&p).unwrap();
        assert!(!b.is_bookmark("/synthetic/a"), "alte Instanz darf gelöschtes Bookmark nicht wiederbeleben");
        assert!(b.is_bookmark("/synthetic/b"));
        std::fs::write(&p, "invalid-json").unwrap();
        assert!(b.save_at(&p).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "invalid-json");
        std::fs::remove_dir_all(dir).unwrap();
    }

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
