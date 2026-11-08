//! Kontext-Repair für pi-Sessiondateien (Format v3, siehe pi `docs/session-format.md`).
//!
//! Grundsätze:
//! - **Nur pi-eigene Eintragstypen.** Kürzungen sind `context_edit`-Einträge mit einer
//!   Ersatz-Zeichenkette (nie `null` bei Werkzeugergebnissen, damit kein Tool-Aufruf ohne
//!   Ergebnis entsteht), ein Titel ist ein `session_info`-Eintrag, ein `.md`-Import ist eine
//!   neue Session mit `session_info` + `custom_message`. Kein eigenes Format.
//! - **Nicht destruktiv.** Es wird nur angehängt; die Rohdaten bleiben in der Datei und
//!   `context_edit` ist zweigrelativ. Zusätzlich kann der Anhang direkt danach exakt wieder
//!   abgeschnitten werden ([`undo`]), solange die Datei seitdem nicht gewachsen ist.
//! - **Streamend.** Sessions können über 100 MB groß sein; pro Zeile wird nur eine kleine
//!   Zusammenfassung behalten.
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Größte Sessiondatei, die analysiert wird.
const MAX_FILE: u64 = 600 * 1024 * 1024;
/// Werkzeugergebnisse ab dieser Länge (Zeichen) gelten als „groß“.
const LARGE: usize = 12_000;
/// Die letzten so vielen Werkzeugergebnisse bleiben immer unangetastet.
const KEEP_RECENT_TOOLS: usize = 8;
/// Geschätzte Tokens je Bild.
const IMAGE_TOKENS: u64 = 1_600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Großes, älteres Werkzeugergebnis.
    Large,
    /// Ältere Lesung derselben Datei/desselben Ausschnitts.
    DuplicateRead,
    /// Fehlerserie desselben Werkzeugs (alle bis auf den letzten Fehler).
    ErrorLoop,
    /// Bild in älterem Nutzer- oder Werkzeugeintrag.
    Image,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Large => "großes Ergebnis",
            Kind::DuplicateRead => "doppelt gelesen",
            Kind::ErrorLoop => "Fehlerschleife",
            Kind::Image => "Bild",
        }
    }
}

/// Vorschlag: Inhalt eines Eintrags im Modellkontext durch `replacement` ersetzen.
#[derive(Clone, Debug)]
pub struct Finding {
    pub target: String,
    pub kind: Kind,
    pub label: String,
    pub tokens_saved: u64,
    pub replacement: String,
    pub default_on: bool,
}

/// Anzeichen für „Kontextfäule“ (blockierte oder unbrauchbare Session).
#[derive(Clone, Debug)]
pub struct Rot {
    /// 1 = Warnung, 2 = blockiert die Arbeit wahrscheinlich.
    pub level: u8,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Overlap {
    pub path: String,
    pub title: String,
    pub modified: u64,
    pub shared: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub path: String,
    pub cwd: String,
    /// Dateilänge und letzter Eintrag zum Analysezeitpunkt (Konfliktprüfung beim Anwenden).
    pub len: u64,
    pub leaf: String,
    pub entries: usize,
    pub messages: usize,
    pub compactions: usize,
    pub context_edits: usize,
    pub context_tokens: u64,
    pub findings: Vec<Finding>,
    pub rot: Vec<Rot>,
    pub last_prompts: Vec<String>,
    pub status: Vec<String>,
    pub todos: Vec<String>,
    pub modified_files: Vec<String>,
    pub name: String,
    pub suggested_title: Option<String>,
    pub delete_hint: Option<String>,
    pub overlaps: Vec<Overlap>,
    pub last_activity: String,
}

impl Report {
    pub fn saved_tokens(&self, on: &[bool]) -> u64 {
        self.findings.iter().zip(on).filter(|(_, o)| **o).map(|(f, _)| f.tokens_saved).sum()
    }
}

// ---------- Einlesen ----------

#[derive(Default)]
struct Ent {
    id: String,
    parent: Option<String>,
    ty: String,
    role: String,
    ts: String,
    /// Geschätzte Kontext-Tokens dieses Eintrags.
    tokens: u64,
    chars: usize,
    images: usize,
    /// Nutzer-/Assistenttext (gekürzt) bzw. Kompaktierungs-Zusammenfassung.
    text: String,
    tool_call: String,
    tool_name: String,
    is_error: bool,
    stop: String,
    error: String,
    /// toolCall-Blöcke eines Assistenten: (id, name, Schlüssel für Lesungen, Pfad).
    calls: Vec<(String, String, String, String)>,
    head: String,
    tail: String,
    first_kept: String,
    target: String,
}

fn text_of(content: &Value) -> (String, usize, usize) {
    if let Some(s) = content.as_str() {
        return (s.to_string(), s.chars().count(), 0);
    }
    let (mut out, mut chars, mut images) = (String::new(), 0usize, 0usize);
    for part in content.as_array().into_iter().flatten() {
        match part["type"].as_str() {
            Some("text") => {
                let t = part["text"].as_str().unwrap_or("");
                chars += t.chars().count();
                if !out.is_empty() { out.push('\n'); }
                out.push_str(t);
            }
            Some("image") => images += 1,
            Some("toolCall") => chars += part["arguments"].to_string().len(),
            Some("thinking") => chars += part["thinking"].as_str().map(str::len).unwrap_or(0),
            _ => {}
        }
    }
    (out, chars, images)
}

fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n { return s.to_string(); }
    let mut t: String = s.chars().take(n).collect();
    t.push('…');
    t
}

fn tail_chars(s: &str, n: usize) -> String {
    let c = s.chars().count();
    s.chars().skip(c.saturating_sub(n)).collect()
}

fn parse(v: &Value) -> Ent {
    let mut e = Ent {
        id: v["id"].as_str().unwrap_or("").into(),
        parent: v["parentId"].as_str().map(String::from),
        ty: v["type"].as_str().unwrap_or("").into(),
        ts: v["timestamp"].as_str().unwrap_or("").into(),
        ..Default::default()
    };
    match e.ty.as_str() {
        "message" => {
            let m = &v["message"];
            e.role = m["role"].as_str().unwrap_or("").into();
            let (text, chars, images) = text_of(&m["content"]);
            e.chars = chars;
            e.images = images;
            e.tokens = (chars as u64).div_ceil(4) + images as u64 * IMAGE_TOKENS;
            match e.role.as_str() {
                "user" => e.text = cut(&text, 2_000),
                "assistant" => {
                    e.text = tail_chars(&text, 6_000);
                    e.stop = m["stopReason"].as_str().unwrap_or("").into();
                    e.error = cut(m["errorMessage"].as_str().unwrap_or(""), 240);
                    for p in m["content"].as_array().into_iter().flatten().filter(|p| p["type"] == "toolCall") {
                        let a = &p["arguments"];
                        let path = a["path"].as_str().or_else(|| a["file_path"].as_str()).unwrap_or("").to_string();
                        let key = format!("{}|{}|{}", path, a["offset"], a["limit"]);
                        e.calls.push((p["id"].as_str().unwrap_or("").into(), p["name"].as_str().unwrap_or("").into(), key, path));
                    }
                }
                "toolResult" => {
                    e.tool_call = m["toolCallId"].as_str().unwrap_or("").into();
                    e.tool_name = m["toolName"].as_str().unwrap_or("").into();
                    e.is_error = m["isError"].as_bool().unwrap_or(false);
                    e.head = cut(&text, 400);
                    e.tail = if text.chars().count() > 700 { tail_chars(&text, 200) } else { String::new() };
                }
                "system" => e.tokens = 0,
                _ => {}
            }
        }
        "custom_message" => {
            let (text, chars, images) = text_of(&v["content"]);
            e.role = "custom".into();
            e.chars = chars;
            e.images = images;
            e.text = cut(&text, 400);
            e.tokens = (chars as u64).div_ceil(4) + images as u64 * IMAGE_TOKENS;
        }
        "compaction" | "branch_summary" => {
            let s = v["summary"].as_str().unwrap_or("");
            e.text = cut(s, 20_000);
            e.tokens = (s.len() as u64).div_ceil(4);
            e.first_kept = v["firstKeptEntryId"].as_str().unwrap_or("").into();
        }
        "context_edit" => {
            e.target = v["targetId"].as_str().unwrap_or("").into();
            e.text = match &v["replacement"] { Value::Null => String::new(), r => r.as_str().map(String::from).unwrap_or_else(|| r.to_string()) };
            e.chars = e.text.chars().count();
            e.images = usize::from(v["replacement"].is_null());
        }
        "session_info" => e.text = v["name"].as_str().unwrap_or("").trim().into(),
        _ => {}
    }
    e
}

struct Loaded {
    cwd: String,
    len: u64,
    ents: Vec<Ent>,
}

fn load(path: &Path) -> Result<Loaded, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE {
        return Err(format!("Sessiondatei ist zu groß ({} MB)", meta.len() / 1_048_576));
    }
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(f);
    let (mut cwd, mut ents, mut line, mut first, mut len) = (String::new(), Vec::new(), String::new(), true, 0u64);
    loop {
        line.clear();
        let n = r.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 || !line.ends_with('\n') { break; }
        len += n as u64;
        let Ok(v) = serde_json::from_str::<Value>(line.trim_end()) else { continue };
        if first {
            first = false;
            if v["type"] != "session" { return Err("Keine pi-Sessiondatei (Kopfzeile fehlt)".into()); }
            cwd = v["cwd"].as_str().unwrap_or("").into();
            continue;
        }
        ents.push(parse(&v));
    }
    if first { return Err("Leere Sessiondatei".into()); }
    Ok(Loaded { cwd, len, ents })
}

/// Aktiver Pfad (Wurzel → Blatt). Blatt ist wie in pi der letzte Eintrag der Datei.
fn active_path(ents: &[Ent]) -> Vec<usize> {
    let by: HashMap<&str, usize> = ents.iter().enumerate().map(|(i, e)| (e.id.as_str(), i)).collect();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut cur = ents.len().checked_sub(1);
    while let Some(i) = cur {
        if !seen.insert(i) { break; }
        out.push(i);
        cur = ents[i].parent.as_deref().and_then(|p| by.get(p).copied());
    }
    out.reverse();
    out
}

/// Einträge im Modellkontext nach pi `buildContextEntries()` (neueste Kompaktierung).
fn context_entries(ents: &[Ent], path: &[usize]) -> Vec<usize> {
    let Some(ci) = path.iter().rposition(|&i| ents[i].ty == "compaction") else { return path.to_vec() };
    let c = &ents[path[ci]];
    let mut out = vec![path[ci]];
    if let Some(k) = path[..ci].iter().position(|&i| ents[i].id == c.first_kept) {
        out.extend(path[k..ci].iter().copied().filter(|&i| ents[i].role != "system"));
    }
    out.extend(path[ci + 1..].iter().copied());
    out
}

fn is_context_msg(e: &Ent) -> bool {
    matches!(e.ty.as_str(), "message" | "custom_message" | "compaction" | "branch_summary") && e.role != "system"
}

fn stub(e: &Ent, why: &str) -> String {
    let mut s = format!("[Kontext-Repair: {why} – Ergebnis von `{}` ({} Zeichen) im Modellkontext gekürzt; das Original bleibt in der Sessiondatei.]", e.tool_name, e.chars);
    if !e.head.is_empty() { s.push_str("\n"); s.push_str(&e.head); }
    if !e.tail.is_empty() { s.push_str("\n…\n"); s.push_str(&e.tail); }
    s
}

// ---------- Analyse ----------

/// Analysiert eine lokale Sessiondatei. `other` = weitere lokale Sessions für Überschneidungen.
pub fn analyze(path: &str, with_overlaps: bool) -> Result<Report, String> {
    let p = Path::new(path);
    let l = load(p)?;
    let ents = &l.ents;
    let apath = active_path(ents);
    let ctx = context_entries(ents, &apath);
    let by: HashMap<&str, usize> = ents.iter().enumerate().map(|(i, e)| (e.id.as_str(), i)).collect();

    // Letzte Bearbeitung je Ziel auf dem aktiven Zweig.
    let mut edits: HashMap<&str, usize> = HashMap::new();
    for &i in &apath {
        if ents[i].ty == "context_edit" { edits.insert(ents[i].target.as_str(), i); }
    }
    let effective = |i: usize| -> u64 {
        match edits.get(ents[i].id.as_str()) {
            Some(&k) if ents[k].images == 1 => 0,
            Some(&k) => (ents[k].chars as u64).div_ceil(4),
            None => ents[i].tokens,
        }
    };
    let visible: Vec<usize> = ctx.iter().copied().filter(|&i| is_context_msg(&ents[i])).collect();
    let context_tokens: u64 = visible.iter().map(|&i| effective(i)).sum();

    // toolCallId → (Name, Lese-Schlüssel, Pfad)
    let mut calls: HashMap<&str, (&str, &str, &str)> = HashMap::new();
    for &i in &apath {
        for (id, name, key, path) in &ents[i].calls { calls.insert(id, (name, key, path)); }
    }
    let tools: Vec<usize> = visible.iter().copied().filter(|&i| ents[i].role == "toolResult" && !edits.contains_key(ents[i].id.as_str())).collect();
    let recent: HashSet<usize> = tools.iter().rev().take(KEEP_RECENT_TOOLS).copied().collect();
    let mut findings = Vec::new();
    let mut taken = HashSet::new();
    // Fehlerschleifen: ≥3 aufeinanderfolgende Fehler desselben Werkzeugs.
    let mut run: Vec<usize> = Vec::new();
    let flush = |run: &mut Vec<usize>, findings: &mut Vec<Finding>, taken: &mut HashSet<usize>| {
        if run.len() >= 3 {
            for &i in &run[..run.len() - 1] {
                let e = &ents[i];
                if e.chars > 200 && !recent.contains(&i) && taken.insert(i) {
                    findings.push(Finding { target: e.id.clone(), kind: Kind::ErrorLoop, label: format!("Fehler {} von {} · {}", e.tool_name, run.len(), cut(&e.head.replace('\n', " "), 80)), tokens_saved: e.tokens.saturating_sub(60), replacement: stub(e, "wiederholter Fehler"), default_on: true });
                }
            }
        }
        run.clear();
    };
    for &i in &tools {
        let e = &ents[i];
        if e.is_error && run.last().is_none_or(|&j| ents[j].tool_name == e.tool_name) { run.push(i); } else {
            flush(&mut run, &mut findings, &mut taken);
            if e.is_error { run.push(i); }
        }
    }
    flush(&mut run, &mut findings, &mut taken);
    // Doppelte Lesungen: jede ältere Lesung desselben Ausschnitts.
    let mut last_read: HashMap<&str, usize> = HashMap::new();
    for &i in &tools {
        if let Some((name, key, _)) = calls.get(ents[i].tool_call.as_str()) {
            if *name == "read" && !key.starts_with('|') { last_read.insert(key, i); }
        }
    }
    for &i in &tools {
        let e = &ents[i];
        let Some((name, key, path)) = calls.get(e.tool_call.as_str()) else { continue };
        if *name == "read" && last_read.get(key).is_some_and(|&j| j != i) && e.chars > 1_000 && !e.is_error && taken.insert(i) {
            findings.push(Finding { target: e.id.clone(), kind: Kind::DuplicateRead, label: format!("{} · später erneut gelesen", crate::tilde(path)), tokens_saved: e.tokens.saturating_sub(60), replacement: format!("[Kontext-Repair: ältere Lesung von {path} entfernt – dieselbe Datei wurde später erneut gelesen.]"), default_on: true });
        }
    }
    // Große Ergebnisse und Bilder.
    for &i in &tools {
        let e = &ents[i];
        if recent.contains(&i) || taken.contains(&i) { continue; }
        if e.images > 0 {
            taken.insert(i);
            findings.push(Finding { target: e.id.clone(), kind: Kind::Image, label: format!("{} Bild(er) aus {}", e.images, e.tool_name), tokens_saved: e.tokens.saturating_sub(80), replacement: format!("{}\n[Kontext-Repair: {} Bild(er) aus dem Modellkontext entfernt.]", e.head, e.images), default_on: true });
        } else if e.chars >= LARGE {
            taken.insert(i);
            let what = calls.get(e.tool_call.as_str()).map(|c| if c.2.is_empty() { String::new() } else { format!(" · {}", crate::tilde(c.2)) }).unwrap_or_default();
            findings.push(Finding { target: e.id.clone(), kind: Kind::Large, label: format!("{}{} · {} Zeichen", e.tool_name, what, e.chars), tokens_saved: e.tokens.saturating_sub(200), replacement: stub(e, "großes Ergebnis"), default_on: true });
        }
    }
    let recent_msgs: HashSet<usize> = visible.iter().rev().take(6).copied().collect();
    for &i in &visible {
        let e = &ents[i];
        if e.role == "user" && e.images > 0 && !recent_msgs.contains(&i) && !edits.contains_key(e.id.as_str()) {
            findings.push(Finding { target: e.id.clone(), kind: Kind::Image, label: format!("{} Bild(er) in Nutzernachricht · {}", e.images, cut(&e.text.replace('\n', " "), 60)), tokens_saved: e.images as u64 * IMAGE_TOKENS, replacement: format!("{}\n[Kontext-Repair: {} Bild(er) aus dem Modellkontext entfernt.]", e.text, e.images), default_on: true });
        }
    }
    findings.sort_by_key(|f| by.get(f.target.as_str()).copied().unwrap_or(0));

    // Kennzahlen und Zusammenfassung.
    let users: Vec<&Ent> = apath.iter().map(|&i| &ents[i]).filter(|e| e.ty == "message" && e.role == "user").collect();
    let assistants: Vec<&Ent> = apath.iter().map(|&i| &ents[i]).filter(|e| e.ty == "message" && e.role == "assistant").collect();
    let compactions = apath.iter().filter(|&&i| ents[i].ty == "compaction").count();
    let mut rot = Vec::new();
    if context_tokens > 300_000 {
        rot.push(Rot { level: 2, text: format!("Kontext sehr groß (≈{} Tokens): viele Modelle lehnen ab oder antworten unzuverlässig.", context_tokens) });
    } else if context_tokens > 140_000 {
        rot.push(Rot { level: 1, text: format!("Kontext groß (≈{} Tokens): frühe Details gehen unter, Antworten werden teurer.", context_tokens) });
    }
    let tail_bad: Vec<&&Ent> = assistants.iter().rev().take(3).filter(|e| matches!(e.stop.as_str(), "error" | "aborted")).collect();
    if tail_bad.len() >= 2 {
        let msg = tail_bad.iter().find(|e| !e.error.is_empty()).map(|e| format!(" – {}", e.error)).unwrap_or_default();
        rot.push(Rot { level: 2, text: format!("Die letzten Antworten endeten mit Fehler/Abbruch{msg}.") });
    }
    if assistants.last().is_some_and(|e| e.stop == "length") {
        rot.push(Rot { level: 1, text: "Letzte Antwort wurde wegen Längenlimit abgeschnitten.".into() });
    }
    let last_tools: Vec<usize> = tools.iter().rev().take(10).copied().collect();
    let errs = last_tools.iter().filter(|&&i| ents[i].is_error).count();
    if errs >= 5 {
        rot.push(Rot { level: 2, text: format!("{errs} der letzten {} Werkzeugaufrufe schlugen fehl (Fehlerschleife).", last_tools.len()) });
    }
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let recent_users: Vec<String> = users.iter().rev().take(6).map(|e| norm(&e.text)).filter(|t| t.len() > 8).collect();
    if recent_users.iter().enumerate().any(|(i, t)| recent_users[i + 1..].contains(t)) {
        rot.push(Rot { level: 1, text: "Dieselbe Anweisung wurde mehrfach wiederholt – das Modell setzt sie offenbar nicht um.".into() });
    }
    if compactions >= 5 {
        rot.push(Rot { level: 1, text: format!("{compactions} Kompaktierungen: Zusammenfassungen von Zusammenfassungen verlieren Details.") });
    }
    if let Some(&big) = visible.iter().max_by_key(|&&i| effective(i)) {
        let t = effective(big);
        if context_tokens > 20_000 && t * 10 > context_tokens * 3 {
            rot.push(Rot { level: 1, text: format!("Ein einzelner Eintrag belegt ≈{} % des Kontexts.", t * 100 / context_tokens.max(1)) });
        }
    }

    let last_prompts: Vec<String> = users.iter().rev().take(5).rev().map(|e| cut(&e.text, 500)).collect();
    let latest_summary = apath.iter().rev().map(|&i| &ents[i]).find(|e| e.ty == "compaction").map(|e| e.text.as_str()).unwrap_or("");
    let mut sources: Vec<&str> = vec![latest_summary];
    sources.extend(assistants.iter().rev().take(3).rev().map(|e| e.text.as_str()));
    let (status, todos) = extract_status(&sources);
    let mut modified: Vec<String> = Vec::new();
    for &i in &apath {
        for (_, name, _, path) in &ents[i].calls {
            if matches!(name.as_str(), "edit" | "write") && !path.is_empty() {
                let abs = absolutize(&l.cwd, path);
                if !modified.contains(&abs) { modified.push(abs); }
            }
        }
    }
    let name = apath.iter().rev().map(|&i| &ents[i]).find(|e| e.ty == "session_info").map(|e| e.text.clone()).unwrap_or_default();
    let suggested_title = if name.is_empty() { suggest_title(latest_summary, users.first().map(|e| e.text.as_str()).unwrap_or("")) } else { None };
    let age_days = std::fs::metadata(p).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).map(|d| d.as_secs() / 86_400).unwrap_or(0);
    let delete_hint = if users.is_empty() {
        Some("Keine Nutzernachricht – Session ist leer.".into())
    } else if assistants.iter().all(|e| e.text.trim().is_empty() && e.calls.is_empty()) {
        Some("Keine verwertbare Antwort enthalten.".into())
    } else if users.len() <= 2 && modified.is_empty() && age_days >= 7 {
        Some(format!("Nur {} Frage(n), keine Dateiänderungen, seit {age_days} Tagen unverändert.", users.len()))
    } else { None };
    let mut report = Report {
        path: path.to_string(),
        cwd: l.cwd.clone(),
        len: l.len,
        leaf: ents.last().map(|e| e.id.clone()).unwrap_or_default(),
        entries: ents.len(),
        messages: visible.len(),
        compactions,
        context_edits: edits.len(),
        context_tokens,
        findings,
        rot,
        last_prompts,
        status,
        todos,
        modified_files: modified,
        name,
        suggested_title,
        delete_hint,
        overlaps: Vec::new(),
        last_activity: ents.last().map(|e| e.ts.clone()).unwrap_or_default(),
    };
    if with_overlaps && !report.modified_files.is_empty() {
        report.overlaps = overlaps(p, &report.modified_files);
    }
    Ok(report)
}

fn absolutize(cwd: &str, path: &str) -> String {
    let p = crate::tilde_expand(path);
    if Path::new(&p).is_absolute() || cwd.is_empty() { p } else { Path::new(cwd).join(p).to_string_lossy().into() }
}

/// Status- und ToDo-Listen aus Markdown-Abschnitten bzw. Ankreuzlisten.
pub fn extract_status(sources: &[&str]) -> (Vec<String>, Vec<String>) {
    let (mut status, mut todos) = (Vec::<String>::new(), Vec::<String>::new());
    let push = |v: &mut Vec<String>, s: &str| {
        let s = cut(s.trim(), 220);
        if s.len() > 2 && !v.contains(&s) && v.len() < 16 { v.push(s); }
    };
    for src in sources {
        // 0 = neutral, 1 = Status, 2 = ToDo
        let mut mode = 0u8;
        for line in src.lines() {
            let t = line.trim();
            let head = t.starts_with('#') || (t.starts_with("**") && t.ends_with("**")) || (t.ends_with(':') && t.len() < 60 && !t.starts_with('-'));
            if head {
                let h = t.trim_matches(|c| c == '#' || c == '*' || c == ':' || c == ' ').to_lowercase();
                mode = if ["next", "nächst", "todo", "to-do", "offen", "in progress", "blocked", "geplant", "noch zu"].iter().any(|k| h.contains(k)) { 2 }
                    else if ["done", "erledigt", "progress", "stand", "status", "ergebnis", "fertig", "umgesetzt"].iter().any(|k| h.contains(k)) { 1 }
                    else { 0 };
                continue;
            }
            if let Some(rest) = t.strip_prefix("- [ ]").or_else(|| t.strip_prefix("* [ ]")) { push(&mut todos, rest); continue; }
            if let Some(rest) = t.strip_prefix("- [x]").or_else(|| t.strip_prefix("- [X]")) { push(&mut status, rest); continue; }
            let item = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| {
                let d = t.find(". ")?;
                t[..d].chars().all(|c| c.is_ascii_digit()).then(|| &t[d + 2..])
            });
            if let Some(item) = item {
                match mode { 1 => push(&mut status, item), 2 => push(&mut todos, item), _ => {} }
            }
        }
    }
    (status, todos)
}

/// Titelvorschlag: Ziel aus der letzten Kompaktierung, sonst erste Nutzernachricht.
pub fn suggest_title(summary: &str, first_prompt: &str) -> Option<String> {
    let mut goal = None;
    let mut in_goal = false;
    for line in summary.lines() {
        let t = line.trim();
        if t.starts_with('#') { in_goal = t.to_lowercase().contains("goal") || t.to_lowercase().contains("ziel"); continue; }
        if in_goal && !t.is_empty() { goal = Some(t.trim_start_matches(|c: char| c == '-' || c == '*' || c.is_ascii_digit() || c == '.' || c == ' ').to_string()); break; }
    }
    let src = goal.unwrap_or_else(|| first_prompt.lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_string());
    let words: Vec<&str> = src.split_whitespace().filter(|w| !w.starts_with("/skill:") && !w.starts_with('/') && !w.starts_with("http")).collect();
    let mut out = String::new();
    for w in words {
        let w = w.trim_matches(|c| c == '*' || c == '`' || c == '"');
        if out.chars().count() + w.chars().count() > 60 { break; }
        if !out.is_empty() { out.push(' '); }
        out.push_str(w);
    }
    let out = out.trim_end_matches([',', ';', ':', '-']).to_string();
    (out.chars().count() >= 4).then_some(out)
}

/// Andere lokale Sessions (neueste 80, je ≤ 80 MB), die dieselben Dateien bearbeitet haben.
fn overlaps(own: &Path, files: &[String]) -> Vec<Overlap> {
    let Some(root) = crate::sessions::sessions_root() else { return Vec::new() };
    let own = own.canonicalize().unwrap_or_else(|_| own.to_path_buf());
    let mut all: Vec<(u64, PathBuf)> = Vec::new();
    for d in std::fs::read_dir(root).into_iter().flatten().flatten() {
        for f in std::fs::read_dir(d.path()).into_iter().flatten().flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }
            let Ok(m) = p.metadata() else { continue };
            if m.len() > 80 * 1024 * 1024 { continue; }
            let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            all.push((t, p));
        }
    }
    all.sort_by_key(|a| std::cmp::Reverse(a.0));
    let want: HashSet<&str> = files.iter().map(String::as_str).collect();
    let mut out = Vec::new();
    for (t, p) in all.into_iter().take(80) {
        if p.canonicalize().ok().as_ref() == Some(&own) { continue; }
        let Ok(f) = std::fs::File::open(&p) else { continue };
        let (mut cwd, mut title, mut name, mut shared) = (String::new(), String::new(), String::new(), Vec::<String>::new());
        for (n, line) in BufReader::new(f).lines().map_while(Result::ok).enumerate() {
            if n == 0 { cwd = serde_json::from_str::<Value>(&line).ok().and_then(|v| v["cwd"].as_str().map(String::from)).unwrap_or_default(); continue; }
            if line.contains("\"type\":\"session_info\"") {
                if let Ok(v) = serde_json::from_str::<Value>(&line) { name = v["name"].as_str().unwrap_or("").into(); }
            } else if title.is_empty() && line.contains("\"role\":\"user\"") {
                if let Ok(v) = serde_json::from_str::<Value>(&line) { title = cut(&text_of(&v["message"]["content"]).0, 80); }
            } else if line.contains("\"toolCall\"") && (line.contains("\"name\":\"edit\"") || line.contains("\"name\":\"write\"")) {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    for c in v["message"]["content"].as_array().into_iter().flatten().filter(|c| c["type"] == "toolCall" && matches!(c["name"].as_str(), Some("edit" | "write"))) {
                        if let Some(fp) = c["arguments"]["path"].as_str() {
                            let abs = absolutize(&cwd, fp);
                            if want.contains(abs.as_str()) && !shared.contains(&abs) { shared.push(abs); }
                        }
                    }
                }
            }
        }
        if !shared.is_empty() {
            out.push(Overlap { path: p.to_string_lossy().into(), title: if name.is_empty() { title } else { name }, modified: t, shared });
        }
    }
    out.sort_by(|a, b| b.shared.len().cmp(&a.shared.len()).then(b.modified.cmp(&a.modified)));
    out.truncate(12);
    out
}

// ---------- Schreiben ----------

/// Ergebnis eines Anhangs; erlaubt exaktes Rückgängigmachen.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    pub old_len: u64,
    pub new_len: u64,
    pub entries: usize,
}

fn checked_path(path: &str) -> Result<PathBuf, String> {
    let root = crate::sessions::sessions_root().ok_or("kein Sessions-Ordner")?.canonicalize().map_err(|e| e.to_string())?;
    let p = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
    if !p.starts_with(&root) || p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
        return Err("Pfad liegt nicht im Sessions-Ordner".into());
    }
    Ok(p)
}

/// Zufällige 8-stellige Hex-Kennung wie bei pi, eindeutig gegenüber `taken`.
fn new_id(taken: &mut HashSet<String>) -> String {
    use std::hash::{BuildHasher, Hasher};
    loop {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_usize(taken.len());
        let id = format!("{:08x}", h.finish() as u32);
        if taken.insert(id.clone()) { return id; }
    }
}

/// UTC-Zeitstempel im ISO-Format von pi (`2026-11-08T12:34:56.789Z`).
pub fn iso_now() -> String {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    iso_ms(ms)
}

pub fn iso_ms(ms: i64) -> String {
    let (secs, milli) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant: civil_from_days
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{milli:03}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Hängt `context_edit`-Einträge (`(Ziel, Ersatz)`) und optional einen `session_info`-Namen
/// an. Bricht ab, wenn die Datei seit der Analyse verändert wurde.
pub fn apply(path: &str, expect_len: u64, expect_leaf: &str, edits: &[(String, String)], title: Option<&str>) -> Result<Applied, String> {
    let p = checked_path(path)?;
    apply_at(&p, expect_len, expect_leaf, edits, title)
}

fn apply_at(p: &Path, expect_len: u64, expect_leaf: &str, edits: &[(String, String)], title: Option<&str>) -> Result<Applied, String> {
    let l = load(p)?;
    let disk = std::fs::metadata(p).map_err(|e| e.to_string())?.len();
    if l.len != expect_len || disk != expect_len || l.ents.last().map(|e| e.id.as_str()) != Some(expect_leaf) {
        return Err("Session wurde seit der Analyse verändert – bitte neu analysieren.".into());
    }
    if edits.is_empty() && title.is_none() { return Err("Nichts ausgewählt.".into()); }
    let ids: HashSet<&str> = l.ents.iter().map(|e| e.id.as_str()).collect();
    for (t, _) in edits {
        if !ids.contains(t.as_str()) { return Err(format!("Zieleintrag {t} fehlt")); }
    }
    let mut taken: HashSet<String> = l.ents.iter().map(|e| e.id.clone()).collect();
    let mut parent = expect_leaf.to_string();
    let mut out = String::new();
    let ts = iso_now();
    for (target, replacement) in edits {
        let id = new_id(&mut taken);
        out.push_str(&json!({"type":"context_edit","id":id,"parentId":parent,"timestamp":ts,"targetId":target,"replacement":replacement}).to_string());
        out.push('\n');
        parent = id;
    }
    if let Some(t) = title.map(str::trim).filter(|t| !t.is_empty()) {
        let id = new_id(&mut taken);
        out.push_str(&json!({"type":"session_info","id":id,"parentId":parent,"timestamp":ts,"name":t}).to_string());
        out.push('\n');
    }
    let mut f = std::fs::OpenOptions::new().append(true).open(p).map_err(|e| e.to_string())?;
    f.write_all(out.as_bytes()).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    Ok(Applied { old_len: expect_len, new_len: expect_len + out.len() as u64, entries: out.lines().count() })
}

/// Schneidet einen eben angehängten Repair-Anhang wieder ab (nur wenn unverändert).
pub fn undo(path: &str, a: &Applied) -> Result<(), String> {
    let p = checked_path(path)?;
    undo_at(&p, a)
}

fn undo_at(p: &Path, a: &Applied) -> Result<(), String> {
    let len = std::fs::metadata(p).map_err(|e| e.to_string())?.len();
    if len != a.new_len {
        return Err("Session ist seit dem Repair weitergelaufen – Rückgängig nicht mehr möglich (Änderungen bleiben über den Zweig umkehrbar).".into());
    }
    let tail = {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = std::fs::File::open(p).map_err(|e| e.to_string())?;
        f.seek(SeekFrom::Start(a.old_len)).map_err(|e| e.to_string())?;
        let mut s = String::new();
        f.read_to_string(&mut s).map_err(|e| e.to_string())?;
        s
    };
    if !tail.lines().all(|l| serde_json::from_str::<Value>(l).is_ok_and(|v| matches!(v["type"].as_str(), Some("context_edit" | "session_info")))) {
        return Err("Unerwarteter Dateiinhalt – nichts abgeschnitten.".into());
    }
    let f = std::fs::OpenOptions::new().write(true).open(p).map_err(|e| e.to_string())?;
    f.set_len(a.old_len).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())
}

// ---------- Markdown ----------

/// Markdown-Export des aktiven Modellkontexts (mit angewandten `context_edit`s) samt Bericht.
pub fn export_md(path: &str, report: &Report) -> Result<String, String> {
    let p = Path::new(path);
    let l = load(p)?;
    let ents = &l.ents;
    let apath = active_path(ents);
    let ctx = context_entries(ents, &apath);
    let mut edits: HashMap<String, Value> = HashMap::new();
    let want: HashSet<String> = ctx.iter().map(|&i| ents[i].id.clone()).collect();
    // Zweiter Durchlauf: volle Inhalte nur der Kontexteinträge.
    let mut full: HashMap<String, Value> = HashMap::new();
    let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let on_path: HashSet<&str> = apath.iter().map(|&i| ents[i].id.as_str()).collect();
    for line in BufReader::new(f).lines().map_while(Result::ok).skip(1) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let id = v["id"].as_str().unwrap_or("");
        if v["type"] == "context_edit" && on_path.contains(id) {
            edits.insert(v["targetId"].as_str().unwrap_or("").into(), v["replacement"].clone());
        } else if want.contains(id) {
            full.insert(id.into(), v);
        }
    }
    let title = if report.name.is_empty() { report.suggested_title.clone().unwrap_or_else(|| "pi-Session".into()) } else { report.name.clone() };
    let mut md = format!("# {title}\n\n");
    md.push_str(&format!("- Quelle: `{}`\n- Arbeitsordner: `{}`\n- Exportiert: {}\n- Kontext: ≈{} Tokens, {} Nachrichten, {} Kompaktierung(en)\n\n", crate::tilde(path), crate::tilde(&l.cwd), iso_now(), report.context_tokens, report.messages, report.compactions));
    md.push_str(&report_md(report));
    md.push_str("\n## Verlauf (Modellkontext)\n\n");
    for &i in &ctx {
        let e = &ents[i];
        let Some(v) = full.get(&e.id) else { continue };
        let edited = edits.get(&e.id);
        if matches!(edited, Some(Value::Null)) { continue; }
        match e.ty.as_str() {
            "compaction" | "branch_summary" => md.push_str(&format!("### Zusammenfassung ({})\n\n{}\n\n", e.ty, v["summary"].as_str().unwrap_or(""))),
            "custom_message" => md.push_str(&format!("### Kontext ({})\n\n{}\n\n", v["customType"].as_str().unwrap_or(""), text_of(&v["content"]).0)),
            "message" => {
                let m = &v["message"];
                let content = edited.cloned().unwrap_or_else(|| m["content"].clone());
                match e.role.as_str() {
                    "user" => md.push_str(&format!("### Nutzer · {}\n\n{}\n\n", e.ts, text_of(&content).0)),
                    "assistant" => {
                        md.push_str(&format!("### Assistent · {}\n\n", m["model"].as_str().unwrap_or("")));
                        let t = text_of(&content).0;
                        if !t.trim().is_empty() { md.push_str(&t); md.push_str("\n\n"); }
                        for c in content.as_array().into_iter().flatten().filter(|c| c["type"] == "toolCall") {
                            md.push_str(&format!("- Werkzeug `{}`: `{}`\n", c["name"].as_str().unwrap_or(""), cut(&c["arguments"].to_string(), 300).replace('`', "'")));
                        }
                        md.push('\n');
                    }
                    "toolResult" => {
                        let t = text_of(&content).0;
                        md.push_str(&format!("<details><summary>Ergebnis {}{} ({} Zeichen)</summary>\n\n```text\n{}\n```\n\n</details>\n\n", m["toolName"].as_str().unwrap_or(""), if e.is_error { " · Fehler" } else { "" }, t.chars().count(), cut(&t, 2_000).replace("```", "'''")));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(md)
}

/// Berichtsteil (Fäule, Status, ToDos, letzte Prompts, Überschneidungen) als Markdown.
pub fn report_md(r: &Report) -> String {
    let mut md = String::from("## Kontext-Repair\n\n");
    if r.rot.is_empty() { md.push_str("Keine Anzeichen für Kontextfäule.\n\n"); }
    for x in &r.rot { md.push_str(&format!("- {} {}\n", if x.level >= 2 { "**Blockierend:**" } else { "Warnung:" }, x.text)); }
    let list = |md: &mut String, h: &str, v: &[String]| {
        if v.is_empty() { return; }
        md.push_str(&format!("\n### {h}\n\n"));
        for s in v { md.push_str(&format!("- {}\n", s.replace('\n', " "))); }
    };
    list(&mut md, "Status", &r.status);
    list(&mut md, "ToDos", &r.todos);
    list(&mut md, "Letzte Prompts", &r.last_prompts);
    list(&mut md, "Geänderte Dateien", &r.modified_files.iter().map(|f| crate::tilde(f)).collect::<Vec<_>>());
    list(&mut md, "Überschneidungen mit anderen Sessions", &r.overlaps.iter().map(|o| format!("{} – {} gemeinsame Datei(en): {}", o.title, o.shared.len(), o.shared.iter().map(|f| crate::tilde(f)).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>());
    md
}

/// Markdown als neue pi-Session importieren: Kopf, `session_info` (Titel) und eine
/// `custom_message` mit dem Text als Kontext. Liefert den Pfad der neuen Sessiondatei.
pub fn import_md(md_path: &str, cwd: &str) -> Result<String, String> {
    let root = crate::sessions::sessions_root().ok_or("kein Sessions-Ordner")?;
    import_md_at(&root, md_path, cwd)
}

fn import_md_at(root: &Path, md_path: &str, cwd: &str) -> Result<String, String> {
    let src = Path::new(&crate::tilde_expand(md_path)).to_path_buf();
    if !src.extension().and_then(|e| e.to_str()).unwrap_or("").eq_ignore_ascii_case("md") {
        return Err("Nur .md-Dateien können importiert werden".into());
    }
    if std::fs::metadata(&src).map_err(|e| e.to_string())?.len() > 4_000_000 {
        return Err("Markdown-Datei ist größer als 4 MB".into());
    }
    let text = std::fs::read_to_string(&src).map_err(|e| e.to_string())?;
    let title = text.lines().find_map(|l| l.strip_prefix("# ")).map(|t| t.trim().to_string())
        .unwrap_or_else(|| src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let cwd = if cwd.is_empty() { crate::home().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default() } else { cwd.to_string() };
    let safe = format!("--{}--", cwd.trim_start_matches(['/', '\\']).replace(['/', '\\', ':'], "-"));
    let dir = root.join(safe);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = iso_now();
    let mut taken = HashSet::new();
    let uuid = {
        let h: String = (0..4).map(|_| new_id(&mut taken)).collect();
        format!("{}-{}-4{}-a{}-{}", &h[..8], &h[8..12], &h[13..16], &h[17..20], &h[20..32])
    };
    let file = dir.join(format!("{}_{uuid}.jsonl", ts.replace([':', '.'], "-")));
    let (a, b) = (new_id(&mut taken), new_id(&mut taken));
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let lines = [
        json!({"type":"session","version":3,"id":uuid,"timestamp":ts,"cwd":cwd}),
        json!({"type":"session_info","id":a,"parentId":null,"timestamp":ts,"name":format!("Import: {title}")}),
        json!({"type":"custom_message","id":b,"parentId":a,"timestamp":ts,"customType":"copi-md-import","content":format!("Importierter Kontext aus {}:\n\n{text}", src.display()),"display":true,"details":{"source":src.to_string_lossy(),"importedAt":ms}}),
    ];
    let body: String = lines.iter().map(|v| format!("{v}\n")).collect();
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&file).map_err(|e| e.to_string())?;
    f.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    Ok(file.to_string_lossy().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("copi-repair-{tag}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn msg(id: &str, parent: Option<&str>, m: Value) -> String {
        json!({"type":"message","id":id,"parentId":parent,"timestamp":"2026-01-01T00:00:00.000Z","message":m}).to_string()
    }

    /// Kleine Session: zwei doppelte Lesungen, große Ausgabe, Fehlerserie, offene ToDos.
    fn sample(dir: &Path) -> PathBuf {
        let big = "x".repeat(20_000);
        let mut l = vec![json!({"type":"session","version":3,"id":"s","timestamp":"t","cwd":"/w"}).to_string()];
        let mut parent: Option<String> = None;
        let mut n = 0;
        let mut push = |l: &mut Vec<String>, m: Value| {
            n += 1;
            let id = format!("{n:08x}");
            l.push(msg(&id, parent.as_deref(), m));
            parent = Some(id.clone());
            id
        };
        push(&mut l, json!({"role":"user","content":"Baue die Funktion foo in src/a.rs","timestamp":1}));
        for k in 0..2 {
            push(&mut l, json!({"role":"assistant","content":[{"type":"toolCall","id":format!("r{k}"),"name":"read","arguments":{"path":"src/a.rs"}}],"stopReason":"toolUse"}));
            push(&mut l, json!({"role":"toolResult","toolCallId":format!("r{k}"),"toolName":"read","content":[{"type":"text","text":"y".repeat(3_000)}],"isError":false}));
        }
        push(&mut l, json!({"role":"assistant","content":[{"type":"toolCall","id":"b","name":"bash","arguments":{"command":"cargo test"}}],"stopReason":"toolUse"}));
        push(&mut l, json!({"role":"toolResult","toolCallId":"b","toolName":"bash","content":[{"type":"text","text":big}],"isError":false}));
        for k in 0..3 {
            push(&mut l, json!({"role":"assistant","content":[{"type":"toolCall","id":format!("e{k}"),"name":"edit","arguments":{"path":"src/a.rs"}}],"stopReason":"toolUse"}));
            push(&mut l, json!({"role":"toolResult","toolCallId":format!("e{k}"),"toolName":"edit","content":[{"type":"text","text":"Fehler: Text nicht gefunden ".repeat(20)}],"isError":true}));
        }
        // Genug spätere Werkzeugergebnisse, damit die älteren nicht mehr „neu“ sind.
        for k in 0..9 {
            push(&mut l, json!({"role":"assistant","content":[{"type":"toolCall","id":format!("z{k}"),"name":"bash","arguments":{"command":"ls"}}],"stopReason":"toolUse"}));
            push(&mut l, json!({"role":"toolResult","toolCallId":format!("z{k}"),"toolName":"bash","content":[{"type":"text","text":"ok"}],"isError":false}));
        }
        push(&mut l, json!({"role":"assistant","content":[{"type":"text","text":"## Stand\n- foo gebaut\n\n## Nächste Schritte\n- Tests ergänzen\n- [ ] Doku"}],"stopReason":"stop"}));
        let p = dir.join("s.jsonl");
        std::fs::write(&p, l.join("\n") + "\n").unwrap();
        p
    }

    #[test]
    fn analyze_finds_duplicates_large_and_error_loop_and_summary() {
        let d = tmp("an");
        let p = sample(&d);
        let r = analyze(p.to_str().unwrap(), false).unwrap();
        let kinds: Vec<Kind> = r.findings.iter().map(|f| f.kind).collect();
        assert!(kinds.contains(&Kind::DuplicateRead), "{kinds:?}");
        assert!(kinds.contains(&Kind::Large), "{kinds:?}");
        assert_eq!(kinds.iter().filter(|k| **k == Kind::ErrorLoop).count(), 2, "alle bis auf den letzten Fehler");
        assert_eq!(r.status, vec!["foo gebaut"]);
        assert_eq!(r.todos, vec!["Tests ergänzen", "Doku"]);
        assert_eq!(r.last_prompts.len(), 1);
        assert_eq!(r.suggested_title.as_deref(), Some("Baue die Funktion foo in src/a.rs"));
        assert_eq!(r.modified_files, vec!["/w/src/a.rs"]);
        assert!(r.delete_hint.is_none());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn apply_writes_valid_pi_entries_and_undo_restores_bytes() {
        let d = tmp("ap");
        let p = sample(&d);
        let before = std::fs::read(&p).unwrap();
        let r = analyze(p.to_str().unwrap(), false).unwrap();
        let edits: Vec<(String, String)> = r.findings.iter().map(|f| (f.target.clone(), f.replacement.clone())).collect();
        let a = apply_at(&p, r.len, &r.leaf, &edits, Some("Foo bauen")).unwrap();
        assert_eq!(a.entries, edits.len() + 1);
        let text = std::fs::read_to_string(&p).unwrap();
        let tail: Vec<Value> = text.lines().skip(before.iter().filter(|b| **b == b'\n').count()).map(|l| serde_json::from_str(l).unwrap()).collect();
        let mut parent = r.leaf.clone();
        for v in &tail {
            assert_eq!(v["parentId"], json!(parent), "lückenlose Kette ab dem Blatt");
            assert_eq!(v["id"].as_str().unwrap().len(), 8);
            assert!(v["timestamp"].as_str().unwrap().ends_with('Z'));
            parent = v["id"].as_str().unwrap().into();
        }
        assert!(tail[..edits.len()].iter().all(|v| v["type"] == "context_edit" && v["replacement"].is_string()));
        assert_eq!(tail.last().unwrap()["type"], "session_info");
        // Neue Analyse: Kontext kleiner, keine erneuten Vorschläge, Name gesetzt.
        let r2 = analyze(p.to_str().unwrap(), false).unwrap();
        assert!(r2.context_tokens < r.context_tokens);
        assert!(r2.findings.is_empty(), "{:?}", r2.findings);
        assert_eq!(r2.name, "Foo bauen");
        assert!(r2.suggested_title.is_none());
        // Veralteter Stand wird abgelehnt.
        assert!(apply_at(&p, r.len, &r.leaf, &edits, None).is_err());
        undo_at(&p, &a).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), before);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn compaction_hides_older_entries_from_context() {
        let d = tmp("co");
        let p = d.join("c.jsonl");
        let l = [
            json!({"type":"session","version":3,"id":"s","timestamp":"t","cwd":"/w"}).to_string(),
            msg("00000001", None, json!({"role":"user","content":"alt ".repeat(4000)})),
            msg("00000002", Some("00000001"), json!({"role":"user","content":"behalten"})),
            json!({"type":"compaction","id":"00000003","parentId":"00000002","timestamp":"t","summary":"## Goal\n- Parser reparieren\n","firstKeptEntryId":"00000002","tokensBefore":1}).to_string(),
        ];
        std::fs::write(&p, l.join("\n") + "\n").unwrap();
        let r = analyze(p.to_str().unwrap(), false).unwrap();
        assert!(r.context_tokens < 100, "{}", r.context_tokens);
        assert_eq!(r.compactions, 1);
        assert_eq!(r.suggested_title.as_deref(), Some("Parser reparieren"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn export_and_import_markdown_roundtrip_is_valid_session() {
        let d = tmp("md");
        let p = sample(&d);
        let r = analyze(p.to_str().unwrap(), false).unwrap();
        let md = export_md(p.to_str().unwrap(), &r).unwrap();
        assert!(md.starts_with("# Baue die Funktion foo"));
        assert!(md.contains("### Nutzer") && md.contains("### ToDos") && md.contains("- Tests ergänzen"));
        let mdp = d.join("x.md");
        std::fs::write(&mdp, &md).unwrap();
        let root = d.join("sessions");
        let out = import_md_at(&root, mdp.to_str().unwrap(), "/w/projekt").unwrap();
        assert!(out.contains("--w-projekt--"));
        let lines: Vec<Value> = std::fs::read_to_string(&out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines[0]["type"], "session");
        assert_eq!(lines[0]["version"], 3);
        assert_eq!(lines[1]["type"], "session_info");
        assert_eq!(lines[2]["type"], "custom_message");
        assert_eq!(lines[2]["parentId"], lines[1]["id"]);
        let info = crate::sessions::read_info(Path::new(&out), 0).unwrap();
        assert_eq!(info.cwd, "/w/projekt");
        assert!(info.name.starts_with("Import: Baue"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn iso_and_status_helpers() {
        assert_eq!(iso_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_ms(1_793_915_213_733), "2026-11-05T21:46:53.733Z");
        let (s, t) = extract_status(&["**Done**\n1. Aaa\n\nNext Steps:\n- Bbb\n- [ ] Ccc"]);
        assert_eq!(s, vec!["Aaa"]);
        assert_eq!(t, vec!["Bbb", "Ccc"]);
        assert_eq!(suggest_title("", "/skill:copidev bitte die Sidebar umbauen"), Some("bitte die Sidebar umbauen".into()));
    }
}

#[cfg(test)]
mod probe {
    /// Nur lesend: `REPAIR_PROBE=<datei> … -- --ignored probe`
    #[test]
    #[ignore]
    fn probe() {
        let p = std::env::var("REPAIR_PROBE").unwrap();
        let t = std::time::Instant::now();
        let r = super::analyze(&p, true).unwrap();
        println!("{:?} tokens={} msgs={} comp={} edits={} findings={} saved={}", t.elapsed(), r.context_tokens, r.messages, r.compactions, r.context_edits, r.findings.len(), r.saved_tokens(&vec![true; r.findings.len()]));
        for f in r.findings.iter().take(12) { println!("  {:?} {} -{}", f.kind, f.label, f.tokens_saved); }
        for x in &r.rot { println!("ROT{} {}", x.level, x.text); }
        println!("title={:?} name={:?} del={:?}", r.suggested_title, r.name, r.delete_hint);
        println!("status={:?}\ntodos={:?}", r.status, r.todos);
        println!("prompts={:?}", r.last_prompts.iter().map(|s| s.chars().take(60).collect::<String>()).collect::<Vec<_>>());
        for o in &r.overlaps { println!("OV {} {:?}", o.title.chars().take(50).collect::<String>(), o.shared); }
    }
}
