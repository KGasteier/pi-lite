//! Stichwortsuche über lokale pi-Sessions (Kontextmanager). Indiziert wird nur, was ein
//! Mensch als Gesprächsinhalt wiedererkennt: Nutzer- und Assistententext, Zusammenfassungen
//! (`compaction`, `branch_summary`) und Sessionnamen – keine Werkzeugausgaben. Der Index liegt
//! nur im Speicher, wird je Datei über Länge+Änderungszeit aktualisiert und liest Sessions
//! ausschließlich (nie schreibend).
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Textanteil je Session höchstens 4 MB (der Rest wird nicht durchsucht).
const MAX_DOC: usize = 4 << 20;
/// Nur in den ersten Bytes einer Zeile nach Typ/Rolle suchen (vor dem teuren JSON-Parsen).
const HEAD: usize = 400;

#[derive(Default)]
pub struct Corpus {
    docs: HashMap<String, Doc>,
}

struct Doc {
    len: u64,
    mtime: u64,
    text: String,
    lower: String,
}

/// Treffer einer Session: Anzahl je Suchwort (gleiche Reihenfolge wie die Anfrage) und ein
/// kurzer Ausschnitt um den ersten Treffer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hit {
    pub counts: Vec<usize>,
    pub snippet: String,
}

fn mtime(m: &std::fs::Metadata) -> u64 {
    m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

fn head(line: &str) -> &str {
    let mut n = line.len().min(HEAD);
    while !line.is_char_boundary(n) {
        n -= 1;
    }
    &line[..n]
}

/// Durchsuchbarer Text einer Sessiondatei (`None`, wenn sie nicht lesbar ist).
pub fn extract(path: &Path) -> Option<String> {
    let f = std::fs::File::open(path).ok()?;
    let mut out = String::new();
    for raw in BufReader::new(f).split(b'\n').map_while(Result::ok) {
        let line = String::from_utf8_lossy(&raw);
        let h = head(&line);
        let wanted = h.contains("\"role\":\"user\"") || h.contains("\"role\":\"assistant\"") || h.contains("\"type\":\"compaction\"") || h.contains("\"type\":\"branch_summary\"") || h.contains("\"type\":\"session_info\"");
        if !wanted {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        match v["type"].as_str().unwrap_or("") {
            "message" => {
                let m = &v["message"];
                if !matches!(m["role"].as_str(), Some("user" | "assistant")) {
                    continue;
                }
                match &m["content"] {
                    serde_json::Value::String(s) => push(&mut out, s),
                    serde_json::Value::Array(parts) => {
                        for p in parts.iter().filter(|p| p["type"] == "text") {
                            push(&mut out, p["text"].as_str().unwrap_or(""));
                        }
                    }
                    _ => {}
                }
            }
            "compaction" | "branch_summary" => push(&mut out, v["summary"].as_str().unwrap_or("")),
            "session_info" => push(&mut out, v["name"].as_str().unwrap_or("")),
            _ => {}
        }
        if out.len() >= MAX_DOC {
            break;
        }
    }
    Some(out)
}

fn push(out: &mut String, s: &str) {
    if !s.trim().is_empty() {
        out.push_str(s);
        out.push('\n');
    }
}

/// Ausschnitt um Byteposition `at` im kleingeschriebenen Text (Zeichenindex auf das Original
/// übertragen; Zeilenumbrüche werden zu Leerzeichen).
fn snippet(text: &str, lower: &str, at: usize) -> String {
    let ci = lower[..at].chars().count();
    let start = ci.saturating_sub(30);
    let s: String = text.chars().skip(start).take(110).map(|c| if c.is_whitespace() { ' ' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if start > 0 { format!("…{s}") } else { s }
}

impl Corpus {
    /// Index auf genau diese Dateien bringen: neue/geänderte lesen, verschwundene entfernen.
    pub fn sync(&mut self, paths: &[String]) {
        let keep: HashSet<&str> = paths.iter().map(String::as_str).collect();
        self.docs.retain(|k, _| keep.contains(k.as_str()));
        for p in paths {
            let Ok(m) = std::fs::metadata(p) else { continue };
            let (len, mt) = (m.len(), mtime(&m));
            if self.docs.get(p).is_some_and(|d| d.len == len && d.mtime == mt) {
                continue;
            }
            if let Some(text) = extract(Path::new(p)) {
                let lower = text.to_lowercase();
                self.docs.insert(p.clone(), Doc { len, mtime: mt, text, lower });
            }
        }
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Treffer je Session (nur Sessions mit mindestens einem Wort im Inhalt). `words` müssen
    /// bereits kleingeschrieben sein; ob *alle* Wörter passen, entscheidet der Aufrufer
    /// zusammen mit Titel/Name/Ordner.
    pub fn search(&self, words: &[String]) -> HashMap<String, Hit> {
        let mut out = HashMap::new();
        if words.is_empty() {
            return out;
        }
        for (path, d) in &self.docs {
            let mut counts = Vec::with_capacity(words.len());
            let mut first = None;
            for w in words {
                let mut n = 0;
                for (i, _) in d.lower.match_indices(w.as_str()) {
                    if first.is_none() {
                        first = Some(i);
                    }
                    n += 1;
                    if n >= 999 {
                        break;
                    }
                }
                counts.push(n);
            }
            if let Some(at) = first {
                out.insert(path.clone(), Hit { counts, snippet: snippet(&d.text, &d.lower, at) });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_user_assistant_summaries_and_names_but_not_tool_output() {
        let dir = std::env::temp_dir().join(format!("pi-search-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("s.jsonl");
        let lines = [
            r#"{"type":"session","version":3,"id":"h","cwd":"/x"}"#,
            r#"{"type":"message","id":"1","parentId":null,"message":{"role":"user","content":"Bitte den Kontextmanager bauen"}}"#,
            r#"{"type":"message","id":"2","parentId":"1","message":{"role":"assistant","content":[{"type":"thinking","thinking":"geheim"},{"type":"text","text":"Mache ich: Seitenleiste umbenennen."}]}}"#,
            r#"{"type":"message","id":"3","parentId":"2","message":{"role":"toolResult","content":[{"type":"text","text":"werkzeugausgabe xyz"}]}}"#,
            r#"{"type":"compaction","id":"4","parentId":"3","summary":"Zusammenfassung Ärger mit Fonts"}"#,
            r#"{"type":"session_info","id":"5","parentId":"4","name":"Sidebar Umbau"}"#,
        ];
        std::fs::write(&p, lines.join("\n") + "\n").unwrap();
        let ps = vec![p.to_string_lossy().into_owned()];
        let mut c = Corpus::default();
        c.sync(&ps);
        assert_eq!(c.len(), 1);
        let w = |s: &[&str]| s.iter().map(|x| x.to_lowercase()).collect::<Vec<_>>();
        let h = c.search(&w(&["kontextmanager", "seitenleiste"]));
        let hit = &h[&ps[0]];
        assert_eq!(hit.counts, vec![1, 1]);
        assert!(hit.snippet.contains("Kontextmanager"), "Ausschnitt im Original-Schreibweise: {}", hit.snippet);
        assert!(c.search(&w(&["werkzeugausgabe"])).is_empty(), "keine Werkzeugausgaben");
        assert!(c.search(&w(&["geheim"])).is_empty(), "kein Thinking");
        assert_eq!(c.search(&w(&["ärger"]))[&ps[0]].counts, vec![1]);
        assert_eq!(c.search(&w(&["umbau"]))[&ps[0]].counts, vec![1]);
        // Teiltreffer: zweites Wort fehlt → Zählung 0, Entscheidung beim Aufrufer.
        assert_eq!(c.search(&w(&["kontextmanager", "fehlt"]))[&ps[0]].counts, vec![1, 0]);
        // Entfernte Dateien fallen aus dem Index.
        c.sync(&[]);
        assert!(c.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
