//! pi-backend – GUI-freie Bausteine für pi-Frontends.
//!
//! Aus pi-lite (`src-tauri`) übernommen und von Tauri gelöst: Statt Events zu
//! emittieren, schreiben Lese-Threads [`Event`]s in einen `mpsc`-Kanal und rufen
//! danach einen `notify`-Callback auf (z. B. `ctx.request_repaint()`).

pub mod cli;
pub mod editor;
pub mod folders;
pub mod rpc;
pub mod sessions;

/// Alles, was aus Hintergrund-Threads zurück an die Oberfläche geht.
#[derive(Debug)]
pub enum Event {
    /// Eine JSONL-Zeile von `pi --mode rpc` (Generation des Prozesses).
    Line(u64, String),
    /// stderr-Zeile von pi.
    Stderr(u64, String),
    /// Der pi-Prozess dieser Generation hat sich beendet.
    Exit(u64),
    /// Ausgabezeile eines `pi <unterbefehl>`-Laufs.
    CliLine { id: String, stderr: bool, line: String },
    /// `pi <unterbefehl>` ist fertig (Exit-Code oder Fehlertext).
    CliDone { id: String, result: Result<i32, String> },
}

/// Rückruf nach jedem gesendeten Event (thread-sicher).
pub type Notify = std::sync::Arc<dyn Fn() + Send + Sync>;

/// Sender plus Weckruf – wird an Hintergrund-Threads weitergereicht.
#[derive(Clone)]
pub struct Sink {
    tx: std::sync::mpsc::Sender<Event>,
    notify: Notify,
}

impl Sink {
    pub fn new(tx: std::sync::mpsc::Sender<Event>, notify: Notify) -> Self {
        Self { tx, notify }
    }
    pub fn send(&self, ev: Event) {
        if self.tx.send(ev).is_ok() {
            (self.notify)();
        }
    }
}

/// Agent-Verzeichnis von pi (`PI_CODING_AGENT_DIR` oder `~/.pi/agent`).
pub fn agent_dir() -> Option<std::path::PathBuf> {
    if let Ok(d) = std::env::var("PI_CODING_AGENT_DIR") {
        return Some(d.into());
    }
    home().map(|h| h.join(".pi/agent"))
}

pub fn home() -> Option<std::path::PathBuf> {
    std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok().map(Into::into)
}

/// `~` für das Home-Verzeichnis einsetzen (Anzeige).
pub fn tilde(path: &str) -> String {
    match home() {
        Some(h) => {
            let h = h.to_string_lossy();
            match path.strip_prefix(h.as_ref()) {
                Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
                _ => path.to_string(),
            }
        }
        None => path.to_string(),
    }
}
