//! Gemeinsame Bookmark-Ablage beider Frontends. Keine Sessioninhalte, keine Netzwerkzugriffe.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Anzahl der Kategorienamen = Länge der Bookmark-Palette (10; 8 und 9 wurden angehängt).
pub const CATEGORY_COUNT: usize = 10;
pub fn empty() -> Value { json!({"bookmarks":[],"bookmark_colors":{},"category_names":[]}) }
pub fn path() -> Result<PathBuf,String> {
    #[cfg(target_os="macos")]
    let root = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(target_os="windows")]
    let root = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(any(target_os="macos",target_os="windows")))]
    let root = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()).map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    Ok(root.ok_or("Kein Konfigurationsordner")?.join("pi-frontends/bookmarks.json"))
}
pub fn read_at(p: &Path) -> Result<Value,String> {
    match std::fs::read(p) {
        Ok(b) => { let v:Value=serde_json::from_slice(&b).map_err(|e|format!("Bookmark-Datei ungültig: {e}"))?;
            if !v["bookmarks"].is_array() || !v["bookmark_colors"].is_object() || !v["category_names"].is_array() { return Err("Ungültiges Bookmark-Format".into()); } Ok(v) }
        Err(e) if e.kind()==std::io::ErrorKind::NotFound => Ok(empty()),
        Err(e)=>Err(e.to_string()),
    }
}
pub fn read() -> Result<Value,String> { read_at(&path()?) }
/// Delta pro Session/Farbindex, nicht vollständigen Altstand überschreiben.
pub fn merge(base:&Value, local:&Value, disk:&Value) -> Value {
    let mut out=disk.clone();
    let old=base["bookmarks"].as_array().cloned().unwrap_or_default();
    let new=local["bookmarks"].as_array().cloned().unwrap_or_default();
    let mut paths=disk["bookmarks"].as_array().cloned().unwrap_or_default();
    paths.retain(|p|!old.contains(p)||new.contains(p));
    for p in new { if !old.contains(&p)&&!paths.contains(&p) { paths.push(p); } }
    out["bookmarks"]=Value::Array(paths);
    let mut colors=disk["bookmark_colors"].as_object().cloned().unwrap_or_default();
    let a=base["bookmark_colors"].as_object().cloned().unwrap_or_default();
    let b=local["bookmark_colors"].as_object().cloned().unwrap_or_default();
    for k in a.keys().chain(b.keys()) { if a.get(k)!=b.get(k) { if let Some(v)=b.get(k) { colors.insert(k.clone(),v.clone()); } else { colors.remove(k); } } }
    colors.retain(|k,_|out["bookmarks"].as_array().unwrap().contains(&Value::String(k.clone())));
    out["bookmark_colors"]=Value::Object(colors);
    let mut names=disk["category_names"].as_array().cloned().unwrap_or_default();
    // Nur verlängern, nie kürzen: Namen künftiger Paletteneinträge bleiben erhalten.
    if names.len()<CATEGORY_COUNT { names.resize(CATEGORY_COUNT,Value::String(String::new())); }
    for (i,n) in local["category_names"].as_array().into_iter().flatten().enumerate().take(CATEGORY_COUNT) {
        if n.as_str().unwrap_or("") != base["category_names"].get(i).and_then(Value::as_str).unwrap_or("") { names[i]=n.clone(); }
    }
    out["category_names"]=Value::Array(names); out
}
pub fn update(base:&Value, local:&Value) -> Result<Value,String> { update_at(&path()?,base,local,false) }
/// Einmalige Übernahme: bestehende gemeinsame Zuordnungen haben Vorrang.
pub fn seed(local:&Value) -> Result<Value,String> { update_at(&path()?,&empty(),local,true) }
pub fn update_at(p:&Path,base:&Value,local:&Value,seed:bool) -> Result<Value,String> {
    std::fs::create_dir_all(p.parent().ok_or("Kein Elternordner")?).map_err(|e|e.to_string())?;
    let lock=std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(p.with_extension("lock")).map_err(|e|e.to_string())?;
    lock.lock().map_err(|e|e.to_string())?;
    let disk=read_at(p)?;
    let mut out=if seed { merge(&empty(),&disk,&merge(&empty(),local,&empty())) } else { merge(base,local,&disk) };
    if seed {
        for p in disk["bookmarks"].as_array().unwrap() {
            if let Some(k)=p.as_str() { if disk["bookmark_colors"].get(k).is_none() { out["bookmark_colors"].as_object_mut().unwrap().remove(k); } }
        }
    }
    let tmp=p.with_extension(format!("{}.tmp",std::process::id()));
    use std::io::Write;
    let mut f=std::fs::File::create(&tmp).map_err(|e|e.to_string())?;
    f.write_all(&serde_json::to_vec_pretty(&out).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    f.sync_all().map_err(|e|e.to_string())?;
    std::fs::rename(tmp,p).map_err(|e|e.to_string())?; Ok(out)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn cross_frontend_delta_and_restart() {
        let dir=std::env::temp_dir().join(format!("shared-bookmarks-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let p=dir.join("bookmarks.json");
        let a=json!({"bookmarks":["/synthetic/a"],"bookmark_colors":{"/synthetic/a":6},"category_names":["Kurs"]});
        let b=json!({"bookmarks":["/synthetic/b"],"bookmark_colors":{"/synthetic/b":3},"category_names":[]});
        let saved=update_at(&p,&empty(),&a,false).unwrap();
        update_at(&p,&empty(),&b,false).unwrap();
        let current=read_at(&p).unwrap(); assert_eq!(current["bookmarks"].as_array().unwrap().len(),2);
        assert_eq!(current["bookmark_colors"]["/synthetic/a"],6);
        let mut changed=saved.clone(); changed["bookmarks"]=json!([]); changed["bookmark_colors"]=json!({});
        update_at(&p,&saved,&changed,false).unwrap();
        assert_eq!(read_at(&p).unwrap()["bookmarks"],json!(["/synthetic/b"]));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn ten_colors_and_old_eight_name_files() {
        // Regression 8 -> 10 Farben: alte Datei mit 8 Namen wird verlängert, Index 9 bleibt erhalten.
        let disk=json!({"bookmarks":["/s/a"],"bookmark_colors":{"/s/a":7},"category_names":["a","b","c","d","e","f","g","h"]});
        let mut local=disk.clone(); local["bookmark_colors"]["/s/a"]=json!(9);
        let mut names=disk["category_names"].as_array().unwrap().clone(); names.resize(CATEGORY_COUNT,json!("")); names[9]=json!("Neu"); local["category_names"]=Value::Array(names);
        let out=merge(&disk,&local,&disk);
        assert_eq!(out["bookmark_colors"]["/s/a"],9);
        let n=out["category_names"].as_array().unwrap();
        assert_eq!(n.len(),CATEGORY_COUNT); assert_eq!(n[0],"a"); assert_eq!(n[9],"Neu");
    }
}
