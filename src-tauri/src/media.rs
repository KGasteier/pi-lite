use std::io::Write;
use std::process::{Command,Stdio};
fn run(job: serde_json::Value) -> Result<serde_json::Value,String> {
    let mut child=Command::new("python3").args(["-c",include_str!("../../scripts/media-worker.py")])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
    child.stdin.take().unwrap().write_all(job.to_string().as_bytes()).map_err(|e|e.to_string())?;
    let output=child.wait_with_output().map_err(|e|e.to_string())?;
    let result:serde_json::Value=serde_json::from_slice(&output.stdout).map_err(|_|"Medienprozess lieferte keine gültige Antwort")?;
    if result["success"]==true {Ok(result["data"].clone())} else {Err(result["error"].as_str().unwrap_or("Medienanalyse fehlgeschlagen").into())}
}
#[tauri::command]
pub async fn media_routes() -> Result<serde_json::Value,String> {
    tauri::async_runtime::spawn_blocking(||run(serde_json::json!({"action":"routes"}))).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn analyze_media(paths:Vec<String>,question:String,route:Option<String>) -> Result<serde_json::Value,String> {
    tauri::async_runtime::spawn_blocking(move||run(serde_json::json!({"paths":paths,"question":question,"route":route}))).await.map_err(|e|e.to_string())?
}
