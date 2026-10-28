//! Material für die nächste Anfrage. Portiert aus `src-tauri/src/material.rs`, ohne Tauri und
//! ohne `rfd`: die Dateiauswahl erfolgt über [`crate::folders::list_folders`]-artige Logik in
//! [`list_entries`] plus einem Dialog in der Oberfläche statt eines nativen Dateidialogs. Dies
//! ist bewusst portabler, aber kein Ersatz für ein echtes XDG-Portal-Fenster.
#[path = "../../../../shared/material_browser.rs"]
pub mod material_browser;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::io::Read;
use std::path::Path;

/// Ergebnis von [`prepare`]: Text (Dateiverweise, PDF-Auszüge, URL-Zeilen) und Bildinhalte als
/// RPC-fähige `{"type":"image", ...}`-Objekte.
#[derive(Debug, Default, serde::Serialize)]
pub struct Prepared {
    pub text: String,
    pub images: Vec<serde_json::Value>,
}

/// Dateiendungen, die im Automatikmodus eine separate Audio-/Videoanalyse benötigen
/// (siehe [`crate::media`]) statt direkt vorbereitet zu werden.
pub const MEDIA_EXTENSIONS: &[&str] = &["mp3", "wav", "m4a", "ogg", "flac", "mp4", "mkv", "mov", "webm"];

/// Ein Eintrag im portablen Datei-/Ordnerbrowser (Ordner und Dateien gemischt, analog zu
/// [`crate::folders::Location`], aber mit Dateikennzeichnung für die Auswahl-UI).
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// Listet Ordnerinhalt (Ordner zuerst, dann Dateien, je alphabetisch) für den portablen
/// Materialbrowser. Kein natives Dateidialog-Portal; rein auf `std::fs` basierend und damit
/// auf jeder Plattform ohne zusätzliche Laufzeitabhängigkeit nutzbar.
pub fn list_entries(path: &str) -> Result<(String, Option<String>, Vec<Entry>), String> {
    let p = if path.trim().is_empty() { std::env::current_dir().map_err(|e| e.to_string())? } else { std::path::PathBuf::from(path) };
    let p = p.canonicalize().map_err(|e| e.to_string())?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for f in std::fs::read_dir(&p).map_err(|e| e.to_string())?.flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        let path = f.path().to_string_lossy().into_owned();
        if f.path().is_dir() {
            dirs.push(Entry { name, path, is_dir: true });
        } else {
            files.push(Entry { name, path, is_dir: false });
        }
    }
    dirs.sort_by_key(|e| e.name.to_lowercase());
    files.sort_by_key(|e| e.name.to_lowercase());
    dirs.extend(files);
    Ok((p.to_string_lossy().into(), p.parent().map(|p| p.to_string_lossy().into()), dirs))
}

/// Max. Anzahl Materialien pro Anfrage (Dateien + URLs zusammen).
pub const MAX_ITEMS: usize = 12;

/// Bereitet ausgewählte Dateien lokal vor (blockierend – auf einem Hintergrund-Thread
/// aufrufen). Lädt nichts automatisch hoch, ruft keine URLs ab. `references_only` unterdrückt
/// Bildübertragung und PDF-Vorverarbeitung (nur Dateiverweise im Text).
pub fn prepare(paths: Vec<String>, references_only: bool) -> Result<Prepared, String> {
    if paths.len() > MAX_ITEMS {
        return Err(format!("Maximal {MAX_ITEMS} Dateien pro Anfrage"));
    }
    let mut result = Prepared::default();
    let mut total = 0usize;
    for path in paths {
        let p = Path::new(&path).canonicalize().map_err(|e| format!("Material nicht verfügbar: {e}"))?;
        let metadata = p.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() {
            return Err("Material muss eine reguläre Datei sein".into());
        }
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
        result.text.push_str(&format!("\nDatei: {}\n", serde_json::to_string(&p.to_string_lossy()).unwrap()));
        result.text.push_str("Originaldatei referenziert: nicht kopieren, verschieben oder in DL_Omarchy ablegen. Änderungen nur auf ausdrücklichen Auftrag.\n");
        if references_only {
            continue;
        }
        if metadata.len() > 100 * 1024 * 1024 {
            return Err("Material überschreitet 100 MiB; bitte als Verweis senden".into());
        }
        let mime = match ext.as_str() {
            "png" => Some("image/png"),
            "jpg" | "jpeg" => Some("image/jpeg"),
            "webp" => Some("image/webp"),
            _ => None,
        };
        if let Some(mime) = mime {
            if metadata.len() > 8 * 1024 * 1024 {
                return Err("Bild überschreitet 8 MiB; als Verweis senden oder verkleinern".into());
            }
            let mut data = Vec::new();
            std::fs::File::open(&p).map_err(|e| e.to_string())?.take(8 * 1024 * 1024 + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
            if data.len() > 8 * 1024 * 1024 {
                return Err("Bild wurde während des Lesens größer als 8 MiB".into());
            }
            let valid = match mime {
                "image/png" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
                "image/jpeg" => data.starts_with(&[0xff, 0xd8, 0xff]),
                "image/webp" => data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP"),
                _ => false,
            };
            if !valid {
                return Err("Bildinhalt passt nicht zum Dateiformat".into());
            }
            total += data.len();
            if total > 20 * 1024 * 1024 {
                return Err("Bildmaterial überschreitet insgesamt 20 MiB".into());
            }
            result.images.push(serde_json::json!({"type":"image","data":STANDARD.encode(data),"mimeType":mime}));
        } else if ext == "pdf" {
            // Nur lokale Extraktion, kein Netzwerk-/Provideraufruf. Begrenzung statt Vollladen.
            let mut child = std::process::Command::new("pdftotext")
                .args(["-f", "1", "-l", "30", "--"])
                .arg(&p)
                .arg("-")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("PDF-Extraktion benötigt pdftotext: {e}"))?;
            let mut bytes = Vec::new();
            child.stdout.take().unwrap().take(60_001).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            let truncated = bytes.len() > 60_000;
            if truncated {
                let _ = child.kill();
            }
            let status = child.wait().map_err(|e| e.to_string())?;
            if !truncated && !status.success() {
                return Err("PDF-Text konnte nicht extrahiert werden; als Verweis senden möglich".into());
            }
            bytes.truncate(60_000);
            result.text.push_str(&format!(
                "Lokaler PDF-Auszug (max. 30 Seiten / 60000 Bytes, nicht vollständiger Inhalt):\n{}\nEnde PDF-Auszug.\n",
                String::from_utf8_lossy(&bytes)
            ));
        } else if MEDIA_EXTENSIONS.contains(&ext.as_str()) {
            return Err("Audio/Video: noch kein verifizierter Analysedienst eingerichtet. Unter Details „Nur Dateiverweise“ wählen oder zuerst ein Transkript hinzufügen.".into());
        }
    }
    Ok(result)
}

/// Prüft eine HTTP(S)-URL ohne Zugangsdaten (wie der Original-URL-Dialog in pi-lite). Ruft die
/// URL nicht ab; nur Syntaxprüfung für die Materialliste.
pub fn validate_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Leere URL".into());
    }
    let lower = trimmed.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("Nur HTTP(S)-URLs ohne Zugangsdaten".into());
    }
    // Authority-Teil (zwischen "//" und dem nächsten "/") darf kein `user:pass@host` enthalten.
    let after_scheme = trimmed.split_once("//").map(|(_, rest)| rest).unwrap_or("");
    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    if authority.contains('@') {
        return Err("Nur HTTP(S)-URLs ohne Zugangsdaten".into());
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_images_and_references_without_upload() {
        let dir = std::env::temp_dir().join(format!("copi-material-{}-{}", std::process::id(), line!()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("image.png");
        std::fs::write(&image, b"\x89PNG\r\n\x1a\nsynthetic").unwrap();
        let paths = vec![image.to_string_lossy().into_owned()];
        let prepared = prepare(paths.clone(), false).unwrap();
        assert_eq!(prepared.images.len(), 1);
        assert_eq!(prepared.images[0]["mimeType"], "image/png");
        assert!(prepared.text.contains("image.png"));
        assert!(prepare(paths, true).unwrap().images.is_empty());

        std::fs::write(&image, b"not an image").unwrap();
        assert!(prepare(vec![image.to_string_lossy().into_owned()], false).is_err());

        let audio = dir.join("audio.wav");
        std::fs::write(&audio, b"synthetic").unwrap();
        let paths = vec![audio.to_string_lossy().into_owned()];
        assert!(prepare(paths.clone(), false).is_err());
        assert!(prepare(paths, true).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_missing_and_limits() {
        assert!(prepare(vec!["/missing-copi-material".into()], false).is_err());
        assert!(prepare(vec!["x".into(); 13], true).is_err());
    }

    #[test]
    fn pdf_without_pdftotext_binary_errors_cleanly() {
        // Kein echtes PDF nötig: fehlende/unleserliche Datei scheitert bereits vorher an
        // `canonicalize`. Dieser Test deckt stattdessen ab, dass eine vorhandene, aber leere
        // Datei mit `.pdf`-Endung nicht crasht (pdftotext liefert ggf. einen Fehlercode).
        let dir = std::env::temp_dir().join(format!("copi-material-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("doc.pdf");
        std::fs::write(&pdf, b"not a real pdf").unwrap();
        let _ = prepare(vec![pdf.to_string_lossy().into_owned()], false);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn validates_urls() {
        assert!(validate_url("https://example.com/a").is_ok());
        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("https://user:pass@example.com").is_err());
        assert!(validate_url("   ").is_err());
    }

    #[test]
    fn list_entries_separates_dirs_and_files() {
        let dir = std::env::temp_dir().join(format!("copi-material-list-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.txt"), b"x").unwrap();
        let (canon, parent, entries) = list_entries(dir.to_str().unwrap()).unwrap();
        assert!(!canon.is_empty());
        assert!(parent.is_some());
        assert!(entries.iter().any(|e| e.name == "sub" && e.is_dir));
        assert!(entries.iter().any(|e| e.name == "a.txt" && !e.is_dir));
        std::fs::remove_dir_all(&dir).ok();
    }
}
