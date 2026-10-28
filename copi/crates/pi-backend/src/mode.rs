//! Modus ohne App-Abhängigkeit. Ohne ausdrückliche Freigabe startet kein lokaler Agent.
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    RemoteOnly,
    Hybrid,
}
static HYBRID: AtomicBool = AtomicBool::new(false);
static WRITE: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct Setting {
    mode: Mode,
}

pub fn current() -> Mode {
    #[cfg(test)]
    if let Some(m) = TEST_MODE.with(|m| m.get()) {
        return m;
    }
    if HYBRID.load(Ordering::SeqCst) {
        Mode::Hybrid
    } else {
        Mode::RemoteOnly
    }
}
fn path() -> Option<std::path::PathBuf> {
    std::env::var_os("COPI_MODE_FILE")
        .map(Into::into)
        .or_else(|| crate::copi_config_dir().map(|p| p.join("mode.json")))
}
fn read_from(p: &std::path::Path) -> Mode {
    std::fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice::<Setting>(&b).ok())
        .map(|s| s.mode)
        .unwrap_or(Mode::RemoteOnly)
}
/// Vor dem ersten Frame aufrufen; unlesbare Einstellungen bleiben sicher gesperrt.
pub fn init() -> Mode {
    let _guard = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    let m = path().map(|p| read_from(&p)).unwrap_or(Mode::RemoteOnly);
    HYBRID.store(m == Mode::Hybrid, Ordering::SeqCst);
    m
}
/// Persistiert zuerst; ein Schreibfehler schaltet niemals Hybrid frei.
pub fn set(m: Mode) -> Result<(), String> {
    let _guard = WRITE.lock().map_err(|e| e.to_string())?;
    let p = path().ok_or("Kein Konfigurationsordner. Der Modus bleibt unverändert.")?;
    std::fs::create_dir_all(p.parent().ok_or("Ungültiger Einstellungspfad")?).map_err(|e| e.to_string())?;
    let tmp = p.with_extension(format!("{}.new", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        use std::io::Write;
        let mut f = options.open(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&serde_json::to_vec(&Setting { mode: m }).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
    })();
    if result.is_ok() {
        HYBRID.store(m == Mode::Hybrid, Ordering::SeqCst);
    }
    result
}

#[derive(Clone, Copy, Debug)]
pub enum SpawnPurpose {
    Chat,
    Cli,
    Version,
    Helper,
}
#[derive(Debug)]
pub struct SpawnBlocked {
    pub purpose: SpawnPurpose,
}
impl std::fmt::Display for SpawnBlocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Hier startet kein Agent. cocopi ist auf ‚Nur Remote‘ gestellt. Deine Sessions laufen auf R2.")
    }
}
impl std::error::Error for SpawnBlocked {}

// Threadlokal: parallele Tests können niemals einem anderen Test Hybrid freigeben.
#[cfg(test)]
thread_local! { static TEST_MODE: std::cell::Cell<Option<Mode>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
pub fn set_for_test(m: Mode) {
    TEST_MODE.with(|v| v.set(Some(m)));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_mode_file_is_remote_only() {
        assert_eq!(
            read_from(&std::env::temp_dir().join(format!("missing-mode-{}", std::process::id()))),
            Mode::RemoteOnly
        );
    }
    #[test]
    fn corrupt_mode_file_is_remote_only() {
        let p = std::env::temp_dir().join(format!("bad-mode-{}", std::process::id()));
        std::fs::write(&p, b"{broken").unwrap();
        assert_eq!(read_from(&p), Mode::RemoteOnly);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn remote_only_blocks_pi_command() {
        set_for_test(Mode::RemoteOnly);
        for purpose in [SpawnPurpose::Chat, SpawnPurpose::Cli, SpawnPurpose::Version, SpawnPurpose::Helper] {
            assert!(crate::winshim::pi_command(purpose, &[]).is_err());
        }
    }
    #[test]
    fn hybrid_allows_pi_command() {
        set_for_test(Mode::Hybrid);
        assert!(crate::winshim::pi_command(SpawnPurpose::Chat, &[]).is_ok()); // kein Spawn
    }
    #[test]
    fn only_winshim_spawns_pi() {
        fn scan(dir: &std::path::Path) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    if !matches!(p.file_name().and_then(|x| x.to_str()), Some("target" | "tests")) {
                        scan(&p);
                    }
                } else if p.extension().is_some_and(|e| e == "rs") && !matches!(p.file_name().and_then(|x| x.to_str()), Some("winshim.rs" | "tests.rs")) {
                    let text = std::fs::read_to_string(&p).unwrap();
                    let production = text.split("#[cfg(test)]\nmod tests").next().unwrap();
                    for line in production.lines().filter(|l| !l.trim_start().starts_with("//")) {
                        if let Some(tail) = line.split("Command::new(").nth(1) {
                            let arg = tail.split(')').next().unwrap_or("").trim().to_ascii_lowercase();
                            assert!(
                                !(arg.contains("\"pi\"")
                                    || arg.contains("/pi\"")
                                    || arg.contains("pi.exe")
                                    || arg.contains("pi_bin")
                                    || arg.contains("pi_path")),
                                "Umgehung der Spawn-Sperre: {}: {}",
                                p.display(),
                                line
                            );
                        }
                    }
                }
            }
        }
        scan(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    }
}
