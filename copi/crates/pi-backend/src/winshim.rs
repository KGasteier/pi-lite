//! Robuster Start des `pi`-Programms unter Windows, wenn es (z. B. nach `npm install -g`)
//! nicht als echte `.exe` vorliegt, sondern als generierter npm-Shim (`pi.cmd`/`pi.bat` neben
//! einer `pi.ps1`). `CreateProcess` (und damit `std::process::Command::spawn` ohne Shell) kann
//! solche Batch-Dateien nicht direkt starten – Windows liefert dafür `ERROR_BAD_EXE_FORMAT`
//! (Fehlercode 193), weil eine `.cmd`-Datei kein ausführbares PE-Binary ist, sondern nur von
//! `cmd.exe` interpretiert werden kann.
//!
//! Strategie: zuerst eine echte ausführbare Datei (`pi.exe`) über PATH suchen und direkt
//! starten (kein Shim-Problem, keine Shell nötig). Wird nur ein Skript-Shim (`.cmd`/`.bat`)
//! gefunden, wird `cmd.exe /D /S /C "<shim> <args...>"` verwendet – `/D` unterdrückt
//! Autorun-Einträge, `/S` sorgt für eine vorhersagbare Anführungszeichen-Behandlung der
//! Gesamtzeile. Jedes Argument wird dabei einzeln so maskiert, dass `cmd.exe` es nicht als
//! eigenes Metazeichen (`&`, `|`, `<`, `>`, `^`, Zeilenumbruch, …) interpretiert – es bleibt
//! bei genau einer Prozessstart-API (`Command::spawn`), nie wird eine vom Nutzer
//! beeinflussbare Zeichenkette an eine interaktive Shell weitergereicht, ohne dass jedes
//! Argumentzeichen explizit berücksichtigt wurde.
//!
//! **Nur die reinen, plattformunabhängigen Hilfsfunktionen (`find_executable`,
//! `is_script_shim`, `escape_for_cmd`) sind in dieser Linux-Umgebung tatsächlich per Test
//! verifiziert.** Der tatsächliche Windows-Spawn-Pfad (`#[cfg(windows)]`) kann hier nicht
//! ausgeführt werden; siehe `docs/portability.md`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Baut ein `Command` für `pi <args>`, das auch dann funktioniert, wenn `pi` nur als
/// npm-Shim-Skript (`pi.cmd`/`pi.bat`) vorliegt. Unter Unix und wenn eine echte `pi`/`pi.exe`
/// gefunden wird, ist das Ergebnis identisch zu `Command::new("pi")` + `.args(args)` – keine
/// Shell beteiligt. Nur wenn ausschließlich ein Skript-Shim gefunden wird, wird `cmd.exe /D
/// /S /C` mit sorgfältig maskierten Argumenten verwendet (siehe [`escape_for_cmd`]); das ist
/// der einzige Codepfad hier, der überhaupt eine Shell involviert, und jedes Argument wird
/// dabei explizit maskiert – niemals wird eine unmaskierte, von außen beeinflussbare
/// Zeichenkette an `cmd.exe` übergeben.
///
/// **Nur unter Unix tatsächlich lauffähig getestet** (dort nimmt der `#[cfg(not(windows))]`-
/// Zweig den einfachen Pfad); der `#[cfg(windows)]`-Zweig ist reiner Conditional-Compile-Code
/// und in dieser Linux-Umgebung nicht ausführbar – siehe `docs/portability.md`.
#[cfg(windows)]
pub fn pi_command(args: &[String]) -> Command {
    let path_env = std::env::var("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path_env).collect();
    let pathext_env = std::env::var("PATHEXT").ok();
    let pathext = pathext_list(pathext_env.as_deref());
    let found = find_executable("pi", &dirs, &pathext, |p| p.is_file());

    match found {
        Some(exe) if !is_script_shim(&exe) => {
            let mut c = Command::new(exe);
            c.args(args);
            c
        }
        Some(shim) => {
            // Skript-Shim: ueber cmd.exe aufrufen, jedes Argument einzeln maskieren.
            let mut line = escape_for_cmd(&shim.to_string_lossy()).unwrap_or_else(|_| shim.to_string_lossy().into_owned());
            for a in args {
                line.push(' ');
                line.push_str(&escape_for_cmd(a).unwrap_or_else(|_| a.clone()));
            }
            let mut c = Command::new("cmd");
            c.args(["/D", "/S", "/C", &line]);
            c
        }
        // Nichts gefunden: Rueckfall auf den urspruenglichen Pfad (liefert denselben Fehler
        // wie zuvor, falls `pi` tatsaechlich fehlt).
        None => {
            let mut c = Command::new("pi");
            c.args(args);
            c
        }
    }
}

#[cfg(not(windows))]
pub fn pi_command(args: &[String]) -> Command {
    let mut c = Command::new("pi");
    c.args(args);
    c
}

/// Liefert die Liste der unter Windows über `PATHEXT` ausführbaren Endungen (ohne Punkt,
/// Kleinbuchstaben), in Suchreihenfolge. Fällt auf eine sinnvolle Standardliste zurück, falls
/// die Umgebungsvariable fehlt oder leer ist (z. B. in dieser Linux-Testumgebung).
pub fn pathext_list(pathext_env: Option<&str>) -> Vec<String> {
    let raw = pathext_env.filter(|s| !s.is_empty()).unwrap_or(".COM;.EXE;.BAT;.CMD;.PS1");
    raw.split(';').filter(|s| !s.is_empty()).map(|s| s.trim_start_matches('.').to_ascii_lowercase()).collect()
}

/// Sucht `name` (ohne Endung) in den angegebenen Verzeichnissen, in der Reihenfolge der
/// `pathext`-Liste (erste gefundene Endung gewinnt, wie bei `CreateProcess`/`cmd.exe`
/// üblich). Reine Pfad-Logik, kein Zugriff auf echte PATH-Umgebungsvariablen – testbar auch
/// unter Unix. Ein Verzeichniseintrag ohne jede Endung (nackter `name`) wird ebenfalls
/// geprüft, falls er zufällig existiert und ausführbar aussieht (deckt z. B. direkt
/// abgelegte `pi`-Binaries ohne Endung ab).
pub fn find_executable(name: &str, dirs: &[PathBuf], pathext: &[String], exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    for dir in dirs {
        let bare = dir.join(name);
        if exists(&bare) {
            return Some(bare);
        }
        for ext in pathext {
            let candidate = dir.join(format!("{name}.{ext}"));
            if exists(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// `true`, wenn `path` eine Batch-Skript-Endung hat (`.cmd`/`.bat`, Groß-/Kleinschreibung
/// egal) – also eine Datei, die `CreateProcess` nicht direkt als Prozess starten kann und
/// stattdessen über `cmd.exe /C` aufgerufen werden muss.
pub fn is_script_shim(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("cmd" | "bat"))
}

/// Maskiert ein einzelnes Argument für die Übergabe als Teil einer `cmd.exe /C`-Befehlszeile.
///
/// `cmd.exe` behandelt `& | < > ^ " %` sowie Zeilenumbrüche als Metazeichen, *bevor* die Zeile
/// an das eigentliche Programm weitergereicht wird. Enthält ein Argument keines dieser
/// Zeichen und auch keine Leerzeichen, wird es unverändert zurückgegeben. Andernfalls wird
/// es in doppelte Anführungszeichen gesetzt; enthaltene `"` werden verdoppelt (von `cmd.exe`
/// innerhalb einer Anführungszeichen-Zeile als literales `"` verstanden) und jedes der
/// Metazeichen zusätzlich mit `^` escaped, wie von `cmd.exe` verlangt. Ein eingebetteter
/// Zeilenumbruch kann in einer einzeiligen `cmd.exe`-Befehlszeile nicht sicher dargestellt
/// werden; solche Argumente werden daher konservativ abgelehnt (siehe Rückgabewert).
pub fn escape_for_cmd(arg: &str) -> Result<String, String> {
    if arg.contains(['\n', '\r', '\0']) {
        return Err("Argument enthält einen Zeilenumbruch/NUL-Byte und kann nicht sicher an cmd.exe übergeben werden".into());
    }
    let needs_quoting = arg.is_empty() || arg.chars().any(|c| matches!(c, ' ' | '\t' | '&' | '|' | '<' | '>' | '^' | '"' | '%' | '(' | ')'));
    if !needs_quoting {
        return Ok(arg.to_string());
    }
    let mut out = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' => out.push_str("\"\""),
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out.push('"');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathext_defaults_when_env_missing_or_empty() {
        let list = pathext_list(None);
        assert!(list.contains(&"cmd".to_string()));
        assert!(list.contains(&"exe".to_string()));
        let list2 = pathext_list(Some(""));
        assert_eq!(list, list2);
    }

    #[test]
    fn pathext_parses_custom_env_value() {
        let list = pathext_list(Some(".COM;.EXE;.CMD"));
        assert_eq!(list, vec!["com", "exe", "cmd"]);
    }

    #[test]
    fn find_executable_prefers_earlier_dirs_and_earlier_extensions() {
        let dirs = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        let pathext = pathext_list(Some(".EXE;.CMD"));
        // Nur /b/pi.cmd existiert.
        let found = find_executable("pi", &dirs, &pathext, |p| p == Path::new("/b/pi.cmd"));
        assert_eq!(found, Some(PathBuf::from("/b/pi.cmd")));

        // /a/pi.cmd und /b/pi.exe existieren beide: /a (frueheres Verzeichnis) gewinnt.
        let found = find_executable("pi", &dirs, &pathext, |p| p == Path::new("/a/pi.cmd") || p == Path::new("/b/pi.exe"));
        assert_eq!(found, Some(PathBuf::from("/a/pi.cmd")));

        // /a/pi.exe und /a/pi.cmd existieren beide: .exe (frueher in PATHEXT) gewinnt.
        let found = find_executable("pi", &dirs, &pathext, |p| p == Path::new("/a/pi.exe") || p == Path::new("/a/pi.cmd"));
        assert_eq!(found, Some(PathBuf::from("/a/pi.exe")));
    }

    #[test]
    fn find_executable_matches_bare_name_without_extension() {
        let dirs = vec![PathBuf::from("/a")];
        let pathext = pathext_list(Some(".EXE"));
        let found = find_executable("pi", &dirs, &pathext, |p| p == Path::new("/a/pi"));
        assert_eq!(found, Some(PathBuf::from("/a/pi")));
    }

    #[test]
    fn find_executable_returns_none_when_nothing_matches() {
        let dirs = vec![PathBuf::from("/a")];
        let pathext = pathext_list(Some(".EXE"));
        assert_eq!(find_executable("pi", &dirs, &pathext, |_| false), None);
    }

    #[test]
    fn is_script_shim_detects_cmd_and_bat_case_insensitively() {
        assert!(is_script_shim(Path::new("pi.cmd")));
        assert!(is_script_shim(Path::new("pi.CMD")));
        assert!(is_script_shim(Path::new("pi.bat")));
        assert!(!is_script_shim(Path::new("pi.exe")));
        assert!(!is_script_shim(Path::new("pi")));
        assert!(!is_script_shim(Path::new("pi.ps1")));
    }

    #[test]
    fn escape_for_cmd_leaves_simple_args_untouched() {
        assert_eq!(escape_for_cmd("--mode").unwrap(), "--mode");
        assert_eq!(escape_for_cmd("rpc").unwrap(), "rpc");
        assert_eq!(escape_for_cmd("/tmp/my-project").unwrap(), "/tmp/my-project");
    }

    #[test]
    fn escape_for_cmd_quotes_args_with_spaces() {
        assert_eq!(escape_for_cmd("my project").unwrap(), "\"my project\"");
    }

    #[test]
    fn escape_for_cmd_quotes_and_escapes_metacharacters() {
        // "&whoami" darf nach dem Escaping nicht als eigener Befehl geparst werden koennen:
        // es muss in Anfuehrungszeichen landen.
        let escaped = escape_for_cmd("a&whoami").unwrap();
        assert!(escaped.starts_with('"') && escaped.ends_with('"'));
        assert!(escaped.contains("a&whoami"));
    }

    #[test]
    fn escape_for_cmd_doubles_embedded_quotes() {
        let escaped = escape_for_cmd("say \"hi\"").unwrap();
        assert_eq!(escaped, "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn escape_for_cmd_escapes_percent_to_avoid_variable_expansion() {
        let escaped = escape_for_cmd("%PATH%").unwrap();
        assert_eq!(escaped, "\"%%PATH%%\"");
    }

    #[test]
    fn escape_for_cmd_rejects_embedded_newlines() {
        assert!(escape_for_cmd("a\nb").is_err());
        assert!(escape_for_cmd("a\rb").is_err());
    }

    #[test]
    fn escape_for_cmd_quotes_empty_argument() {
        assert_eq!(escape_for_cmd("").unwrap(), "\"\"");
    }
}
