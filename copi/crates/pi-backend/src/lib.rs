//! pi-backend – GUI-freie Bausteine für pi-Frontends.
//!
//! Aus pi-lite (`src-tauri`) übernommen und von Tauri gelöst: Statt Events zu
//! emittieren, schreiben Lese-Threads [`Event`]s in einen `mpsc`-Kanal und rufen
//! danach einen `notify`-Callback auf (z. B. `ctx.request_repaint()`).

pub mod backends;
pub mod cli;
pub mod editor;
pub mod folders;
pub mod instances;
pub mod inventory;
pub mod material;
pub mod media;
pub mod preview;
pub mod rpc;
pub mod sessions;
pub mod ssh;
#[cfg(test)]
mod test_support;
pub mod winshim;

/// Alles, was aus Hintergrund-Threads zurück an die Oberfläche geht.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// Eine JSONL-Zeile von `pi --mode rpc` (Generation des Prozesses).
    Line(u64, String),
    /// stderr-Zeile von pi.
    Stderr(u64, String),
    /// Der pi-Prozess dieser Generation hat sich beendet.
    Exit(u64),
    /// Ausgabezeile eines `pi <unterbefehl>`-Laufs.
    CliLine { id: String, stderr: bool, line: String },
    /// `pi <unterbefehl>` ist fertig (Exit-Code oder Fehlertext).
    CliDone { id: String, result: Result<i32, String> },
}

/// Rückruf nach jedem gesendeten Event (thread-sicher).
pub type Notify = std::sync::Arc<dyn Fn() + Send + Sync>;

/// Sender plus Weckruf – wird an Hintergrund-Threads weitergereicht.
#[derive(Clone)]
pub struct Sink {
    tx: std::sync::mpsc::Sender<Event>,
    notify: Notify,
}

impl Sink {
    pub fn new(tx: std::sync::mpsc::Sender<Event>, notify: Notify) -> Self {
        Self { tx, notify }
    }
    pub fn send(&self, ev: Event) {
        if self.tx.send(ev).is_ok() {
            (self.notify)();
        }
    }
}

/// Agent-Verzeichnis von pi (`PI_CODING_AGENT_DIR` oder `~/.pi/agent`).
pub fn agent_dir() -> Option<std::path::PathBuf> {
    if let Ok(d) = std::env::var("PI_CODING_AGENT_DIR") {
        return Some(d.into());
    }
    home().map(|h| h.join(".pi/agent"))
}

pub fn home() -> Option<std::path::PathBuf> {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok().map(Into::into)
}

/// `~/…` zu einem absoluten Pfad auflösen.
pub fn tilde_expand(path: &str) -> String {
    match (path.strip_prefix("~/"), home()) {
        (Some(rest), Some(h)) => h.join(rest).to_string_lossy().into(),
        _ => path.to_string(),
    }
}

/// `~` für das Home-Verzeichnis einsetzen (Anzeige).
pub fn tilde(path: &str) -> String {
    match home() {
        Some(h) => {
            let h = h.to_string_lossy();
            match path.strip_prefix(h.as_ref()) {
                Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
                _ => path.to_string(),
            }
        }
        None => path.to_string(),
    }
}

/// Plattform-Konfigurationswurzel (`~/.config` unter Linux/BSD via XDG-Konvention,
/// `~/Library/Application Support` unter macOS, `%APPDATA%` unter Windows) über die
/// `dirs`-Hilfsbibliothek. **Verifikationsstatus (siehe `docs/portability.md` für Details):**
/// Linux-Pfad ist in diesem Repository tatsächlich gebaut und per Testlauf verifiziert
/// (`cargo test`, `HOME`-Override). macOS/Windows-Zweige sind von `dirs` bedingt kompilierter,
/// dokumentierter Code – in dieser Umgebung nicht ausführbar und daher nicht lokal
/// nachgewiesen, sondern auf die Korrektheit der `dirs`-Crate (weit verbreitet, aktiv
/// gewartet) vertraut. Fällt auf `$HOME/.config` zurück, falls `dirs::config_dir()` nichts
/// liefert (z. B. Minimalumgebung ohne von `dirs` erkannte Plattformkonvention).
pub fn config_base_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().or_else(|| home().map(|h| h.join(".config")))
}

/// Plattform-Datenwurzel (`~/.local/share` unter Linux via XDG, `~/Library/Application
/// Support` unter macOS, `%APPDATA%` unter Windows) über `dirs::data_dir()`. Gleicher
/// Verifikationsstatus wie [`config_base_dir`]: Linux lokal ausgeführt/getestet;
/// macOS/Windows sind bedingte Kompilierungspfade der `dirs`-Crate, nicht hier nachgestellt.
/// Fällt auf `$HOME/.local/share` zurück, falls `dirs` nichts liefert.
pub fn data_base_dir() -> Option<std::path::PathBuf> {
    dirs::data_dir().or_else(|| home().map(|h| h.join(".local/share")))
}

/// Bevorzugtes Konfigurationsverzeichnis von copi (`<config_base_dir>/copi`). Ziel für alle
/// Neuanlagen; niemals der alte `pi-lite`-Pfad.
pub fn copi_config_dir() -> Option<std::path::PathBuf> {
    config_base_dir().map(|d| d.join("copi"))
}

/// Bevorzugtes Datenverzeichnis von copi (`<data_base_dir>/copi`).
pub fn copi_data_dir() -> Option<std::path::PathBuf> {
    data_base_dir().map(|d| d.join("copi"))
}

/// Plattform-Cache-Wurzel (`~/.cache` unter Linux via XDG, `~/Library/Caches` unter macOS,
/// `%LOCALAPPDATA%` unter Windows) über `dirs::cache_dir()`. Gleicher Verifikationsstatus wie
/// [`config_base_dir`]: der Linux-Pfad wird hier tatsächlich gebaut und getestet, die übrigen
/// sind dokumentierte, bedingt kompilierte Zweige. Fällt auf `$HOME/.cache` zurück.
pub fn cache_base_dir() -> Option<std::path::PathBuf> {
    dirs::cache_dir().or_else(|| home().map(|h| h.join(".cache")))
}

/// Bevorzugtes Cache-Verzeichnis von copi (`<cache_base_dir>/copi`).
pub fn copi_cache_dir() -> Option<std::path::PathBuf> {
    cache_base_dir().map(|d| d.join("copi"))
}

/// Alter, vor der Umbenennung von `pi-lite` zu `copi` genutzter Konfigurationspfad
/// (`$HOME/.config/pi-lite`, unabhängig von Plattformkonvention, wie historisch fest
/// verdrahtet angelegt). Nur zum lesenden Rückfall – niemals Ziel neuer Schreibvorgänge und
/// keine automatische Migration.
pub fn legacy_pi_lite_config_dir() -> Option<std::path::PathBuf> {
    home().map(|h| h.join(".config/pi-lite"))
}

/// Alter Datenpfad vor der Umbenennung (`$HOME/.local/share/pi-lite`). Nur lesender Rückfall.
pub fn legacy_pi_lite_data_dir() -> Option<std::path::PathBuf> {
    home().map(|h| h.join(".local/share/pi-lite"))
}

/// Löst eine Konfigurationsdatei relativ zu `rel` auf: bevorzugt
/// `<copi_config_dir>/<rel>`, falls dort vorhanden; sonst – nur lesend, keine Migration –
/// der alte `<legacy_pi_lite_config_dir>/<rel>`-Pfad, falls dieser existiert; sonst der
/// bevorzugte copi-Pfad (auch wenn er noch nicht existiert), damit Aufrufer dort neu anlegen
/// können.
pub fn resolve_config_file(rel: &str) -> Option<std::path::PathBuf> {
    let preferred = copi_config_dir().map(|d| d.join(rel));
    if let Some(p) = &preferred {
        if p.is_file() {
            return Some(p.clone());
        }
    }
    if let Some(legacy) = legacy_pi_lite_config_dir().map(|d| d.join(rel)) {
        if legacy.is_file() {
            return Some(legacy);
        }
    }
    preferred
}

/// Löst ein Datenverzeichnis relativ zu `rel` auf: bevorzugt `<copi_data_dir>/<rel>`;
/// existiert dort nichts, aber unter dem alten `pi-lite`-Pfad, wird dieser nur lesend als
/// Rückfall verwendet (keine automatische Migration). Für Neuanlagen wird stets der
/// bevorzugte copi-Pfad zurückgegeben.
pub fn resolve_data_dir(rel: &str) -> Option<std::path::PathBuf> {
    let preferred = copi_data_dir().map(|d| d.join(rel));
    if let Some(p) = &preferred {
        if p.is_dir() {
            return Some(p.clone());
        }
    }
    if let Some(legacy) = legacy_pi_lite_data_dir().map(|d| d.join(rel)) {
        if legacy.is_dir() {
            return Some(legacy);
        }
    }
    preferred
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::EnvGuard;

    /// Setzt `HOME` (und entfernt `XDG_CONFIG_HOME`/`XDG_DATA_HOME`) für die Dauer von `f`
    /// über den crate-weiten [`EnvGuard`] – serialisiert gegen *alle* anderen env-mutierenden
    /// Tests im gesamten Crate (nicht nur innerhalb dieses Moduls), und stellt die alten
    /// Werte auch bei einem Panic in `f` zuverlässig wieder her.
    fn with_temp_home<T>(f: impl FnOnce(&std::path::Path) -> T) -> T {
        let home = std::env::temp_dir().join(format!(
            "pi-backend-lib-home-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&home).unwrap();
        let _guard = EnvGuard::temp_home(&home);
        let r = f(&home);
        std::fs::remove_dir_all(&home).ok();
        r
    }

    #[test]
    fn copi_config_dir_prefers_new_path() {
        with_temp_home(|home| {
            let dir = copi_config_dir().unwrap();
            assert_eq!(dir, home.join(".config/copi"));
        });
    }

    #[test]
    fn resolve_config_file_prefers_copi_over_legacy() {
        with_temp_home(|home| {
            let legacy = home.join(".config/pi-lite");
            std::fs::create_dir_all(&legacy).unwrap();
            std::fs::write(legacy.join("x.json"), "legacy").unwrap();
            // Nur legacy vorhanden -> Rückfall.
            let resolved = resolve_config_file("x.json").unwrap();
            assert_eq!(resolved, legacy.join("x.json"));

            let preferred_dir = home.join(".config/copi");
            std::fs::create_dir_all(&preferred_dir).unwrap();
            std::fs::write(preferred_dir.join("x.json"), "new").unwrap();
            // Beide vorhanden -> der neue copi-Pfad gewinnt.
            let resolved = resolve_config_file("x.json").unwrap();
            assert_eq!(resolved, preferred_dir.join("x.json"));
        });
    }

    #[test]
    fn resolve_config_file_falls_back_to_preferred_when_neither_exists() {
        with_temp_home(|home| {
            let resolved = resolve_config_file("missing.json").unwrap();
            assert_eq!(resolved, home.join(".config/copi/missing.json"));
        });
    }
}
