//! Nebenagenten mit eigener Persönlichkeit („Personas“).
//!
//! Markdown mit einfachem Front-Matter (kein YAML): `name`, `description`, `model`,
//! `tools: none|read`, `color: #rrggbb` zwischen zwei `---`-Zeilen, danach Systemprompt.
//! Lokale Definitionen überschreiben Beispiele mit gleicher Dateikennung.
//! pi-Argumente sind mit `pi --help` von pi 1.4.0 abgeglichen; kein Shell-Aufruf.
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

use crate::model::PersonaId;

const MAX_FILE_BYTES: usize = 64 * 1024;
const MAX_FILES: usize = 100;

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
        validate_id(id)?;
        // split_inclusive erhält den Prompt einschließlich abschließendem Zeilenumbruch.
        let mut lines = text.split_inclusive('\n');
        if lines.next().map(str::trim) != Some("---") {
            return Err("Front-Matter muss mit einer --- Zeile beginnen".into());
        }
        let mut fields = std::collections::HashMap::new();
        let mut closed = false;
        for line in lines.by_ref() {
            let line = line.trim();
            if line == "---" {
                closed = true;
                break;
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                return Err(format!(
                    "Front-Matter: Schlüssel: Wert erwartet, gefunden: {line}"
                ));
            };
            let key = key.trim();
            if !matches!(key, "name" | "description" | "model" | "tools" | "color") {
                continue;
            }
            if fields
                .insert(key.to_string(), decode_field(value.trim())?)
                .is_some()
            {
                return Err(format!("Front-Matter: Schlüssel {key} ist doppelt"));
            }
        }
        if !closed {
            return Err("Abschließende --- Zeile im Front-Matter fehlt".into());
        }
        let name = fields.remove("name").ok_or("Name fehlt (name)")?;
        if name.trim().is_empty() || name.chars().count() > 40 || name.chars().any(forbidden_char) {
            return Err(
                "Name muss 1 bis 40 Zeichen ohne Steuer- oder unsichtbare Zeichen enthalten".into(),
            );
        }
        let model = fields.remove("model").filter(|s| !s.is_empty());
        if model.as_ref().is_some_and(|m| {
            !m.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._/:-".contains(&c))
        }) {
            return Err("Modell darf nur A–Z, a–z, 0–9 und . _ / : - enthalten".into());
        }
        let tools = match fields.get("tools").map(String::as_str).unwrap_or("none") {
            "none" => PersonaTools::None,
            "read" => PersonaTools::Read,
            _ => return Err("Werkzeuge (tools) müssen none oder read sein".into()),
        };
        let color = fields
            .remove("color")
            .filter(|c| valid_color(c))
            .unwrap_or_else(|| default_color(id));
        let system_prompt = lines.collect::<String>();
        if system_prompt.trim().is_empty() || system_prompt.chars().count() > 8000 {
            return Err("Systemprompt muss 1 bis 8000 Zeichen enthalten".into());
        }
        Ok(Self {
            id: id.into(),
            name,
            description: fields.remove("description").unwrap_or_default(),
            model,
            tools,
            color,
            system_prompt,
            builtin: false,
        })
    }

    /// Als Datei serialisieren (Gegenstück zu [`Persona::parse`]).
    /// Textfelder werden JSON-quotiert, damit auch Doppelpunkte/Zeilenumbrüche rundlaufen.
    /// `builtin` ist Herkunft, kein Dateifeld: eingelesene Dateien sind immer lokal.
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "---\nname: {}\ndescription: {}\n",
            quote(&self.name),
            quote(&self.description)
        );
        if let Some(model) = &self.model {
            out.push_str(&format!("model: {}\n", quote(model)));
        }
        out.push_str(&format!(
            "tools: {}\ncolor: {}\n---\n{}",
            match self.tools {
                PersonaTools::None => "none",
                PersonaTools::Read => "read",
            },
            quote(&self.color),
            self.system_prompt
        ));
        out
    }

    /// Zusätzliche pi-Argumente für die Instanz (nach `--mode rpc`).
    /// Lesewerkzeuge sind keine Dateisystem-Sandbox; nur ausdrücklich konfigurieren.
    pub fn pi_args(&self) -> Vec<String> {
        let mut args = vec![
            "--system-prompt".into(),
            format!(
                "Du bist {}, ein Nebenagent in einem cocopi-Chat. Antworte kurz auf Deutsch und bleibe bei deiner Rolle.\n{}",
                crate::a2a::sanitize(&self.name, 160),
                self.system_prompt
            ),
            "--no-session".into(),
            "--no-extensions".into(),
            "--no-skills".into(),
            "--no-context-files".into(),
        ];
        match self.tools {
            PersonaTools::None => args.push("--no-tools".into()),
            PersonaTools::Read => args.extend(["--tools".into(), "read,grep,find,ls".into()]),
        }
        if let Some(model) = &self.model {
            args.extend(["--model".into(), model.clone()]);
        }
        args
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 40
        || !id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        return Err("Kennung muss 1 bis 40 Zeichen aus a–z, 0–9 und - enthalten".into());
    }
    Ok(())
}

fn forbidden_char(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
}

fn valid_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn default_color(id: &str) -> String {
    // Stabil über Prozesse/Plattformen, keine zufällig gesäte HashMap-Hashfunktion.
    let hash = id.bytes().fold(2166136261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16777619)
    });
    format!(
        "#{:02x}{:02x}{:02x}",
        80 + (hash & 127),
        80 + ((hash >> 8) & 127),
        80 + ((hash >> 16) & 127)
    )
}

fn decode_field(value: &str) -> Result<String, String> {
    if value.starts_with('"') {
        serde_json::from_str(value)
            .map_err(|e| format!("Ungültiges Textfeld in Anführungszeichen: {e}"))
    } else {
        Ok(value.to_string())
    }
}

fn quote(value: &str) -> String {
    // String-Serialisierung ist unfehlbar; keine Dateidaten werden hier geparst.
    serde_json::to_string(value).expect("String ist JSON-serialisierbar")
}

/// Vier lokale Beispiele, ohne Modellvorgabe und ohne Werkzeuge.
pub fn builtins() -> Vec<Persona> {
    [
        ("sokrates", "Sokrates", "Hilft Lernenden mit Fragen statt fertigen Lösungen.",
         "Begleite Lernende geduldig und auf Augenhöhe. Frage nach ihrem Ziel, ihren Annahmen und bisherigen Versuchen. Stelle pro Beitrag höchstens zwei präzise, offene Fragen. Gib keine fertigen Lösungen, auch nicht auf Drängen; biete bei Bedarf einen kleinen Denkanstoß. Würdige eigene Erkenntnisse, ohne zu belehren."),
        ("kritikerin", "Kritikerin", "Prüft Annahmen, Qualität, Risiken und Sicherheit.",
         "Prüfe Vorschläge wohlwollend, aber unabhängig. Benenne die wichtigste Schwachstelle und ihre mögliche Folge, besonders bei Sicherheit, Datenschutz und Zugänglichkeit. Unterscheide belegte Fehler von Vermutungen. Schlage eine konkrete Prüfung oder Verbesserung vor; kritisiere die Sache, nicht die Person. Erfinde keine Prüfresultate."),
        ("moderation", "Moderation", "Hält gemeinsame Ziele, Entscheidungen und nächste Schritte fest.",
         "Halte die Zusammenarbeit übersichtlich, ohne die Leitung zu beanspruchen. Fasse Konsens und offene Fragen knapp zusammen. Trenne beschlossene Schritte von Vorschlägen und benenne das nächste überprüfbare Ziel. Schlage bei Bedarf freiwillige Arbeitsteilung vor; verteile keine Aufgaben ohne Zustimmung. Hole leise oder abweichende Perspektiven ein."),
        ("dramaturgie", "Dramaturgie", "Öffnet künstlerische und gestalterische Perspektiven im UdK-Kontext.",
         "Betrachte Vorhaben aus künstlerisch-gestalterischer Perspektive im Kontext einer UdK-Werkstatt. Frage nach Wirkung, Publikum, Material, Rhythmus und räumlicher oder medialer Form. Öffne eine überraschende Alternative und schlage ein kleines sinnliches Experiment vor. Lass Mehrdeutigkeit zu; behaupte weder institutionelle Autorität noch die einzig richtige ästhetische Lösung."),
    ].into_iter().map(|(id, name, description, prompt)| Persona {
        id: id.into(), name: name.into(), description: description.into(), model: None,
        tools: PersonaTools::None, color: default_color(id), system_prompt: prompt.into(), builtin: true,
    }).collect()
}

/// Alle Personas: mitgelieferte + höchstens 100 reguläre `.md`-Dateien (je ≤ 64 KiB).
/// Fehlerhafte Dateien werden übersprungen und als (Dateiname, Fehler) gemeldet.
/// Symlinks werden nicht verfolgt. Ein fehlender Ordner ist ein normaler Erststart.
pub fn load_all(dir: &std::path::Path) -> (Vec<Persona>, Vec<(String, String)>) {
    let mut personas = builtins();
    let mut errors = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => Some(entries),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            errors.push((
                dir.display().to_string(),
                format!("Ordner nicht lesbar: {e}"),
            ));
            None
        }
    };
    let mut files = Vec::new();
    if let Some(entries) = entries {
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    errors.push((dir.display().to_string(), e.to_string()));
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            // Begrenzung auch für ungültige Dateien; keine unbegrenzte Verzeichnisliste sammeln.
            if files.len() == MAX_FILES {
                errors.push((
                    dir.display().to_string(),
                    "Höchstens 100 Persona-Dateien werden geladen".into(),
                ));
                break;
            }
            files.push(path);
        }
    }
    files.sort();
    for path in files {
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let result = (|| {
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Dateiname ist kein UTF-8")?;
            validate_id(id)?;
            let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !meta.is_file() {
                return Err("Keine reguläre Persona-Datei (Symlinks sind nicht erlaubt)".into());
            }
            if meta.len() > MAX_FILE_BYTES as u64 {
                return Err("Persona-Datei ist größer als 64 KiB".into());
            }
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let opened = file.metadata().map_err(|e| e.to_string())?;
                if opened.dev() != meta.dev() || opened.ino() != meta.ino() {
                    return Err("Persona-Datei wurde während des Öffnens ausgetauscht".into());
                }
            }
            let mut bytes = Vec::new();
            file.take(MAX_FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > MAX_FILE_BYTES {
                return Err("Persona-Datei ist größer als 64 KiB".into());
            }
            let text = std::str::from_utf8(&bytes).map_err(|_| "Persona-Datei ist kein UTF-8")?;
            Persona::parse(id, text)
        })();
        match result {
            Ok(p) => {
                personas.retain(|old| old.id != p.id);
                personas.push(p);
            }
            Err(e) => errors.push((file_name, e)),
        }
    }
    personas.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    (personas, errors)
}

/// Persona-Datei atomar speichern; `id` muss `[a-z0-9-]{1,40}` sein.
/// Eine vorhandene `.new`-Datei wird nicht überschrieben (auch kein Symlink).
pub fn save(dir: &std::path::Path, p: &Persona) -> Result<std::path::PathBuf, String> {
    validate_id(p.id.as_str())?;
    let text = p.to_markdown();
    Persona::parse(p.id.as_str(), &text)?;
    if text.len() > MAX_FILE_BYTES {
        return Err("Persona-Datei ist größer als 64 KiB".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Persona-Ordner nicht angelegt: {e}"))?;
    let dest = dir.join(format!("{}.md", p.id));
    let temp = dir.join(format!("{}.md.new", p.id));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp)
        .map_err(|e| format!("Temporäre Persona-Datei nicht angelegt: {e}"))?;
    let result = (|| {
        file.write_all(text.as_bytes())
            .map_err(|e| format!("Persona nicht geschrieben: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Persona nicht gesichert: {e}"))?;
        drop(file);
        std::fs::rename(&temp, &dest).map_err(|e| format!("Persona nicht ersetzt: {e}"))?;
        Ok(dest)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("cocopi-persona-{}", crate::model::random_id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn parse(fields: &str, prompt: &str) -> Result<Persona, String> {
        Persona::parse("test-1", &format!("---\n{fields}\n---\n{prompt}"))
    }
    #[test]
    fn parse_defaults_and_all_fields() {
        let p = parse("name: Änne\nunknown: ignoriert", "Frag nach.\n").unwrap();
        assert_eq!(p.name, "Änne");
        assert_eq!(p.system_prompt, "Frag nach.\n");
        assert_eq!(p.tools, PersonaTools::None);
        assert_eq!(p.model, None);
        assert!(!p.builtin);
        assert!(valid_color(&p.color));
        let p = parse("name: Test\ndescription: kurz: klar\nmodel: udk2/cc/model-1:high\ntools: read\ncolor: #aAbBcC", "Text").unwrap();
        assert_eq!(p.description, "kurz: klar");
        assert_eq!(p.tools, PersonaTools::Read);
        assert_eq!(p.color, "#aAbBcC");
        assert_eq!(p.model.as_deref(), Some("udk2/cc/model-1:high"));
        let crlf = Persona::parse("crlf", "---\r\nname: Test\r\n---\r\nHallo\r\n").unwrap();
        assert_eq!(crlf.system_prompt, "Hallo\r\n");
    }
    #[test]
    fn round_trip_quotes_colons_newlines_and_builtins() {
        let mut p = parse("name: Test", "\n---\nPrompt: \"x\"\n").unwrap();
        p.name = "Test: \"eins\"".into();
        p.description = "mehr\nZeilen: \\\"".into();
        assert_eq!(Persona::parse(p.id.as_str(), &p.to_markdown()).unwrap(), p);
        let examples = builtins();
        assert_eq!(examples.len(), 4);
        for mut p in examples {
            assert!(p.builtin);
            assert_eq!(p.tools, PersonaTools::None);
            assert!(p.model.is_none());
            assert!(p.system_prompt.chars().count() > 100);
            p.builtin = false;
            assert_eq!(Persona::parse(p.id.as_str(), &p.to_markdown()).unwrap(), p);
        }
    }
    #[test]
    fn parse_errors_limits_and_color_fallback() {
        for id in ["", "../x", "a/b", "A", "ä", ".", "x_y", &"a".repeat(41)] {
            assert!(
                Persona::parse(id, "---\nname: X\n---\nPrompt").is_err(),
                "{id}"
            );
        }
        for bad in [
            "name: x\n---\nPrompt",
            "---\nname: x",
            "---\n---\nPrompt",
            "---\nname: x\n---\n \n",
        ] {
            assert!(Persona::parse("x", bad).is_err());
        }
        for fields in [
            "name:",
            "name: x\nname: y",
            "name: x\ntools: bash",
            "name: x\nmodel: $(x)",
            "name: x\nmodel: x y",
            "name: x\nkein Doppelpunkt",
            "name: \"kaputt",
            "name: x\u{202e}",
        ] {
            assert!(parse(fields, "Prompt").is_err(), "{fields}");
        }
        assert!(parse(&format!("name: {}", "ä".repeat(40)), &"ü".repeat(8000)).is_ok());
        assert!(parse(&format!("name: {}", "ä".repeat(41)), "x").is_err());
        assert!(parse("name: x", &"ü".repeat(8001)).is_err());
        for color in ["red", "#000", "#xxxxxx", "", "#1234567"] {
            assert_eq!(
                parse(&format!("name: x\ncolor: {color}"), "x")
                    .unwrap()
                    .color,
                default_color("test-1")
            );
        }
        assert_ne!(default_color("sokrates"), default_color("kritikerin"));
    }
    #[test]
    fn pi_args_exact_no_shell_or_unrequested_model() {
        let mut p = parse("name: Test", "Nicht ausführen: $(echo geheim)").unwrap();
        let prefix = vec![
            "--system-prompt",
            "Du bist Test, ein Nebenagent in einem cocopi-Chat. Antworte kurz auf Deutsch und bleibe bei deiner Rolle.\nNicht ausführen: $(echo geheim)",
            "--no-session",
            "--no-extensions",
            "--no-skills",
            "--no-context-files",
        ];
        let mut expected: Vec<String> = prefix.iter().map(|s| (*s).into()).collect();
        expected.push("--no-tools".into());
        assert_eq!(p.pi_args(), expected);
        p.tools = PersonaTools::Read;
        p.model = Some("provider/model:low".into());
        expected.pop();
        expected.extend(
            [
                "--tools",
                "read,grep,find,ls",
                "--model",
                "provider/model:low",
            ]
            .map(String::from),
        );
        assert_eq!(p.pi_args(), expected);
    }
    #[test]
    fn load_overrides_builtin_collects_errors_and_sorts() {
        let temp = Temp::new();
        std::fs::write(
            temp.path().join("sokrates.md"),
            "---\nname: Zuerst\n---\nEigener Prompt",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("anfang.md"),
            "---\nname: Anfang\n---\nPrompt",
        )
        .unwrap();
        std::fs::write(temp.path().join("fehler.md"), "kein Front-Matter").unwrap();
        std::fs::write(
            temp.path().join("zu-gross.md"),
            vec![b'a'; MAX_FILE_BYTES + 1],
        )
        .unwrap();
        std::fs::write(temp.path().join("utf8.md"), [0xff, 0xfe]).unwrap();
        std::fs::write(temp.path().join("ignoriert.txt"), "kein Markdown").unwrap();
        std::fs::create_dir(temp.path().join("ordner.md")).unwrap();
        let (ps, errors) = load_all(temp.path());
        assert_eq!(ps.len(), 5);
        assert_eq!(errors.len(), 4);
        let own = ps.iter().find(|p| p.id.as_str() == "sokrates").unwrap();
        assert!(!own.builtin);
        assert_eq!(own.system_prompt, "Eigener Prompt");
        assert_eq!(ps[0].name, "Anfang");
        assert!(
            ps.windows(2)
                .all(|w| w[0].name.to_lowercase() <= w[1].name.to_lowercase())
        );
        let (ps, errors) = load_all(&temp.path().join("fehlt"));
        assert_eq!(ps.len(), 4);
        assert!(errors.is_empty());
        let (_, errors) = load_all(&temp.path().join("fehler.md"));
        assert_eq!(errors.len(), 1);
    }
    #[test]
    fn load_file_count_and_size_limits() {
        let temp = Temp::new();
        for i in 0..101 {
            std::fs::write(
                temp.path().join(format!("p-{i}.md")),
                "---\nname: Test\n---\nPrompt",
            )
            .unwrap();
        }
        let (ps, errors) = load_all(temp.path());
        assert_eq!(ps.len(), 104);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].1.contains("100"));
        let text = format!(
            "---\nname: x\ndescription: {}\n---\nx",
            "d".repeat(MAX_FILE_BYTES - 37)
        );
        // Exakt 64 KiB darf gelesen werden, ein Byte mehr nicht.
        let mut bytes = text.into_bytes();
        bytes.resize(MAX_FILE_BYTES, b' ');
        let boundary = Temp::new();
        let path = boundary.path().join("grenze.md");
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(bytes.len(), MAX_FILE_BYTES);
        let (ps, errors) = load_all(boundary.path());
        assert!(errors.is_empty());
        assert!(ps.iter().any(|p| p.id.as_str() == "grenze"));
        bytes.push(b' ');
        std::fs::write(path, bytes).unwrap();
        let (ps, errors) = load_all(boundary.path());
        assert_eq!(ps.len(), 4);
        assert_eq!(errors.len(), 1);
    }
    #[test]
    fn save_atomic_round_trip_create_directory_and_validate() {
        let temp = Temp::new();
        let dir = temp.path().join("neu/personas");
        let mut p = parse("name: Test", "Prompt").unwrap();
        let dest = save(&dir, &p).unwrap();
        assert_eq!(dest, dir.join("test-1.md"));
        assert_eq!(
            Persona::parse("test-1", &std::fs::read_to_string(&dest).unwrap()).unwrap(),
            p
        );
        assert!(!dir.join("test-1.md.new").exists());
        p.system_prompt = "Neue Fassung".into();
        save(&dir, &p).unwrap();
        assert!(
            std::fs::read_to_string(&dest)
                .unwrap()
                .ends_with("Neue Fassung")
        );
        std::fs::write(dir.join("test-1.md.new"), "nicht überschreiben").unwrap();
        assert!(save(&dir, &p).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.join("test-1.md.new")).unwrap(),
            "nicht überschreiben"
        );
        let blocked = dir.join("blockiert.md");
        std::fs::create_dir(&blocked).unwrap();
        let mut invalid_target = p.clone();
        invalid_target.id = "blockiert".into();
        assert!(save(&dir, &invalid_target).is_err());
        assert!(!dir.join("blockiert.md.new").exists());
        assert!(blocked.is_dir());
        p.id = "../ausbruch".into();
        assert!(save(&dir, &p).is_err());
        p.id = "ok".into();
        p.name.clear();
        assert!(save(&dir, &p).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dest).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_loaded_or_followed_when_saving() {
        use std::os::unix::fs::symlink;
        let temp = Temp::new();
        let outside = temp.path().join("a.txt");
        std::fs::write(&outside, "---\nname: X\n---\nPrompt").unwrap();
        symlink(&outside, temp.path().join("link.md")).unwrap();
        let (ps, errors) = load_all(temp.path());
        assert_eq!(ps.len(), 4);
        assert_eq!(errors.len(), 1);
        let p = parse("name: Test", "Prompt").unwrap();
        symlink(&outside, temp.path().join("test-1.md.new")).unwrap();
        assert!(save(temp.path(), &p).is_err());
        assert!(
            std::fs::read_to_string(outside)
                .unwrap()
                .contains("name: X")
        );
    }
}
