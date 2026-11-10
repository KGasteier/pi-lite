//! Kontextmanager für pi-lite: Stichwortsuche, Kontext-Repair und Markdown-Export/-Import
//! über dieselbe Logik wie copi/cocopi (`pi_backend::search`, `pi_backend::repair`).
//! Alles läuft in Hintergrund-Threads (`spawn_blocking`), damit das Fenster nicht hängt.
//! Echte Sessions werden von Suche und Analyse nur gelesen; geschrieben wird ausschließlich
//! über `repair::apply`/`undo`/`import_md` (pi-eigene Eintragstypen, nur anhängend).
use pi_backend::repair::{self, Applied, Report};
use pi_backend::search::Corpus;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// Gemeinsamer Suchindex (nur im Speicher, inkrementell über Länge+Änderungszeit).
#[derive(Default, Clone)]
pub struct SearchState(Arc<Mutex<Corpus>>);

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Suchwörter wie in copi: kleingeschrieben, höchstens acht.
fn words(q: &str) -> Vec<String> {
    q.split_whitespace().map(str::to_lowercase).filter(|w| !w.is_empty()).take(8).collect()
}

/// Inhaltstreffer je lokaler Session: `{pfad: {counts, snippet}}`. Ob *alle* Wörter passen,
/// entscheidet die Oberfläche zusammen mit Name/Titel/Ordner.
#[tauri::command]
pub async fn session_search(query: String, paths: Vec<String>, state: tauri::State<'_, SearchState>) -> Result<Value, String> {
    let corpus = state.0.clone();
    blocking(move || {
        let w = words(&query);
        let mut c = corpus.lock().map_err(|_| "Suchindex gesperrt")?;
        c.sync(&paths);
        let hits = c.search(&w);
        Ok(Value::Object(hits.into_iter().map(|(p, h)| (p, json!({"counts": h.counts, "snippet": h.snippet}))).collect()))
    })
    .await
}

fn report_json(r: &Report) -> Value {
    json!({
        "path": r.path, "cwd": r.cwd, "len": r.len, "leaf": r.leaf,
        "entries": r.entries, "messages": r.messages, "compactions": r.compactions,
        "context_edits": r.context_edits, "context_tokens": r.context_tokens,
        "findings": r.findings.iter().map(|f| json!({
            "target": f.target, "kind": f.kind.label(), "label": f.label,
            "tokens_saved": f.tokens_saved, "replacement": f.replacement, "default_on": f.default_on,
        })).collect::<Vec<_>>(),
        "rot": r.rot.iter().map(|x| json!({"level": x.level, "text": x.text})).collect::<Vec<_>>(),
        "last_prompts": r.last_prompts, "status": r.status, "todos": r.todos,
        "modified_files": r.modified_files.iter().map(|f| pi_backend::tilde(f)).collect::<Vec<_>>(),
        "name": r.name, "suggested_title": r.suggested_title, "delete_hint": r.delete_hint,
        "overlaps": r.overlaps.iter().map(|o| json!({"path": o.path, "title": o.title, "modified": o.modified, "shared": o.shared.iter().map(|f| pi_backend::tilde(f)).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "last_activity": r.last_activity,
    })
}

/// Nur lesende Analyse einer lokalen Session.
#[tauri::command]
pub async fn repair_analyze(path: String) -> Result<Value, String> {
    blocking(move || repair::analyze(&path, true).map(|r| report_json(&r))).await
}

/// Ausgewählte Kürzungen (`[[ziel, ersatz], …]`) und optional einen Titel anhängen.
/// Bricht ab, wenn die Datei seit der Analyse verändert wurde (`len`/`leaf`).
#[tauri::command]
pub async fn repair_apply(path: String, len: u64, leaf: String, edits: Vec<(String, String)>, title: Option<String>) -> Result<Value, String> {
    blocking(move || {
        let a = repair::apply(&path, len, &leaf, &edits, title.as_deref())?;
        Ok(json!({"old_len": a.old_len, "new_len": a.new_len, "entries": a.entries}))
    })
    .await
}

/// Eben angehängten Repair abschneiden (nur solange die Datei genau so lang ist).
#[tauri::command]
pub async fn repair_undo(path: String, old_len: u64, new_len: u64, entries: usize) -> Result<(), String> {
    blocking(move || repair::undo(&path, &Applied { old_len, new_len, entries })).await
}

/// Bericht und aktiven Kontext als neue `.md` schreiben (vorhandene Dateien bleiben).
#[tauri::command]
pub async fn repair_export(path: String, target: String) -> Result<String, String> {
    blocking(move || {
        let target = pi_backend::tilde_expand(target.trim());
        if !target.to_lowercase().ends_with(".md") {
            return Err("Zielpfad muss auf .md enden.".into());
        }
        let report = repair::analyze(&path, false)?;
        let md = repair::export_md(&path, &report)?;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|e| format!("{target}: {e}"))?;
        f.write_all(md.as_bytes()).map_err(|e| e.to_string())?;
        Ok(target)
    })
    .await
}

/// `.md` (max. 4 MB) als neue lokale Session anlegen; liefert den neuen Sessionpfad.
#[tauri::command]
pub async fn import_md(path: String, cwd: String) -> Result<String, String> {
    blocking(move || repair::import_md(&pi_backend::tilde_expand(path.trim()), &cwd)).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn words_lowercase_and_capped() {
        assert_eq!(super::words("  Foo BAR "), vec!["foo", "bar"]);
        assert_eq!(super::words("a b c d e f g h i j").len(), 8);
    }
}
