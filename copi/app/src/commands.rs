//! Eingabeauswertung wie in pi-lite (ui/commands.js, slash.js, history.js).
use pi_backend::cli::{FLAGS, SUBCOMMANDS};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Cmd {
    pub name: String,
    pub description: String,
    pub source: String,
}

pub const LOCAL_COMMANDS: [(&str, &str); 14] = [
    ("model", "Modell auswählen"),
    ("thinking", "Denkstufe setzen oder anzeigen"),
    ("new", "Neue Session starten"),
    ("name", "Session benennen"),
    ("session", "Sessioninfo und Statistik"),
    ("compact", "Kontext komprimieren"),
    ("copy", "Letzte Antwort kopieren"),
    ("export", "Session als HTML exportieren"),
    ("clone", "Session an dieser Stelle duplizieren"),
    ("fork", "Ab früherer Nachricht abzweigen"),
    ("resume", "Andere Session öffnen (Seitenleiste)"),
    ("hotkeys", "Tastaturhilfe"),
    ("changelog", "pi-Changelog im Browser"),
    ("quit", "copi beenden"),
];

/// Nur im Terminal-UI sinnvoll; ehrlicher Hinweis statt Weiterleitung an das Modell.
pub const TERMINAL_ONLY: [&str; 11] = ["settings", "scoped-models", "login", "logout", "llama", "tree", "import", "share", "bug", "trust", "reload"];

pub fn local_commands() -> Vec<Cmd> {
    LOCAL_COMMANDS.iter().map(|(n, d)| Cmd { name: n.to_string(), description: d.to_string(), source: "local".into() }).collect()
}

#[derive(Debug, PartialEq)]
pub enum Parsed {
    Prompt,
    Bash { command: String, exclude: bool },
    Cli(Vec<String>),
    Local { name: String, arg: String },
    Terminal(String),
}

pub fn parse_input(text: &str) -> Parsed {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix('!') {
        let exclude = rest.starts_with('!');
        let command = if exclude { &rest[1..] } else { rest }.trim();
        return if command.is_empty() { Parsed::Prompt } else { Parsed::Bash { command: command.into(), exclude } };
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    if words.len() > 1 && words[0] == "pi" && (SUBCOMMANDS.contains(&words[1]) || FLAGS.contains(&words[1])) {
        return Parsed::Cli(words[1..].iter().map(|s| s.to_string()).collect());
    }
    if let Some(rest) = t.strip_prefix('/') {
        let (name, arg) = match rest.find(char::is_whitespace) {
            Some(i) => (&rest[..i], rest[i..].trim()),
            None => (rest, ""),
        };
        if !name.is_empty() {
            let name = name.to_lowercase();
            if LOCAL_COMMANDS.iter().any(|(n, _)| *n == name) {
                return Parsed::Local { name, arg: arg.into() };
            }
            if TERMINAL_ONLY.contains(&name.as_str()) {
                return Parsed::Terminal(name);
            }
        }
    }
    Parsed::Prompt
}

/// Passende Befehle für die Eingabe `/abc` (None = kein Slash-Modus).
pub fn slash_matches(cmds: &[Cmd], value: &str) -> Option<Vec<Cmd>> {
    let q = value.strip_prefix('/')?;
    if q.chars().any(char::is_whitespace) {
        return None;
    }
    let q = q.to_lowercase();
    let mut out: Vec<Cmd> = cmds
        .iter()
        .filter(|c| {
            let n = c.name.to_lowercase();
            n.starts_with(&q) || n.strip_prefix("skill:").is_some_and(|s| s.starts_with(&q))
        })
        .cloned()
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    Some(out)
}

pub fn source_label(s: &str) -> &str {
    match s {
        "extension" => "Extension",
        "prompt" => "Vorlage",
        "skill" => "Skill",
        "local" => "copi",
        other => other,
    }
}

/// Eingabeverlauf mit Entwurf (wie history.js).
#[derive(Default)]
pub struct InputHistory {
    items: Vec<String>,
    index: usize,
    draft: String,
}

impl InputHistory {
    pub fn reset(&mut self) {
        self.index = self.items.len();
        self.draft.clear();
    }
    pub fn add(&mut self, text: &str) {
        if !text.is_empty() && self.items.last().map(String::as_str) != Some(text) {
            self.items.push(text.into());
        }
        if self.items.len() > 100 {
            self.items.drain(..self.items.len() - 100);
        }
        self.reset();
    }
    pub fn move_by(&mut self, dir: i32, current: &str) -> String {
        if self.items.is_empty() {
            return current.into();
        }
        if self.index == self.items.len() {
            self.draft = current.into();
        }
        self.index = (self.index as i64 + dir as i64).clamp(0, self.items.len() as i64) as usize;
        if self.index == self.items.len() { self.draft.clone() } else { self.items[self.index].clone() }
    }
}

pub fn token_short(n: Option<f64>) -> String {
    let Some(n) = n.filter(|n| n.is_finite()) else { return "—".into() };
    let f = |v: f64, s: &str| {
        let t = format!("{v:.1}");
        format!("{}{s}", t.strip_suffix(".0").unwrap_or(&t))
    };
    if n >= 1e6 {
        f(n / 1e6, "M")
    } else if n >= 1e3 {
        f(n / 1e3, "k")
    } else {
        format!("{}", n.round() as i64)
    }
}

fn local(sec: i64) -> (i32, i32, i32, i32, i32) {
    let t: libc::time_t = sec as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    #[cfg(unix)]
    unsafe { libc::localtime_r(&t, &mut tm) };
    #[cfg(windows)]
    unsafe { libc::localtime_s(&mut tm, &t) };
    (tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min)
}

/// Heute: Uhrzeit, sonst TT.MM.JJ (wie fmtDate).
pub fn fmt_date(sec: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d, hh, mm) = local(sec as i64);
    let (ny, nm, nd, _, _) = local(now as i64);
    if (y, m, d) == (ny, nm, nd) {
        format!("{hh:02}:{mm:02}")
    } else {
        format!("{d:02}.{m:02}.{:02}", y % 100)
    }
}

/// ANSI-CSI-Sequenzen entfernen (`\x1b[…X`).
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' && it.peek() == Some(&'[') {
            it.next();
            while let Some(&n) = it.peek() {
                if ('0'..='9').contains(&n) || n == ';' || n == '?' {
                    it.next();
                } else {
                    break;
                }
            }
            while let Some(&n) = it.peek() {
                if (' '..='/').contains(&n) {
                    it.next();
                } else {
                    break;
                }
            }
            if let Some(&n) = it.peek() {
                if ('@'..='~').contains(&n) {
                    it.next();
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Textanteile eines Inhalts (String oder [{type:text,text}]).
pub fn text_of(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter(|p| p["type"] == "text").filter_map(|p| p["text"].as_str()).collect(),
        _ => String::new(),
    }
}

/// Liegt der Pfad wie ein Dateiverweis vor (kein URL-Schema)?
pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("mailto:")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(h) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(h);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn has_url_scheme(s: &str) -> bool {
    let Some(colon) = s.find(':') else { return false };
    if !s[colon..].starts_with("://") {
        return false;
    }
    let scheme = &s[..colon];
    let mut chars = scheme.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
}

/// `base:line` oder `base:line:column` am Ende von `path` (letzterer Teil wird für die
/// Zeile verwendet, die Spalte wird aktuell nicht weiterverarbeitet).
fn strip_colon_line(path: &str) -> Option<(String, u32)> {
    let parts: Vec<&str> = path.rsplitn(2, ':').collect();
    if parts.len() < 2 {
        return None;
    }
    let (last, before_last) = (parts[0], parts[1]);
    if last.is_empty() || !last.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if let Some(idx) = before_last.rfind(':') {
        let maybe_line = &before_last[idx + 1..];
        if !maybe_line.is_empty() && maybe_line.chars().all(|c| c.is_ascii_digit()) {
            let line: u32 = maybe_line.parse().ok()?;
            return Some((before_last[..idx].to_string(), line));
        }
    }
    let line: u32 = last.parse().ok()?;
    Some((before_last.to_string(), line))
}

/// `base#L<zeile>` oder `base#L<zeile>-L<zeile>` am Ende von `path`.
fn strip_hash_line(path: &str) -> Option<(String, u32)> {
    let idx = path.rfind("#L")?;
    let after = &path[idx + 2..];
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let rest = &after[digits.len()..];
    let valid = rest.is_empty()
        || (rest.starts_with('-') && {
            let r = rest[1..].strip_prefix('L').unwrap_or(&rest[1..]);
            !r.is_empty() && r.chars().all(|c| c.is_ascii_digit())
        });
    if !valid {
        return None;
    }
    let line: u32 = digits.parse().ok()?;
    Some((path[..idx].to_string(), line))
}

/// Letzte Pfadkomponente nach `/` *oder* `\` extrahieren (plattformunabhängige Textprüfung,
/// nicht `std::path`-Semantik): erlaubt Windows-Backslash-Pfade wie `src\app.rs`, ohne dass
/// diese Funktion je unter `cfg(windows)` laufen muss.
fn looks_like_filename(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let Some(dot) = name.rfind('.') else { return false };
    let (base, ext) = (&name[..dot], &name[dot + 1..]);
    !base.is_empty() && !ext.is_empty() && ext.len() <= 12 && ext.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Portiert aus `ui/editor.js` (`editorReference`): erkennt konservativ Datei-/Zeilenverweise
/// in Markdown-Links/Text und unterscheidet sie von externen URLs/Schemata. Liefert `(pfad,
/// zeile)` oder `None`, wenn `value` eher wie eine URL/E-Mail/o. ä. aussieht.
///
/// Erkennt auch Windows-Backslash-Pfade (`src\app.rs:42`, `C:\Users\x\a.rs`) und
/// `file://`-URIs mit Laufwerksbuchstaben (`file:///C:/Users/x/a.rs`); `looks_like_filename`
/// prüft nur die letzte Komponente nach `/` *oder* `\`, daher funktioniert dieselbe Logik auf
/// beiden Trennzeichenstilen ohne separaten Windows-Zweig (reine Textverarbeitung, keine
/// `std::path`-Plattformsemantik nötig; das eigentliche Öffnen inkl. Laufwerksbuchstaben
/// übernimmt `pi_backend::editor::resolve_target`/`open_in_editor` als `PathBuf`).
pub fn editor_reference(value: &str) -> Option<(String, Option<u32>)> {
    let mut path = value.trim().to_string();
    if path.is_empty() {
        return None;
    }
    if let Some(rest) = path.strip_prefix("file://") {
        let (host, pathpart) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        if !host.is_empty() && host != "localhost" {
            return None;
        }
        path = percent_decode(pathpart);
    }
    if has_url_scheme(&path) {
        return None;
    }
    let lower = path.to_lowercase();
    if ["mailto:", "javascript:", "data:", "vscode:", "command:"].iter().any(|s| lower.starts_with(s)) {
        return None;
    }
    if path.contains(['\n', '\r', '\0']) {
        return None;
    }
    let mut line = None;
    if let Some((base, l)) = strip_colon_line(&path) {
        path = base;
        line = Some(l);
    } else if let Some((base, l)) = strip_hash_line(&path) {
        path = base;
        line = Some(l);
    }
    if !looks_like_filename(&path) {
        return None;
    }
    if path.chars().any(|c| "<>\"`|?*".contains(c)) {
        return None;
    }
    Some((path, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(parse_input("!ls"), Parsed::Bash { command: "ls".into(), exclude: false });
        assert_eq!(parse_input("!! ls -l"), Parsed::Bash { command: "ls -l".into(), exclude: true });
        assert_eq!(parse_input("!"), Parsed::Prompt);
        assert_eq!(parse_input("pi update"), Parsed::Cli(vec!["update".into()]));
        assert_eq!(parse_input("pi hallo"), Parsed::Prompt);
        assert_eq!(parse_input("/Model foo bar"), Parsed::Local { name: "model".into(), arg: "foo bar".into() });
        assert_eq!(parse_input("/login"), Parsed::Terminal("login".into()));
        assert_eq!(parse_input("/skill:x"), Parsed::Prompt);
        assert_eq!(parse_input("Hallo"), Parsed::Prompt);
    }

    #[test]
    fn slash() {
        let c = |n: &str| Cmd { name: n.into(), description: String::new(), source: "x".into() };
        let cmds = vec![c("new"), c("skill:notes"), c("name"), c("model")];
        let names = |v: Vec<Cmd>| v.into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names(slash_matches(&cmds, "/n").unwrap()), ["name", "new", "skill:notes"]);
        assert_eq!(names(slash_matches(&cmds, "/").unwrap()).len(), 4);
        assert!(slash_matches(&cmds, "/n x").is_none());
        assert!(slash_matches(&cmds, "n").is_none());
    }

    #[test]
    fn history() {
        let mut h = InputHistory::default();
        h.add("a");
        h.add("b");
        h.add("b");
        assert_eq!(h.move_by(-1, "draft"), "b");
        assert_eq!(h.move_by(-1, "b"), "a");
        assert_eq!(h.move_by(-1, "a"), "a");
        assert_eq!(h.move_by(1, "a"), "b");
        assert_eq!(h.move_by(1, "b"), "draft");
    }

    #[test]
    fn editor_ref() {
        assert_eq!(editor_reference("src/app.rs:42"), Some(("src/app.rs".into(), Some(42))));
        assert_eq!(editor_reference("src/app.rs:42:7"), Some(("src/app.rs".into(), Some(42))));
        assert_eq!(editor_reference("src/app.rs#L10"), Some(("src/app.rs".into(), Some(10))));
        assert_eq!(editor_reference("src/app.rs#L10-L20"), Some(("src/app.rs".into(), Some(10))));
        assert_eq!(editor_reference("README.md"), Some(("README.md".into(), None)));
        assert_eq!(editor_reference("https://example.com/a.md"), None);
        assert_eq!(editor_reference("mailto:a@b.com"), None);
        assert_eq!(editor_reference("javascript:alert(1)"), None);
        assert_eq!(editor_reference("not-a-file"), None);
        assert_eq!(editor_reference("file:///home/x/a.rs:3"), Some(("/home/x/a.rs".into(), Some(3))));
        assert_eq!(editor_reference("has space.md"), Some(("has space.md".into(), None)));
        assert_eq!(editor_reference("bad<name>.md"), None);
        // Windows-Backslash-Pfade (inkl. Laufwerksbuchstabe) müssen unabhängig vom aktuellen
        // Build-Ziel textuell erkannt werden; die eigentliche Pfadauflösung/-öffnung läuft
        // getrennt über `pi_backend::editor` mit echter `std::path`-Semantik.
        assert_eq!(editor_reference("src\\app.rs:42"), Some(("src\\app.rs".into(), Some(42))));
        assert_eq!(editor_reference("C:\\Users\\x\\a.rs"), Some(("C:\\Users\\x\\a.rs".into(), None)));
        assert_eq!(editor_reference("C:\\Users\\x\\a.rs:3"), Some(("C:\\Users\\x\\a.rs".into(), Some(3))));
        assert_eq!(editor_reference("file:///C:/Users/x/a.rs:3"), Some(("/C:/Users/x/a.rs".into(), Some(3))));
    }

    #[test]
    fn tokens_ansi() {
        assert_eq!(token_short(Some(1500.0)), "1.5k");
        assert_eq!(token_short(Some(2_000_000.0)), "2M");
        assert_eq!(token_short(Some(12.0)), "12");
        assert_eq!(token_short(None), "—");
        assert_eq!(strip_ansi("\x1b[1;32mok\x1b[0m!"), "ok!");
        assert_eq!(text_of(&serde_json::json!([{"type":"text","text":"a"},{"type":"image"},{"type":"text","text":"b"}])), "ab");
    }
}
