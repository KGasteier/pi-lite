use std::path::PathBuf;
use serde::Serialize;
#[derive(Serialize)]
pub struct Location { name: String, path: String }
#[derive(Serialize)]
pub struct Folders { path: String, parent: Option<String>, folders: Vec<Location> }
fn home() -> PathBuf { std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(".")) }
#[tauri::command]
pub fn resume_session() -> Option<String> { std::env::var("PI_LITE_RESUME_SESSION").ok().filter(|p| PathBuf::from(p).is_file()) }
#[tauri::command]
pub fn work_locations() -> Result<Vec<Location>, String> {
    let default = std::env::current_dir().map_err(|e| e.to_string())?;
    let mut out = vec![Location { name: "Standard (Startordner)".into(), path: default.to_string_lossy().into() }, Location { name: "Home".into(), path: home().to_string_lossy().into() }];
    for path in [PathBuf::from("/mnt/mac"), PathBuf::from("/media/sf_shared"), PathBuf::from("/mnt/shared")] {
        if path.is_dir() { out.push(Location { name: "Shared Folder".into(), path: path.to_string_lossy().into() }); }
    }
    Ok(out)
}
#[tauri::command]
pub async fn list_folders(path: String) -> Result<Folders, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = if path.trim().is_empty() { std::env::current_dir().map_err(|e| e.to_string())? } else { PathBuf::from(path) };
        let p = p.canonicalize().map_err(|e| e.to_string())?;
        let dirs = std::fs::read_dir(&p).map_err(|e| e.to_string())?;
        let mut folders = Vec::new();
        for f in dirs.flatten() {
            if f.path().is_dir() { folders.push(Location { name: f.file_name().to_string_lossy().into(), path: f.path().to_string_lossy().into() }); }
        }
        folders.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(Folders { path: p.to_string_lossy().into(), parent: p.parent().map(|p| p.to_string_lossy().into()), folders })
    }).await.map_err(|e| e.to_string())?
}
