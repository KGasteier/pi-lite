//! Lokaler Materialbrowser und begrenzte Vorschauen, gemeinsam für beide Frontends.
use std::{path::{Path, PathBuf}, io::Read};
use base64::{engine::general_purpose::STANDARD, Engine};
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Entry { pub name: String, pub path: String, pub is_dir: bool, pub modified: u64 }
#[derive(serde::Serialize)]
pub struct Preview { pub text: String, pub png: Option<String> }
pub fn default_dir(fallback: &str) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let mut candidates = Vec::new();
    if let Some(h) = home { candidates.push(h.join("DL_Omarchy")); candidates.push(h.join("Data/DL_Omarchy")); }
    candidates.push(PathBuf::from("/mnt/mac/DL_Omarchy"));
    candidates.into_iter().find(|p| p.is_dir()).unwrap_or_else(|| PathBuf::from(fallback)).to_string_lossy().into()
}
pub fn list(path: &str, by_date: bool) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for f in std::fs::read_dir(path).map_err(|e|e.to_string())? {
        let f = f.map_err(|e|e.to_string())?;
        let m = f.metadata().map_err(|e|e.to_string())?;
        entries.push(Entry { name:f.file_name().to_string_lossy().into(), path:f.path().to_string_lossy().into(), is_dir:m.is_dir(), modified:m.modified().ok().and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|t|t.as_secs()).unwrap_or(0) });
    }
    entries.sort_by(|a,b| b.is_dir.cmp(&a.is_dir).then_with(|| if by_date {b.modified.cmp(&a.modified)} else {std::cmp::Ordering::Equal}).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}
fn bounded(path: &Path, limit:u64) -> Result<Vec<u8>,String> {
    let mut data=Vec::new(); std::fs::File::open(path).map_err(|e|e.to_string())?.take(limit+1).read_to_end(&mut data).map_err(|e|e.to_string())?;
    if data.len() as u64 > limit {return Err("Datei zu groß für lokale Vorschau".into());} Ok(data)
}
pub fn preview(path:&str) -> Result<Preview,String> {
    let p=Path::new(path); let ext=p.extension().and_then(|s|s.to_str()).unwrap_or("").to_lowercase();
    if !p.is_file() {return Err("Keine reguläre Datei".into());}
    if ext=="png" {let bytes=bounded(p,8*1024*1024)?;return Ok(Preview{text:"Lokale Bildvorschau · Original unverändert".into(),png:Some(STANDARD.encode(bytes))});}
    if ext=="pdf" || ["jpg","jpeg","webp"].contains(&ext.as_str()) {
        if p.metadata().map_err(|e|e.to_string())?.len()>100*1024*1024 {return Err("Vorschau auf 100 MiB begrenzt".into());}
        // Eindeutiger privater temporärer Arbeitsordner, immer entfernen.
        static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
        let dir=std::env::temp_dir().join(format!("material-preview-{}-{}",std::process::id(),NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));
        std::fs::create_dir(&dir).map_err(|e|e.to_string())?;
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&dir,std::fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;}
        let result=(|| {
            let output=dir.join("page.png");
            let mut cmd=if ext=="pdf" {let mut c=std::process::Command::new("pdftoppm");c.args(["-f","1","-singlefile","-scale-to","1200","-png"]).arg(p).arg(dir.join("page"));c} else {let mut c=std::process::Command::new("magick");c.arg(p).args(["-thumbnail","1200x1200>"]).arg(&output);c};
            let mut child=cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|e|format!("Vorschau benötigt {}: {e}",if ext=="pdf" {"pdftoppm (Poppler)"} else {"ImageMagick"}))?;
            let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
            loop {if let Some(status)=child.try_wait().map_err(|e|e.to_string())? {if !status.success(){return Err("Lokale Vorschau konnte nicht erzeugt werden".into());}break;}
                if std::time::Instant::now()>deadline {let _=child.kill();let _=child.wait();return Err("Vorschau-Zeitlimit (10 s) erreicht".into());} std::thread::sleep(std::time::Duration::from_millis(30));}
            Ok(Preview{text:if ext=="pdf" {"PDF · lokale Vorschau der ersten Seite".into()} else {"Lokale Bildvorschau".into()},png:Some(STANDARD.encode(bounded(&output,8*1024*1024)?))})
        })();
        let _=std::fs::remove_dir_all(dir);return result;
    }
    let mut bytes=Vec::new();std::fs::File::open(p).map_err(|e|e.to_string())?.take(60_001).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.contains(&0) {return Err("Für diesen Binärdateityp gibt es keine lokale Vorschau".into());}
    let truncated=bytes.len()>60_000;bytes.truncate(60_000);
    Ok(Preview{text:format!("{}{}",String::from_utf8_lossy(&bytes),if truncated {"\n[Vorschau gekürzt]"} else {""}),png:None})
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn local_preview_leaves_original_unchanged(){let dir=std::env::temp_dir().join(format!("material-test-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();let p=dir.join("test.md");std::fs::write(&p,b"# Original").unwrap();assert_eq!(preview(p.to_str().unwrap()).unwrap().text,"# Original");assert_eq!(std::fs::read(&p).unwrap(),b"# Original");assert_eq!(list(dir.to_str().unwrap(),true).unwrap().len(),1);std::fs::remove_dir_all(dir).unwrap();}
}
