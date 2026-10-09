//! Leichter Syntax-Tokenizer ohne Abhängigkeiten.
//!
//! Kein vollständiger Parser: erkennt Kommentare, Strings, Zahlen, Schlüsselwörter,
//! Literale und Funktionsaufrufe für die gängigen Sprachen in Agenten-Antworten.
//! Unbekannte Sprachen werden nur mit allgemeinen Regeln (Strings, Zahlen, `#`/`//`)
//! hervorgehoben. Die Farben legt der Renderer fest (in pi-lite: hljs „GitHub Dark“).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Keyword,
    Literal,
    String,
    Number,
    Comment,
    Function,
    Type,
    /// Diff: hinzugefügte / entfernte Zeile.
    Added,
    Removed,
    /// Überschriften/Abschnitte (Diff-Hunks, INI/TOML-Sektionen).
    Section,
}

struct Lang {
    line: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    keywords: &'static [&'static str],
    literals: &'static [&'static str],
    /// Backtick-Strings (JS, Go, Shell).
    backtick: bool,
    /// Einfache Anführungszeichen als String (nicht in Rust: Lifetimes).
    single: bool,
}

const RUST_KW: &[&str] = &["as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type", "unsafe", "use", "where", "while"];
const JS_KW: &[&str] = &["async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else", "export", "extends", "finally", "for", "from", "function", "if", "import", "in", "instanceof", "interface", "let", "new", "of", "return", "static", "switch", "this", "throw", "try", "type", "typeof", "var", "void", "while", "yield"];
const PY_KW: &[&str] = &["and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield"];
const SH_KW: &[&str] = &["case", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function", "if", "in", "local", "return", "then", "until", "while", "echo", "cd", "sudo", "set", "source"];
const C_KW: &[&str] = &["auto", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else", "enum", "extern", "for", "func", "go", "goto", "if", "import", "include", "interface", "namespace", "new", "package", "private", "public", "protected", "return", "sizeof", "static", "struct", "switch", "template", "this", "throw", "try", "typedef", "union", "using", "var", "void", "volatile", "while", "defer", "range", "chan", "map"];
const SQL_KW: &[&str] = &["select", "from", "where", "insert", "into", "values", "update", "set", "delete", "create", "table", "drop", "alter", "join", "left", "right", "inner", "outer", "on", "group", "by", "order", "having", "limit", "and", "or", "not", "as", "distinct", "union", "primary", "key", "index"];
const LIT: &[&str] = &["true", "false", "null", "None", "True", "False", "nil", "undefined", "NULL"];

fn lang(name: &str) -> Lang {
    let n = name.to_ascii_lowercase();
    let generic = Lang { line: &["//", "#"], block: Some(("/*", "*/")), keywords: &[], literals: LIT, backtick: false, single: true };
    match n.as_str() {
        "rust" | "rs" => Lang { line: &["//"], block: Some(("/*", "*/")), keywords: RUST_KW, literals: LIT, backtick: false, single: false },
        "js" | "javascript" | "ts" | "typescript" | "jsx" | "tsx" | "mjs" | "cjs" => Lang { line: &["//"], block: Some(("/*", "*/")), keywords: JS_KW, literals: LIT, backtick: true, single: true },
        "py" | "python" => Lang { line: &["#"], block: None, keywords: PY_KW, literals: LIT, backtick: false, single: true },
        "sh" | "bash" | "zsh" | "shell" | "console" | "fish" => Lang { line: &["#"], block: None, keywords: SH_KW, literals: LIT, backtick: true, single: true },
        "c" | "h" | "cpp" | "c++" | "hpp" | "go" | "java" | "kotlin" | "swift" | "cs" | "csharp" => Lang { line: &["//"], block: Some(("/*", "*/")), keywords: C_KW, literals: LIT, backtick: n == "go", single: true },
        "json" | "jsonc" | "json5" => Lang { line: &["//"], block: None, keywords: &[], literals: LIT, backtick: false, single: false },
        "toml" | "yaml" | "yml" | "ini" | "conf" | "dockerfile" | "make" | "makefile" => Lang { line: &["#"], block: None, keywords: &[], literals: LIT, backtick: false, single: true },
        "lua" => Lang { line: &["--"], block: None, keywords: &["and", "break", "do", "else", "elseif", "end", "for", "function", "if", "in", "local", "not", "or", "repeat", "return", "then", "until", "while"], literals: LIT, backtick: false, single: true },
        "sql" => Lang { line: &["--"], block: Some(("/*", "*/")), keywords: SQL_KW, literals: LIT, backtick: false, single: true },
        "css" | "scss" => Lang { line: &[], block: Some(("/*", "*/")), keywords: &[], literals: &[], backtick: false, single: true },
        "html" | "xml" | "svg" => Lang { line: &[], block: Some(("<!--", "-->")), keywords: &[], literals: &[], backtick: false, single: true },
        _ => generic,
    }
}

/// Zerlegt Code in (Bytebereich, Token-Art). Die Bereiche decken den Text lückenlos ab.
pub fn highlight(lang_name: &str, code: &str) -> Vec<(std::ops::Range<usize>, Tok)> {
    let ln = lang_name.to_ascii_lowercase();
    if ln == "diff" || ln == "patch" {
        return diff(code);
    }
    if ln.is_empty() || ln == "text" || ln == "txt" || ln == "plain" || ln == "markdown" || ln == "md" {
        return vec![(0..code.len(), Tok::Plain)];
    }
    let l = lang(&ln);
    let sql = ln == "sql";
    let toml = matches!(ln.as_str(), "toml" | "ini" | "conf");
    let b = code.as_bytes();
    let mut out: Vec<(std::ops::Range<usize>, Tok)> = Vec::new();
    let mut push = |r: std::ops::Range<usize>, t: Tok| {
        if r.is_empty() {
            return;
        }
        if let Some((last, lt)) = out.last_mut() {
            if *lt == t && last.end == r.start {
                last.end = r.end;
                return;
            }
        }
        out.push((r, t));
    };
    let mut i = 0;
    let mut line_start = true;
    while i < b.len() {
        let c = b[i];
        // TOML/INI-Sektion
        if toml && line_start && c == b'[' {
            let e = code[i..].find('\n').map(|p| i + p).unwrap_or(b.len());
            push(i..e, Tok::Section);
            i = e;
            continue;
        }
        if c == b'\n' {
            line_start = true;
            push(i..i + 1, Tok::Plain);
            i += 1;
            continue;
        }
        if !c.is_ascii_whitespace() {
            line_start = false;
        }
        // Zeilenkommentar
        if let Some(m) = l.line.iter().find(|m| code[i..].starts_with(**m)) {
            // `#` in Shell nur als Kommentar, wenn am Wortanfang
            let ok = !(*m == "#" && i > 0 && !b[i - 1].is_ascii_whitespace() && b[i - 1] != b';');
            if ok {
                let e = code[i..].find('\n').map(|p| i + p).unwrap_or(b.len());
                push(i..e, Tok::Comment);
                i = e;
                continue;
            }
        }
        if let Some((s, e)) = l.block {
            if code[i..].starts_with(s) {
                let end = code[i + s.len()..].find(e).map(|p| i + s.len() + p + e.len()).unwrap_or(b.len());
                push(i..end, Tok::Comment);
                i = end;
                continue;
            }
        }
        // Strings
        if c == b'"' || (c == b'\'' && l.single) || (c == b'`' && l.backtick) {
            let mut j = i + 1;
            while j < b.len() && b[j] != c {
                if b[j] == b'\\' {
                    j += 1;
                }
                if c != b'`' && j < b.len() && b[j] == b'\n' {
                    break;
                }
                j += 1;
            }
            let end = (j + 1).min(b.len());
            push(i..end, Tok::String);
            i = end;
            continue;
        }
        // Zahlen
        if c.is_ascii_digit() && (i == 0 || !is_ident(b[i - 1])) {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'.') {
                if b[j] == b'.' && !(j + 1 < b.len() && b[j + 1].is_ascii_digit()) {
                    break;
                }
                j += 1;
            }
            push(i..j, Tok::Number);
            i = j;
            continue;
        }
        // Bezeichner
        if is_ident_start(c) {
            let mut j = i + 1;
            while j < b.len() && is_ident(b[j]) {
                j += 1;
            }
            let w = &code[i..j];
            let kw = if sql { l.keywords.iter().any(|k| k.eq_ignore_ascii_case(w)) } else { l.keywords.contains(&w) };
            let t = if kw {
                Tok::Keyword
            } else if l.literals.contains(&w) {
                Tok::Literal
            } else if j < b.len() && (b[j] == b'(' || (b[j] == b'!' && ln.starts_with("r"))) {
                Tok::Function
            } else if !l.keywords.is_empty() && w.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && w.chars().any(|c| c.is_ascii_lowercase()) {
                Tok::Type
            } else {
                Tok::Plain
            };
            push(i..j, t);
            i = j;
            continue;
        }
        // Unicode-sicher weitergehen
        let len = utf8_len(c);
        push(i..(i + len).min(b.len()), Tok::Plain);
        i += len;
    }
    out
}

fn diff(code: &str) -> Vec<(std::ops::Range<usize>, Tok)> {
    let mut out = Vec::new();
    let mut pos = 0;
    for line in code.split_inclusive('\n') {
        let t = if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
            Tok::Section
        } else if line.starts_with('+') {
            Tok::Added
        } else if line.starts_with('-') {
            Tok::Removed
        } else {
            Tok::Plain
        };
        out.push((pos..pos + line.len(), t));
        pos += line.len();
    }
    out
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'$'
}
fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}
fn utf8_len(c: u8) -> usize {
    match c {
        0xF0..=0xFF => 4,
        0xE0..=0xEF => 3,
        0xC0..=0xDF => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(lang: &str, code: &str) -> Vec<(String, Tok)> {
        highlight(lang, code).into_iter().filter(|(_, t)| *t != Tok::Plain).map(|(r, t)| (code[r].to_string(), t)).collect()
    }

    #[test]
    fn rust() {
        let k = kinds("rust", "fn main() { let s = \"ä\"; // hi\n 42 }");
        assert!(k.contains(&("fn".into(), Tok::Keyword)));
        assert!(k.contains(&("main".into(), Tok::Function)));
        assert!(k.contains(&("\"ä\"".into(), Tok::String)));
        assert!(k.contains(&("// hi".into(), Tok::Comment)));
        assert!(k.contains(&("42".into(), Tok::Number)));
    }

    #[test]
    fn abdeckung_lueckenlos() {
        let code = "x = 'ü' # c\nprint(1.5)\n€";
        let r = highlight("python", code);
        let mut p = 0;
        for (rg, _) in &r {
            assert_eq!(rg.start, p);
            p = rg.end;
        }
        assert_eq!(p, code.len());
    }

    #[test]
    fn shell_und_diff() {
        let k = kinds("bash", "echo $HOME#x # kommentar");
        assert!(k.contains(&("# kommentar".into(), Tok::Comment)));
        assert!(!k.iter().any(|(s, _)| s == "#x"));
        let d = kinds("diff", "@@ -1 +1 @@\n-a\n+b\n");
        assert_eq!(d[1].1, Tok::Removed);
        assert_eq!(d[2].1, Tok::Added);
    }
}
