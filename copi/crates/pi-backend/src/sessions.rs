//! Sessiondateien von pi auflisten und löschen.
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct SessionInfo {
    pub path: String,
    pub cwd: String,
    pub title: String,
    /// Von Nutzer oder Extension vergebener Name (letzter `session_info`-Eintrag).
    pub name: String,
    /// Änderungszeit in Sekunden seit Epoch.
    pub modified: u64,
    /// Leer = lokale Session; sonst Kennung des Remote-Backends (`backends.json`-id).
    pub backend: String,
}

impl SessionInfo {
    /// Schlüssel für Bookmarks/Farben: lokal der Pfad, remote `remote:<backend>:<pfad>`.
    /// Lokale Pfade sind absolut (beginnen mit `/` bzw. Laufwerk), kollidieren also nie.
    pub fn key(&self) -> String {
        remote_key(&self.backend, &self.path)
    }
    pub fn is_remote(&self) -> bool {
        !self.backend.is_empty()
    }
}

/// Siehe [`SessionInfo::key`]; identisch zu `remoteSessionKey` in `ui/remote-sessions.js`.
pub fn remote_key(backend: &str, path: &str) -> String {
    if backend.is_empty() {
        path.to_string()
    } else {
        format!("remote:{backend}:{path}")
    }
}

pub fn sessions_root() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("PI_CODING_AGENT_SESSION_DIR") {
        return Some(d.into());
    }
    crate::agent_dir().map(|a| a.join("sessions"))
}

/// Verarbeitet eine vollständige JSONL-Zeile: Kopf (`cwd`), erste Nutzer-Nachricht
/// (Titel, nur einmal gesetzt) und `session_info`-Namen (zuletzt gewinnt, läuft
/// also bei jedem weiteren Aufruf mit neueren Zeilen weiter).
fn apply_line(info: &mut SessionInfo, header_done: &mut bool, title_done: &mut bool, line: &str) {
    if !*header_done {
        *header_done = true;
        if let Ok(head) = serde_json::from_str::<serde_json::Value>(line) {
            info.cwd = head["cwd"].as_str().unwrap_or("").to_string();
        }
        // Kopfzeile ist nie zugleich eine Nutzer- oder session_info-Zeile.
        return;
    }
    if line.contains("\"type\":\"session_info\"") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(n) = v["name"].as_str() {
                info.name = n.trim().to_string();
            }
        }
    } else if !*title_done && line.contains("\"role\":\"user\"") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            let c = &v["message"]["content"];
            let t = c.as_str().map(String::from).or_else(|| c.as_array()?.iter().find_map(|p| p["text"].as_str().map(String::from)));
            if let Some(t) = t {
                info.title = t.chars().take(90).collect();
                *title_done = true;
            }
        }
    }
}

/// Liest Kopfzeile, erste Nutzer-Nachricht und `session_info`-Zeilen (voller Durchlauf,
/// für Einzelabfragen ohne Cache; der Cache nutzt stattdessen [`CacheEntry::absorb`]).
pub fn read_info(path: &Path, modified: u64) -> Option<SessionInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut info = SessionInfo { path: path.to_string_lossy().into(), modified, ..Default::default() };
    let (mut header_done, mut title_done) = (false, false);
    // Nur vollständige Zeilen werten (letzte Zeile ohne `\n` kann noch unvollständig sein).
    let complete = complete_prefix(text.as_bytes());
    let text = std::str::from_utf8(complete).ok()?;
    let mut any = false;
    for line in text.split('\n') {
        if line.is_empty() {
            continue;
        }
        any = true;
        apply_line(&mut info, &mut header_done, &mut title_done, line);
    }
    if !any {
        return None;
    }
    Some(info)
}

/// Schneidet `buf` an der letzten `\n` ab; der Rest (unvollständige letzte Zeile) bleibt
/// unberücksichtigt, bis mehr Daten angehängt wurden. Append-only-Dateien können jederzeit
/// mitten in einer Zeile beobachtet werden – wir dürfen nie über das letzte `\n` hinaus lesen.
fn complete_prefix(buf: &[u8]) -> &[u8] {
    match buf.iter().rposition(|&b| b == b'\n') {
        Some(idx) => &buf[..=idx],
        None => &[],
    }
}

/// Anzahl Bytes am Dateianfang, deren Hash als Identitäts-Fingerabdruck dient (siehe
/// [`prefix_fingerprint`]). Deckt JSONL-Kopfzeile (`cwd`, Session-ID, Timestamp) ab, die bei
/// einer Ersetzung durch eine andere Session so gut wie nie zufällig identisch ist.
const PREFIX_FINGERPRINT_LEN: u64 = 4096;

/// Stabiler, nicht kryptografischer Fingerabdruck der ersten `PREFIX_FINGERPRINT_LEN` Bytes
/// (oder weniger, falls die Datei kürzer ist) einer Datei. Dient als plattformunabhängiger
/// Ersatz für eine Inode-Identität, wenn diese nicht verfügbar ist (z. B. Windows): ändert
/// sich der Dateianfang gegenüber dem zuvor gesehenen Stand, ohne dass die Datei nur gekürzt
/// wurde (`len < consumed`), ist das ein starkes Indiz für eine Ersetzung durch eine andere
/// Datei unter demselben Pfad – selbst wenn die neue Datei größer ist als die alte und ihr
/// Wachstum daher nicht trivial als Schrumpfen erkannt werden kann.
fn prefix_fingerprint(path: &Path, len: u64) -> Option<u64> {
    let take = len.min(PREFIX_FINGERPRINT_LEN);
    let file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::with_capacity(take as usize);
    file.take(take).read_to_end(&mut buf).ok()?;
    let mut h = 14695981039346656037u64;
    for b in &buf {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    Some(h)
}

/// Starke, plattformspezifische Dateiidentität (Gerät + Inode) unter Unix: zwei Pfade
/// verweisen genau dann auf dieselbe physische Datei, wenn beide Werte übereinstimmen –
/// unabhängig von Größe/mtime. **Nur unter Unix tatsächlich verifiziert** (siehe
/// `docs/portability.md`); unter Windows liefert diese Funktion stets `None` und die
/// Ersetzungserkennung stützt sich dort allein auf [`prefix_fingerprint`] plus
/// Größe/mtime-Heuristik (siehe [`InfoCache::list`]).
#[cfg(unix)]
fn file_identity(meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((meta.dev(), meta.ino()))
}
#[cfg(not(unix))]
fn file_identity(_meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    None
}

/// Ein Cache-Eintrag merkt sich, wie viele Bytes bereits in vollständigen Zeilen verarbeitet
/// wurden (`consumed`), sowie die zuletzt beobachtete Dateiidentität (`file_id`, unter Unix
/// Gerät+Inode) und den Fingerabdruck des Dateianfangs (`prefix_fp`). Eine am Ende
/// abgeschnittene unvollständige Zeile bleibt bewusst *ungelesen* auf der Platte stehen (kein
/// In-Memory-Duplikat) – beim nächsten Aufruf wird ab `consumed` erneut ab Disk gelesen,
/// wodurch sie automatisch zusammen mit neu angehängten Bytes erneut betrachtet wird.
struct CacheEntry {
    info: SessionInfo,
    stamp: std::time::SystemTime,
    /// Bytes, die bereits vollständig (bis zum letzten `\n`) verarbeitet wurden.
    consumed: u64,
    /// Unix Gerät+Inode bei der letzten Beobachtung (`None`: nicht Unix oder noch nie
    /// beobachtet).
    file_id: Option<(u64, u64)>,
    /// Fingerabdruck des Dateianfangs bei der letzten Beobachtung.
    prefix_fp: Option<u64>,
    header_done: bool,
    title_done: bool,
}

impl CacheEntry {
    fn fresh() -> Self {
        Self {
            info: SessionInfo::default(),
            stamp: std::time::SystemTime::UNIX_EPOCH,
            consumed: 0,
            file_id: None,
            prefix_fp: None,
            header_done: false,
            title_done: false,
        }
    }

    /// Liest ab `consumed` bis `len` (das schließt eine zuvor unvollständige letzte Zeile
    /// erneut ein, falls sie seither nicht weitergewachsen ist) und wertet alle nun
    /// vollständigen Zeilen aus. Eine am Ende weiterhin unvollständige Zeile bleibt auf der
    /// Platte stehen; `consumed` wird nur bis zum letzten `\n` vorgerückt.
    fn absorb(&mut self, path: &Path, len: u64) -> std::io::Result<()> {
        let mut file = std::fs::File::open(path)?;
        file.seek(SeekFrom::Start(self.consumed))?;
        let mut buf = Vec::with_capacity((len - self.consumed) as usize);
        file.take(len - self.consumed).read_to_end(&mut buf)?;
        let complete_len = match buf.iter().rposition(|&b| b == b'\n') {
            Some(idx) => idx + 1,
            // Keine neue vollständige Zeile seit dem letzten Mal; nichts zu tun.
            None => return Ok(()),
        };
        let complete = &buf[..complete_len];
        if let Ok(text) = std::str::from_utf8(complete) {
            for line in text.split('\n') {
                if !line.is_empty() {
                    apply_line(&mut self.info, &mut self.header_done, &mut self.title_done, line);
                }
            }
        }
        self.consumed += complete_len as u64;
        Ok(())
    }
}

/// Cache mit robuster Ersetzungserkennung unter demselben Pfad: Statt sich allein auf
/// Größe/mtime zu verlassen (eine *größere* Ersatzdatei mit neuerer mtime würde von einer
/// reinen Größen-/mtime-Heuristik fälschlich als "gewachsene Originaldatei" behandelt und ab
/// `consumed` "fortgesetzt", was Inhalte aus zwei unterschiedlichen Dateien vermischen würde),
/// vergleicht [`InfoCache::list`] zusätzlich (a) unter Unix Gerät+Inode (zuverlässige, vom
/// Betriebssystem garantierte Dateiidentität) und (b) überall einen Fingerabdruck der ersten
/// Dateibytes (siehe [`prefix_fingerprint`]). Ändert sich eine der beiden Identitäten,
/// ohne dass es sich um ein reines Kürzen handelt, wird der Eintrag vollständig verworfen und
/// neu aufgebaut statt fälschlich fortgesetzt – unabhängig davon, ob die Ersatzdatei größer,
/// kleiner oder gleich groß ist.
#[derive(Default)]
pub struct InfoCache(HashMap<String, CacheEntry>);

impl InfoCache {
    /// Neueste 200 Sessions, absteigend nach Änderungszeit.
    pub fn list(&mut self) -> Vec<SessionInfo> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let Some(root) = sessions_root() else { return out };
        let Ok(dirs) = std::fs::read_dir(root) else { return out };
        for d in dirs.flatten() {
            let Ok(files) = std::fs::read_dir(d.path()) else { continue };
            for f in files.flatten() {
                let p = f.path();
                if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Ok(meta) = p.metadata() else { continue };
                let Ok(stamp) = meta.modified() else { continue };
                let Ok(age) = stamp.duration_since(std::time::UNIX_EPOCH) else { continue };
                let modified = age.as_secs();
                let len = meta.len();
                let key = p.to_string_lossy().to_string();
                seen.insert(key.clone());

                let current_id = file_identity(&meta);
                let current_fp = prefix_fingerprint(&p, len);

                let entry = self.0.entry(key.clone()).or_insert_with(CacheEntry::fresh);
                // Robuste Ersetzungserkennung unter demselben Pfad (siehe Doku an
                // `InfoCache`): keine der folgenden Heuristiken ist allein verlässlich, aber
                // zusammen decken sie Schrumpfen, Ersetzen bei gleicher Größe *und* Ersetzen
                // durch eine größere Datei ab. Nie über das Ende einer unbekannten neuen
                // Datei hinaus "fortsetzen" – im Zweifel kompletter Neuaufbau.
                let shrunk = len < entry.consumed;
                // Nur aussagekräftig, wenn wir zuvor überhaupt eine Identität gesehen haben
                // (Unix) bzw. etwas eingelesen hatten (Fingerabdruck) – sonst ist es die
                // erste Beobachtung dieser Datei, kein Widerspruch.
                let identity_changed = matches!((entry.file_id, current_id), (Some(old), Some(new)) if old != new);
                let prefix_changed =
                    entry.consumed > 0 && matches!((entry.prefix_fp, current_fp), (Some(old), Some(new)) if old != new);
                let same_size_but_stamp_changed = entry.stamp != stamp && len == entry.consumed;
                let replaced = shrunk || identity_changed || prefix_changed || same_size_but_stamp_changed;
                if replaced {
                    *entry = CacheEntry::fresh();
                }
                if len > entry.consumed {
                    let _ = entry.absorb(&p, len);
                }
                entry.stamp = stamp;
                entry.file_id = current_id;
                entry.prefix_fp = current_fp;
                entry.info.path = key.clone();
                entry.info.modified = modified;
                if entry.header_done {
                    out.push(entry.info.clone());
                }
            }
        }
        self.0.retain(|key, _| seen.contains(key));
        out.sort_by_key(|a| std::cmp::Reverse(a.modified));
        out.truncate(200);
        out
    }

    /// Anzahl zwischengespeicherter Dateien (für Tests/Diagnose der Eviction).
    pub fn cached_len(&self) -> usize {
        self.0.len()
    }

    /// Löscht nur `*.jsonl` unterhalb des Sessions-Ordners.
    pub fn delete(&mut self, path: &str) -> Result<(), String> {
        let root = sessions_root().ok_or("kein Sessions-Ordner")?.canonicalize().map_err(|e| e.to_string())?;
        let p = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
        if !p.starts_with(&root) || p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            return Err("Pfad liegt nicht im Sessions-Ordner".into());
        }
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
        self.0.remove(path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_key_separates_local_and_remote() {
        let mut s = SessionInfo { path: "/h/.pi/agent/sessions/a.jsonl".into(), ..Default::default() };
        assert_eq!(s.key(), s.path);
        assert!(!s.is_remote());
        s.backend = "hub-buero".into();
        assert_eq!(s.key(), "remote:hub-buero:/h/.pi/agent/sessions/a.jsonl");
        assert!(s.is_remote());
    }

    fn sessions_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("copi-sess-{tag}-{}-{}", std::process::id(), fastrand_seed()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fastrand_seed() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64
    }

    /// Setzt das `root`-Verzeichnis für die Dauer des Tests als Sessions-Root. Serialisiert
    /// über den crate-weiten [`crate::test_support::EnvGuard`] (nicht nur ein modul-lokaler
    /// Lock), weil Umgebungsvariablen prozessweit gelten und sich sonst mit env-mutierenden
    /// Tests anderer Module (`media::`, `lib::`, `preview::`, `rpc::`) ueberlappen könnten;
    /// stellt den alten Wert auch bei einem Panic im Testkörper zuverlässig wieder her.
    fn with_root<T>(root: &Path, f: impl FnOnce() -> T) -> T {
        use crate::test_support::EnvGuard;
        let _guard = EnvGuard::new(&[("PI_CODING_AGENT_SESSION_DIR", Some(&root.to_string_lossy()))]);
        f()
    }

    #[test]
    fn reads_title_and_last_name() {
        let dir = sessions_dir("basic");
        let f = dir.join("a.jsonl");
        std::fs::write(&f, concat!(
            "{\"type\":\"session\",\"cwd\":\"/tmp/x\"}\n",
            "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"Hallo Welt\"}]}}\n",
            "{\"type\":\"session_info\",\"name\":\"Erst\"}\n",
            "{\"type\":\"session_info\",\"name\":\" Zweit \"}\n",
        )).unwrap();
        let i = read_info(&f, 5).unwrap();
        assert_eq!((i.cwd.as_str(), i.title.as_str(), i.name.as_str(), i.modified), ("/tmp/x", "Hallo Welt", "Zweit", 5));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn read_info_ignores_incomplete_trailing_line() {
        let dir = sessions_dir("partial");
        let f = dir.join("a.jsonl");
        // Letzte Zeile hat kein `\n` -> gilt als noch nicht vollständig geschrieben.
        std::fs::write(&f, concat!(
            "{\"type\":\"session\",\"cwd\":\"/tmp/x\"}\n",
            "{\"type\":\"session_info\",\"name\":\"halbfertig",
        )).unwrap();
        let i = read_info(&f, 1).unwrap();
        assert_eq!(i.name, "");
        assert_eq!(i.cwd, "/tmp/x");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn incremental_cache_picks_up_appended_lines_without_full_reread() {
        let dir = sessions_dir("incr");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let f = sub.join("a.jsonl");
        std::fs::write(&f, "{\"type\":\"session\",\"cwd\":\"/tmp/a\"}\n").unwrap();
        let mut cache = InfoCache::default();
        let list = with_root(&dir, || cache.list());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "");
        assert_eq!(cache.cached_len(), 1);

        // Zeile anhängen, danach mitten in der nächsten Zeile "stehen bleiben" (kein \n):
        // das Cache darf daraus noch keinen Titel ableiten.
        let mut file = std::fs::OpenOptions::new().append(true).open(&f).unwrap();
        use std::io::Write;
        writeln!(file, "{{\"type\":\"message\",\"message\":{{\"role\":\"user\",\"content\":[{{\"type\":\"text\",\"text\":\"Hallo Welt\"}}]}}}}").unwrap();
        write!(file, "{{\"type\":\"session_info\",\"name\":\"unvollst").unwrap();
        drop(file);
        let list = with_root(&dir, || cache.list());
        assert_eq!(list[0].title, "Hallo Welt");
        assert_eq!(list[0].name, "");

        // Zeile vervollständigen -> jetzt erst wird der Name sichtbar.
        let mut file = std::fs::OpenOptions::new().append(true).open(&f).unwrap();
        writeln!(file, "aendig\"}}").unwrap();
        drop(file);
        let list = with_root(&dir, || cache.list());
        assert_eq!(list[0].name, "unvollstaendig");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn truncated_or_replaced_file_is_rebuilt_not_corrupted() {
        let dir = sessions_dir("replace");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let f = sub.join("a.jsonl");
        std::fs::write(&f, concat!(
            "{\"type\":\"session\",\"cwd\":\"/tmp/old\"}\n",
            "{\"type\":\"session_info\",\"name\":\"Alt\"}\n",
        )).unwrap();
        let mut cache = InfoCache::default();
        let list = with_root(&dir, || cache.list());
        assert_eq!(list[0].name, "Alt");
        assert_eq!(list[0].cwd, "/tmp/old");

        // Gleiche Byteanzahl, aber anderer Inhalt und garantiert andere mtime: muss als
        // Ersetzung erkannt werden, nicht naiv an altem Offset fortgesetzt werden.
        std::thread::sleep(std::time::Duration::from_millis(10));
        let replacement = concat!(
            "{\"type\":\"session\",\"cwd\":\"/tmp/new\"}\n",
            "{\"type\":\"session_info\",\"name\":\"Neu\"}\n",
        );
        assert_eq!(replacement.len(), concat!(
            "{\"type\":\"session\",\"cwd\":\"/tmp/old\"}\n",
            "{\"type\":\"session_info\",\"name\":\"Alt\"}\n",
        ).len());
        std::fs::write(&f, replacement).unwrap();
        let list = with_root(&dir, || cache.list());
        assert_eq!(list[0].name, "Neu");
        assert_eq!(list[0].cwd, "/tmp/new");

        // Kürzen (z. B. Kompaktierung) muss ebenfalls zum Neuaufbau führen, nicht zum Absturz.
        std::fs::write(&f, "{\"type\":\"session\",\"cwd\":\"/tmp/short\"}\n").unwrap();
        let list = with_root(&dir, || cache.list());
        assert_eq!(list[0].cwd, "/tmp/short");
        assert_eq!(list[0].name, "");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn larger_replacement_file_at_same_path_is_rebuilt_not_merged() {
        // Regressionstest für den Fehler, den eine reine Größen-/mtime-Heuristik nicht
        // erkennen kann: Eine Ersatzdatei, die *größer* ist als die alte Datei (und damit
        // wie "gewachsen" aussieht), muss trotzdem als Ersetzung erkannt werden – über
        // Dateiidentität (Unix: Gerät+Inode) und/oder Fingerabdruck des Dateianfangs –, statt
        // fälschlich ab dem alten `consumed`-Offset "fortgesetzt" zu werden, was Inhalte aus
        // zwei verschiedenen Dateien vermischen würde.
        let dir = sessions_dir("replace-larger");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let f = sub.join("a.jsonl");
        std::fs::write(&f, concat!(
            "{\"type\":\"session\",\"cwd\":\"/tmp/old\"}\n",
            "{\"type\":\"session_info\",\"name\":\"Alt\"}\n",
        )).unwrap();
        let mut cache = InfoCache::default();
        let list = with_root(&dir, || cache.list());
        assert_eq!(list[0].name, "Alt");
        assert_eq!(list[0].cwd, "/tmp/old");

        // Datei unter demselben Pfad durch eine komplett neue, deutlich größere Datei
        // ersetzen (z. B. `rename()` einer frisch gestarteten Session über die alte Datei).
        std::thread::sleep(std::time::Duration::from_millis(10));
        let big_filler = "x".repeat(4096);
        let replacement = format!(
            "{{\"type\":\"session\",\"cwd\":\"/tmp/new\",\"pad\":\"{big_filler}\"}}\n{{\"type\":\"session_info\",\"name\":\"Neu\"}}\n"
        );
        assert!(replacement.len() > std::fs::metadata(&f).unwrap().len() as usize);
        std::fs::write(&f, &replacement).unwrap();

        let list = with_root(&dir, || cache.list());
        assert_eq!(list[0].name, "Neu");
        assert_eq!(list[0].cwd, "/tmp/new");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn prefix_fingerprint_changes_when_content_changes_and_stable_otherwise() {
        let dir = sessions_dir("fingerprint");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.jsonl");
        std::fs::write(&f, "hello world\n").unwrap();
        let len = std::fs::metadata(&f).unwrap().len();
        let fp1 = prefix_fingerprint(&f, len).unwrap();
        let fp1_again = prefix_fingerprint(&f, len).unwrap();
        assert_eq!(fp1, fp1_again);

        std::fs::write(&f, "completely different content\n").unwrap();
        let len2 = std::fs::metadata(&f).unwrap().len();
        let fp2 = prefix_fingerprint(&f, len2).unwrap();
        assert_ne!(fp1, fp2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn file_identity_differs_across_distinct_files_and_matches_same_file() {
        let dir = sessions_dir("identity");
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.jsonl");
        let b = dir.join("b.jsonl");
        std::fs::write(&a, "a\n").unwrap();
        std::fs::write(&b, "b\n").unwrap();
        let id_a1 = file_identity(&a.metadata().unwrap());
        let id_a2 = file_identity(&a.metadata().unwrap());
        let id_b = file_identity(&b.metadata().unwrap());
        assert!(id_a1.is_some());
        assert_eq!(id_a1, id_a2);
        assert_ne!(id_a1, id_b);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cache_evicts_entries_for_deleted_files() {
        let dir = sessions_dir("evict");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let f = sub.join("a.jsonl");
        std::fs::write(&f, "{\"type\":\"session\",\"cwd\":\"/tmp/a\"}\n").unwrap();
        let mut cache = InfoCache::default();
        with_root(&dir, || cache.list());
        assert_eq!(cache.cached_len(), 1);
        std::fs::remove_file(&f).unwrap();
        let list = with_root(&dir, || cache.list());
        assert!(list.is_empty());
        assert_eq!(cache.cached_len(), 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn delete_removes_file_and_cache_entry() {
        let dir = sessions_dir("delete");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let f = sub.join("a.jsonl");
        std::fs::write(&f, "{\"type\":\"session\",\"cwd\":\"/tmp/a\"}\n").unwrap();
        let mut cache = InfoCache::default();
        with_root(&dir, || cache.list());
        assert_eq!(cache.cached_len(), 1);
        let path = f.to_string_lossy().to_string();
        with_root(&dir, || cache.delete(&path)).unwrap();
        assert!(!f.exists());
        assert_eq!(cache.cached_len(), 0);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
