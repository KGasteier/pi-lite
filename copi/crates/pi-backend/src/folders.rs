//! Arbeitsordner: vorgeschlagene Orte und Ordnerliste.
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Location {
    pub name: String,
    pub path: String,
}

/// Session, die beim Start fortgesetzt werden soll (`COPI_RESUME_SESSION` oder `PI_LITE_RESUME_SESSION`).
pub fn resume_session() -> Option<String> {
    std::env::var("COPI_RESUME_SESSION")
        .or_else(|_| std::env::var("PI_LITE_RESUME_SESSION"))
        .ok()
        .filter(|p| PathBuf::from(p).is_file())
}

pub fn work_locations() -> Vec<Location> {
    let default = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = crate::home().unwrap_or_else(|| PathBuf::from("."));
    let mut out = vec![
        Location { name: "Standard (Startordner)".into(), path: default.to_string_lossy().into() },
        Location { name: "Home".into(), path: home.to_string_lossy().into() },
    ];
    for path in shared_folder_candidates() {
        if PathBuf::from(path).is_dir() {
            out.push(Location { name: "Shared Folder".into(), path: (*path).into() });
        }
    }
    out
}

/// Bekannte Mountpunkte für geteilte Ordner in VMs/Containern. Linux-spezifische Pfade;
/// auf anderen Plattformen gibt es (noch) keine entsprechenden Konventionen hier.
#[cfg(target_os = "linux")]
fn shared_folder_candidates() -> &'static [&'static str] {
    &["/mnt/mac", "/media/sf_shared", "/mnt/shared"]
}
#[cfg(not(target_os = "linux"))]
fn shared_folder_candidates() -> &'static [&'static str] {
    &[]
}

/// Unterordner von `path` (alphabetisch) plus kanonischer Pfad und Elternordner.
pub fn list_folders(path: &str) -> Result<(String, Option<String>, Vec<Location>), String> {
    let p = if path.trim().is_empty() { std::env::current_dir().map_err(|e| e.to_string())? } else { PathBuf::from(path) };
    let p = p.canonicalize().map_err(|e| e.to_string())?;
    let mut folders = Vec::new();
    for f in std::fs::read_dir(&p).map_err(|e| e.to_string())?.flatten() {
        if f.path().is_dir() {
            folders.push(Location { name: f.file_name().to_string_lossy().into(), path: f.path().to_string_lossy().into() });
        }
    }
    folders.sort_by_key(|a| a.name.to_lowercase());
    Ok((p.to_string_lossy().into(), p.parent().map(|p| p.to_string_lossy().into()), folders))
}
