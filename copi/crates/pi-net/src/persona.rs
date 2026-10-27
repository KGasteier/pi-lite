//! Nebenagenten mit eigener Persönlichkeit („Personas“).
//!
//! Definition als Markdown-Datei mit Front-Matter in `<config>/copi/personas/*.md`:
//!
//! ```text
//! ---
//! name: Sokrates
//! description: Fragt nach, statt Antworten zu geben
//! model: udk2/cc/claude-sonnet-5-5   (optional)
//! tools: none | read                 (optional, Standard none)
//! color: #b08cf0                     (optional)
//! ---
//! Systemprompt …
//! ```
//!
//! Personas laufen als eigene `pi --mode rpc`-Instanz mit `--system-prompt` und standardmäßig
//! **ohne Werkzeuge** (`--no-tools`), ohne Erweiterungen und Skills. Werkzeuge nur nach
//! ausdrücklicher Angabe `tools: read` (nur Lesen).
use serde::{Deserialize, Serialize};

use crate::model::PersonaId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PersonaTools {
    #[default]
    None,
    Read,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Persona {
    pub id: PersonaId,
    pub name: String,
    pub description: String,
    pub model: Option<String>,
    pub tools: PersonaTools,
    /// Farbe als `#rrggbb`.
    pub color: String,
    pub system_prompt: String,
    /// Mitgeliefert (nicht editierbar) oder aus Datei.
    pub builtin: bool,
}

impl Persona {
    /// Front-Matter-Datei parsen. `id` = Dateiname ohne Endung.
    pub fn parse(id: &str, text: &str) -> Result<Persona, String> {
        let _ = (id, text);
        Err("noch nicht implementiert".into())
    }
    /// Als Datei serialisieren (Gegenstück zu [`Persona::parse`]).
    pub fn to_markdown(&self) -> String {
        String::new()
    }
    /// Zusätzliche pi-Argumente für die Instanz (nach `--mode rpc`).
    pub fn pi_args(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Mitgelieferte Beispiel-Personas (Platzhalter, bis eigene gebaut sind).
pub fn builtins() -> Vec<Persona> {
    Vec::new()
}

/// Alle Personas: mitgelieferte + Dateien aus `dir` (Dateien mit gleicher ID überschreiben).
/// Fehlerhafte Dateien werden übersprungen und als (Dateiname, Fehler) gemeldet.
pub fn load_all(dir: &std::path::Path) -> (Vec<Persona>, Vec<(String, String)>) {
    let _ = dir;
    (builtins(), Vec::new())
}

/// Persona-Datei atomar speichern; `id` muss `[a-z0-9-]{1,40}` sein.
pub fn save(dir: &std::path::Path, p: &Persona) -> Result<std::path::PathBuf, String> {
    let _ = (dir, p);
    Err("noch nicht implementiert".into())
}
