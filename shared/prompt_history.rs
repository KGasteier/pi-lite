//! Nur originale Nutzereingaben aus einer einzelnen pi-Sessiondatei.
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};

pub fn text(message: &Value) -> Option<String> {
    if message["role"] != "user" { return None; }
    let content = &message["content"];
    let value = content.as_str().map(str::to_owned).unwrap_or_else(|| {
        content.as_array().into_iter().flatten().filter(|p| p["type"] == "text")
            .filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n")
    });
    (!value.trim().is_empty()).then_some(value)
}

pub fn entries(data: &Value) -> Vec<String> {
    data.as_array().into_iter().flatten().filter(|e| e["type"] == "message")
        .filter_map(|e| text(&e["message"])).collect()
}

pub fn read(path: &str) -> Result<Vec<String>, String> {
    const MAX_FILE: u64 = 600 * 1024 * 1024;
    const MAX_LINE: u64 = 32 * 1024 * 1024;
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let meta = f.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_FILE { return Err("Keine reguläre Sessiondatei bis 600 MiB".into()); }
    let mut r = BufReader::new(f.take(meta.len()));
    let mut line = Vec::new();
    let mut result = Vec::new();
    let mut size = 0;
    let mut first = true;
    while r.by_ref().take(MAX_LINE + 1).read_until(b'\n', &mut line).map_err(|e| e.to_string())? > 0 {
        if line.len() as u64 > MAX_LINE { return Err("Eine Sessionzeile überschreitet 32 MiB".into()); }
        let v = serde_json::from_slice::<Value>(&line);
        if first && !v.as_ref().is_ok_and(|e| e["type"] == "session") { return Err("Ungültiger pi-Sessionkopf".into()); }
        first = false;
        if let Ok(e) = v {
            if e["type"] == "message" {
                if let Some(t) = text(&e["message"]) {
                    size += t.len();
                    if size > 32 * 1024 * 1024 { return Err("Prompthistorie überschreitet 32 MiB".into()); }
                    result.push(t);
                }
            }
        }
        line.clear();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn retains_originals_and_repetitions_but_no_tools_or_compaction() {
        let p = std::env::temp_dir().join(format!("prompt-history-{}.jsonl", std::process::id()));
        std::fs::write(&p, concat!(
            "{\"type\":\"session\"}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"erst\"}}\n",
            "{\"type\":\"compaction\",\"summary\":\"kein Prompt\"}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"toolResult\",\"content\":\"privat\"}}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"image\",\"data\":\"xyz\"},{\"type\":\"text\",\"text\":\"erst\"}]}}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"zuletzt\"}}\n{"
        )).unwrap();
        assert_eq!(super::read(p.to_str().unwrap()).unwrap(), ["erst", "erst", "zuletzt"]);
        std::fs::remove_file(p).unwrap();
    }
}
