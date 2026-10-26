//! Lokale Sprachausgabe für Remote-Sessions.
//!
//! Die pi-Extension `vorlesen` spricht auf dem Rechner, auf dem pi läuft – bei einer Remote-
//! Instanz also auf dem Server. copi spricht deshalb selbst, mit derselben Piper-Stimme und
//! demselben dauerhaften Zustand (`~/.pi/agent/vorlesen/state.json`: `enabled`, `voice`) wie
//! `/vorlesen an|aus`. Es gibt keinen Zusammenfassungsaufruf (kein Modell, keine Kosten):
//! kurze Antworten werden ganz gesprochen, bei langen der Anfang bis zu einer Satzgrenze.
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

const DEFAULT_VOICE: &str = "de_DE-thorsten-high";
/// Antworten bis zu dieser Länge (Zeichen, nach Bereinigung) werden ganz gesprochen.
const DIRECT_LIMIT: usize = 280;
/// Bei längeren Antworten: höchstens so viel vom Anfang, an einer Satzgrenze beendet.
const LEAD_LIMIT: usize = 400;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn piper_bin() -> PathBuf {
    home().join(".local/lib/piper/piper")
}

fn voice_file(voice: &str) -> PathBuf {
    home().join(".local/share/piper/voices").join(format!("{voice}.onnx"))
}

fn state_file() -> PathBuf {
    home().join(".pi/agent/vorlesen/state.json")
}

fn state() -> serde_json::Value {
    std::fs::read_to_string(state_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null)
}

pub fn voice() -> String {
    state()["voice"].as_str().filter(|v| !v.is_empty()).unwrap_or(DEFAULT_VOICE).to_string()
}

/// Dauerhafter Schalter (wie `/vorlesen an|aus`); ohne Datei: an.
pub fn enabled() -> bool {
    state()["enabled"].as_bool().unwrap_or(true)
}

/// Schalter dauerhaft setzen; übrige Felder (Stimme) bleiben erhalten.
pub fn set_enabled(on: bool) -> Result<(), String> {
    let mut v = state();
    if !v.is_object() {
        v = serde_json::json!({});
    }
    v["enabled"] = serde_json::json!(on);
    let f = state_file();
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&f, serde_json::to_string_pretty(&v).unwrap_or_default()).map_err(|e| e.to_string())
}

fn in_path(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

/// Piper, die gewählte Stimme und `pw-play` sind lokal vorhanden.
pub fn available() -> bool {
    piper_bin().is_file() && voice_file(&voice()).is_file() && in_path("pw-play")
}

fn sample_rate(voice: &str) -> String {
    let p = format!("{}.json", voice_file(voice).display());
    std::fs::read_to_string(p)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["audio"]["sample_rate"].as_u64())
        .unwrap_or(22050)
        .to_string()
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x2600..=0x27BF | 0x1F000..=0x1FAFF)
}

/// Markdown in sprechbaren Text umwandeln (wie `toSpeakable` der Extension).
pub fn speakable(md: &str) -> String {
    let mut out = String::new();
    let mut fenced = false;
    for line in md.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            fenced = !fenced;
            out.push(' ');
            continue;
        }
        if fenced || (t.starts_with('|') && t.ends_with('|') && t.len() > 1) {
            continue;
        }
        let mut l = t.trim_start_matches('#').trim_start().to_string();
        if let Some(r) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")).or_else(|| l.strip_prefix("+ ")) {
            l = r.to_string();
        } else if let Some(i) = l.find(". ") {
            if i > 0 && l[..i].chars().all(|c| c.is_ascii_digit()) {
                l = l[i + 2..].to_string();
            }
        }
        out.push_str(&l);
        out.push(' ');
    }
    let out = strip_links(&out);
    let words: Vec<String> = out
        .split_whitespace()
        .filter(|w| !(w.starts_with("http://") || w.starts_with("https://")))
        .map(|w| w.chars().filter(|c| !matches!(c, '`' | '*' | '_' | '~' | '>') && !is_emoji(*c)).collect::<String>())
        .filter(|w| !w.is_empty())
        .collect();
    words.join(" ")
}

/// `![alt](url)` entfällt, `[text](url)` wird zu `text`.
fn strip_links(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut o = String::new();
    let mut i = 0;
    while i < cs.len() {
        let image = cs[i] == '!' && cs.get(i + 1) == Some(&'[');
        let st = if image { i + 1 } else { i };
        if cs[st] == '[' {
            if let Some(close) = cs[st..].iter().position(|c| *c == ']').map(|p| st + p) {
                if cs.get(close + 1) == Some(&'(') {
                    if let Some(end) = cs[close..].iter().position(|c| *c == ')').map(|p| close + p) {
                        if !image {
                            o.extend(&cs[st + 1..close]);
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        o.push(cs[i]);
        i += 1;
    }
    o
}

/// Was von einer Antwort gesprochen wird: kurz = ganz, lang = Anfang bis zur Satzgrenze.
pub fn lead(md: &str) -> String {
    let all = speakable(md);
    if all.chars().count() <= DIRECT_LIMIT {
        return all;
    }
    let first = md.split("\n\n").map(speakable).find(|p| !p.is_empty()).unwrap_or(all);
    if first.chars().count() <= LEAD_LIMIT {
        return first;
    }
    let cut: String = first.chars().take(LEAD_LIMIT).collect();
    match cut.rfind(['.', '!', '?']) {
        Some(i) => cut[..=i].to_string(),
        None => cut,
    }
}

/// Laufende Ausgabe (Piper → pw-play).
#[derive(Default)]
pub struct Speech {
    piper: Option<Child>,
    player: Option<Child>,
}

impl Speech {
    pub fn stop(&mut self) {
        for mut c in [self.piper.take(), self.player.take()].into_iter().flatten() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub fn speak(&mut self, text: &str) -> Result<(), String> {
        self.stop();
        if text.trim().is_empty() {
            return Ok(());
        }
        let v = voice();
        let mut pp = Command::new(piper_bin())
            .args(["-m"])
            .arg(voice_file(&v))
            .args(["--output_raw", "-q"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Piper startet nicht: {e}"))?;
        let out = pp.stdout.take().ok_or("Piper ohne Ausgabe")?;
        let pl = Command::new("pw-play")
            .args(["--raw", "--format", "s16", "--rate", &sample_rate(&v), "--channels", "1", "--media-role", "Communication", "-"])
            .stdin(Stdio::from(out))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let pl = match pl {
            Ok(p) => p,
            Err(e) => {
                let _ = pp.kill();
                let _ = pp.wait();
                return Err(format!("pw-play startet nicht: {e}"));
            }
        };
        if let Some(mut si) = pp.stdin.take() {
            let _ = si.write_all(format!("{text}\n").as_bytes());
        }
        self.piper = Some(pp);
        self.player = Some(pl);
        Ok(())
    }
}

impl Drop for Speech {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_becomes_speakable() {
        let md = "# Titel\n\n- **Fett** und `code`\n1. Eins\n```rs\nlet x = 1;\n```\n| a | b |\nSiehe [Doku](https://x.de) oder https://y.de ![Bild](b.png) fertig 🔊.";
        assert_eq!(speakable(md), "Titel Fett und code Eins Siehe Doku oder fertig .");
    }

    #[test]
    fn short_answers_are_spoken_whole_long_ones_by_lead() {
        assert_eq!(lead("Fertig. Alles grün."), "Fertig. Alles grün.");
        let long = format!("{}\n\nZweiter Absatz.", "Ein Satz. ".repeat(60));
        let l = lead(&long);
        assert!(l.chars().count() <= LEAD_LIMIT && l.ends_with('.') && !l.contains("Zweiter"));
        let one = "wort ".repeat(120);
        assert_eq!(lead(&one).chars().count(), LEAD_LIMIT);
    }
}
