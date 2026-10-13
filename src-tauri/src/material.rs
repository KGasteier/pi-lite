#[path = "../../shared/material_browser.rs"]
mod browser;
#[tauri::command]
pub async fn material_browser(path: Option<String>, by_date: bool) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = path.unwrap_or_else(|| browser::default_dir(&std::env::var("HOME").unwrap_or_else(|_| ".".into())));
        let p = std::path::PathBuf::from(&path).canonicalize().map_err(|e|e.to_string())?;
        Ok(serde_json::json!({"path":p.to_string_lossy(),"parent":p.parent().map(|p|p.to_string_lossy().into_owned()),"entries":browser::list(p.to_str().ok_or("Ungültiger Pfad")?,by_date)?}))
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn material_preview(path: String) -> Result<browser::Preview,String> {
    tauri::async_runtime::spawn_blocking(move || browser::preview(&path)).await.map_err(|e|e.to_string())?
}
use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::Path;
use std::io::Read;

#[tauri::command]
pub async fn pick_material() -> Result<Vec<String>, String> {
    Ok(rfd::AsyncFileDialog::new().set_title("Material für nächste Anfrage auswählen")
        .pick_files().await.unwrap_or_default().iter().map(|f| f.path().to_string_lossy().into()).collect())
}

#[derive(serde::Serialize)]
pub struct Prepared { pub text: String, pub images: Vec<serde_json::Value> }

fn prepare(paths: Vec<String>, references_only: bool) -> Result<Prepared, String> {
    if paths.len() > 12 { return Err("Maximal 12 Dateien pro Anfrage".into()); }
    let mut result = Prepared { text: String::new(), images: vec![] };
    let mut total = 0;
    for path in paths {
        let p = Path::new(&path).canonicalize().map_err(|e| format!("Material nicht verfügbar: {e}"))?;
        let metadata = p.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() { return Err("Material muss eine reguläre Datei sein".into()); }
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
        result.text.push_str(&format!("\nDatei: {}\n", serde_json::to_string(&p.to_string_lossy()).unwrap()));
        result.text.push_str("Originaldatei referenziert: nicht kopieren, verschieben oder in DL_Omarchy ablegen. Änderungen nur auf ausdrücklichen Auftrag.\n");
        if references_only { continue; }
        if metadata.len() > 100*1024*1024 { return Err("Material überschreitet 100 MiB; bitte als Verweis senden".into()); }
        let mime = match ext.as_str() { "png"=>Some("image/png"), "jpg"|"jpeg"=>Some("image/jpeg"), "webp"=>Some("image/webp"), _=>None };
        if let Some(mime) = mime {
            if metadata.len() > 8 * 1024 * 1024 { return Err("Bild überschreitet 8 MiB; als Verweis senden oder verkleinern".into()); }
            let mut data = Vec::new();
            std::fs::File::open(&p).map_err(|e|e.to_string())?.take(8*1024*1024+1).read_to_end(&mut data).map_err(|e|e.to_string())?;
            if data.len() > 8*1024*1024 { return Err("Bild wurde während des Lesens größer als 8 MiB".into()); }
            let valid = match mime { "image/png" => data.starts_with(b"\x89PNG\r\n\x1a\n"), "image/jpeg" => data.starts_with(&[0xff,0xd8,0xff]), "image/webp" => data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP"), _ => false };
            if !valid { return Err("Bildinhalt passt nicht zum Dateiformat".into()); }
            total += data.len();
            if total > 20*1024*1024 { return Err("Bildmaterial überschreitet insgesamt 20 MiB".into()); }
            result.images.push(serde_json::json!({"type":"image","data":STANDARD.encode(data),"mimeType":mime}));
        } else if ext == "pdf" {
            // Local extraction, no network/provider call. Limit output rather than loading an entire PDF.
            let mut child = std::process::Command::new("pdftotext").args(["-f","1","-l","30","--"])
                .arg(&p).arg("-").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null())
                .spawn().map_err(|e| format!("PDF-Extraktion benötigt pdftotext: {e}"))?;
            let mut bytes = Vec::new();
            child.stdout.take().unwrap().take(60_001).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
            let truncated = bytes.len()>60_000;
            if truncated { let _=child.kill(); }
            let status=child.wait().map_err(|e|e.to_string())?;
            if !truncated && !status.success() { return Err("PDF-Text konnte nicht extrahiert werden; als Verweis senden möglich".into()); }
            bytes.truncate(60_000);
            result.text.push_str(&format!("Lokaler PDF-Auszug (max. 30 Seiten / 60000 Bytes, nicht vollständiger Inhalt):\n{}\nEnde PDF-Auszug.\n",String::from_utf8_lossy(&bytes)));
        } else if ["mp3","wav","m4a","ogg","flac","mp4","mkv","mov","webm"].contains(&ext.as_str()) {
            return Err("Audio/Video: noch kein verifizierter Analysedienst eingerichtet. Unter Details „Nur Dateiverweise“ wählen oder zuerst ein Transkript hinzufügen.".into());
        }
    }
    Ok(result)
}
#[tauri::command]
pub async fn prepare_material(paths: Vec<String>, references_only: bool) -> Result<Prepared,String> {
    tauri::async_runtime::spawn_blocking(move || prepare(paths,references_only)).await.map_err(|e|e.to_string())?
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn prepares_images_and_references_without_upload() {
        let dir = std::env::temp_dir().join(format!("pi-lite-material-{}",std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("image.png");
        std::fs::write(&image,b"\x89PNG\r\n\x1a\nsynthetic").unwrap();
        let paths=vec![image.to_string_lossy().into_owned()];
        let prepared=prepare(paths.clone(),false).unwrap();
        assert_eq!(prepared.images.len(),1);
        assert_eq!(prepared.images[0]["mimeType"],"image/png");
        assert!(prepared.text.contains("image.png"));
        assert!(prepare(paths,true).unwrap().images.is_empty());
        std::fs::write(&image,b"not an image").unwrap();
        assert!(prepare(vec![image.to_string_lossy().into_owned()],false).is_err());
        let audio=dir.join("audio.wav"); std::fs::write(&audio,b"synthetic").unwrap();
        let paths=vec![audio.to_string_lossy().into_owned()];
        assert!(prepare(paths.clone(),false).is_err());
        assert!(prepare(paths,true).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn rejects_missing_and_limits() {
        assert!(prepare(vec!["/missing-pi-lite-material".into()],false).is_err());
        assert!(prepare(vec!["x".into();13],true).is_err());
    }
}
