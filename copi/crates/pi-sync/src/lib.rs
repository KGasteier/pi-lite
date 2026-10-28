//! Bewusster Dateifluss ohne HTTP, Auto-Ausführung oder Überschreiben.
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

const RULES: &str = include_str!("../../../../shared/sync/klassen.json");
const MAX_ENTRIES: usize = 100_000;
const MAX_BYTES: u64 = 2_147_483_648;
const MAX_FILE: u64 = 209_715_200;
const AREAS: [&str; 4] = ["Projekte", "Ergebnisse", "Skills", "Verlaeufe"];
fn invalid(text: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, text)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Klasse {
    Persoenlich,
    Fluechtig,
    Gross,
    Nie,
}
#[derive(Deserialize)]
struct Rules {
    fluechtige_ordner: Vec<String>,
    fluechtige_endungen: Vec<String>,
    fluechtige_namen: Vec<String>,
    fluechtige_praefixe: Vec<String>,
}
/// Nur portable, relative Archivpfade. Windows-Pfade auch auf Unix ablehnen.
pub fn safe_path(p: &str) -> io::Result<()> {
    if p.is_empty()
        || p.len() > 255
        || p.chars().any(|c| c.is_control() || c == '\\' || c == ':')
        || p.split('/').any(|x| {
            let name = x.split('.').next().unwrap_or("").to_ascii_lowercase();
            x.is_empty()
                || x == "."
                || x == ".."
                || x.ends_with(['.', ' '])
                || ["con", "prn", "aux", "nul"].contains(&name.as_str())
                || (name.len() == 4
                    && (name.starts_with("com") || name.starts_with("lpt"))
                    && matches!(name.as_bytes()[3], b'1'..=b'9'))
        })
    {
        return Err(invalid("Dieser Dateipfad ist nicht erlaubt."));
    }
    Ok(())
}
pub fn classify(p: &str, bytes: u64, regular: bool) -> Klasse {
    if !regular || safe_path(p).is_err() {
        return Klasse::Nie;
    }
    let parts: Vec<_> = p.split('/').collect();
    if !AREAS.contains(&parts[0]) && parts[0] != "Verläufe" {
        return Klasse::Fluechtig;
    }
    if parts.len() == 1 {
        return Klasse::Persoenlich;
    }
    if bytes > MAX_FILE {
        return Klasse::Gross;
    }
    if parts[0] == "Ergebnisse" {
        return Klasse::Persoenlich;
    }
    let r: Rules = serde_json::from_str(RULES).expect("Versionierte Sync-Regeln");
    let name = parts.last().unwrap();
    if parts[1..parts.len() - 1]
        .iter()
        .any(|p| r.fluechtige_ordner.iter().any(|x| x == p))
        || r.fluechtige_endungen.iter().any(|x| name.ends_with(x))
        || r.fluechtige_namen.iter().any(|x| x == name)
        || r.fluechtige_praefixe.iter().any(|x| name.starts_with(x))
    {
        Klasse::Fluechtig
    } else {
        Klasse::Persoenlich
    }
}
fn hardlinked(meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.nlink() > 1
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}
fn no_link(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || (!meta.is_dir() && !meta.is_file()) {
        return Err(invalid(
            "Verknüpfungen und Spezialdateien werden nicht übertragen.",
        ));
    }
    Ok(())
}
fn ensure_tree(path: &Path) -> io::Result<()> {
    // Auch bestehende Vorfahren prüfen; ein normaler letzter Ordner kann unter einem Link liegen.
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && parent != path {
            ensure_tree(parent)?;
        }
    }
    if path.exists() {
        no_link(path)?;
        if !path.is_dir() {
            return Err(invalid("Hier wird ein Ordner benötigt."));
        }
        return Ok(());
    }
    fs::create_dir(path)?;
    no_link(path)
}
fn hash_reader(mut reader: impl Read) -> io::Result<String> {
    let mut hash = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = reader.read(&mut b)?;
        if n == 0 {
            break;
        }
        hash.update(&b[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
fn hash_file(path: &Path) -> io::Result<String> {
    no_link(path)?;
    hash_reader(File::open(path)?)
}
fn atomic_json(path: &Path, data: &impl Serialize) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| invalid("Ordner fehlt."))?;
    ensure_tree(parent)?;
    if path.exists() {
        no_link(path)?;
    }
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(&mut tmp, data)?;
    tmp.as_file_mut().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
#[derive(Clone, Debug)]
pub struct LocalFolder {
    pub root: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Zustand {
    pub v: u8,
    pub letzter_checkin: Option<String>,
    pub auswahl: Vec<Auswahl>,
    pub manifest: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Auswahl {
    pub bereich: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionItem {
    pub bereich: String,
    pub name: String,
    pub bytes: u64,
    pub dateien: u64,
    pub fluechtig_bytes: u64,
    pub fluechtig_dateien: u64,
    pub gross_bytes: u64,
    pub gross_dateien: u64,
}
#[derive(Debug, Clone)]
pub struct Conflict {
    pub original: String,
    pub ruecklaeufer: String,
}
#[derive(Debug)]
pub struct ImportResult {
    pub dateien: usize,
    pub konflikte: Vec<Conflict>,
    pub archiv: PathBuf,
}
impl LocalFolder {
    /// Der Aufrufer übergibt den bewusst gewählten Arbeitsordner; kein Zugriff auf Konfigurationsdaten.
    pub fn create(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        ensure_tree(&root)?;
        for area in [
            "Projekte",
            "Ergebnisse",
            "Skills",
            "Verläufe",
            ".sync/archiv",
            ".sync/konflikte",
        ] {
            ensure_tree(&root.join(area))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for p in [".sync", ".sync/archiv", ".sync/konflikte"] {
                fs::set_permissions(root.join(p), fs::Permissions::from_mode(0o700))?;
            }
        }
        let folder = Self { root };
        if !folder.root.join(".sync/zustand.json").exists() {
            folder.save_state(&Zustand {
                v: 1,
                ..Default::default()
            })?;
        }
        Ok(folder)
    }
    fn area_path(&self, area: &str) -> io::Result<PathBuf> {
        if !AREAS.contains(&area) {
            return Err(invalid("Wähle Projekte, Ergebnisse, Skills oder Verläufe."));
        }
        Ok(self.root.join(if area == "Verlaeufe" {
            "Verläufe"
        } else {
            area
        }))
    }
    pub fn load_state(&self) -> io::Result<Zustand> {
        let p = self.root.join(".sync/zustand.json");
        no_link(&p)?;
        Ok(serde_json::from_reader(File::open(p)?)?)
    }
    pub fn save_state(&self, state: &Zustand) -> io::Result<()> {
        if state.v != 1 {
            return Err(invalid("Unbekannter Sync-Stand."));
        }
        atomic_json(&self.root.join(".sync/zustand.json"), state)
    }
    /// Zeigt jedes Projekt bzw. jeden obersten Eintrag; flüchtige Daten sind nie vorausgewählt.
    pub fn selection(&self) -> io::Result<Vec<SelectionItem>> {
        let mut result = Vec::new();
        for area in AREAS {
            let dir = self.area_path(area)?;
            no_link(&dir)?;
            let mut children: Vec<_> = fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
            children.sort_by_key(|x| x.file_name());
            for child in children {
                let name = child
                    .file_name()
                    .into_string()
                    .map_err(|_| invalid("Der Dateiname ist nicht lesbar."))?;
                let mut item = SelectionItem {
                    bereich: area.into(),
                    name: name.clone(),
                    bytes: 0,
                    dateien: 0,
                    fluechtig_bytes: 0,
                    fluechtig_dateien: 0,
                    gross_bytes: 0,
                    gross_dateien: 0,
                };
                for entry in WalkDir::new(child.path()).follow_links(false) {
                    let entry = entry?;
                    if !entry.file_type().is_file() || hardlinked(&entry.metadata()?) {
                        continue;
                    }
                    let p = relative_portable(&dir, entry.path())?;
                    let size = entry.metadata()?.len();
                    match classify(&format!("{area}/{p}"), size, true) {
                        Klasse::Persoenlich => {
                            item.bytes += size;
                            item.dateien += 1;
                        }
                        Klasse::Fluechtig => {
                            item.fluechtig_bytes += size;
                            item.fluechtig_dateien += 1;
                        }
                        Klasse::Gross => {
                            item.gross_bytes += size;
                            item.gross_dateien += 1;
                        }
                        Klasse::Nie => {}
                    }
                }
                if item.dateien + item.fluechtig_dateien + item.gross_dateien > 0 {
                    result.push(item);
                }
            }
        }
        Ok(result)
    }
    /// Streamt ein ausgewähltes Verzeichnis relativ zu Bereich/Name für POST sync/hoch.
    /// Einzeldateien können als Quelle gewählt werden; Transport-Name ist dann ein neuer Zielordner.
    pub fn upload<W: Write>(
        &self,
        selected: &Auswahl,
        output: W,
    ) -> io::Result<BTreeMap<String, String>> {
        safe_path(&selected.name)?;
        if selected.name.contains('/') {
            return Err(invalid("Wähle einen einzelnen Projektordner."));
        }
        let dir = self.area_path(&selected.bereich)?;
        let source = dir.join(&selected.name);
        no_link(&source)?;
        let base = if source.is_dir() {
            source.as_path()
        } else {
            dir.as_path()
        };
        let mut entries = Vec::new();
        for entry in WalkDir::new(&source).follow_links(false) {
            let entry = entry?;
            if !entry.file_type().is_file() || hardlinked(&entry.metadata()?) {
                continue;
            }
            let p = relative_portable(base, entry.path())?;
            let full = format!("{}/{}/{}", selected.bereich, selected.name, p);
            if classify(&full, entry.metadata()?.len(), true) == Klasse::Persoenlich {
                entries.push((p, entry.path().to_owned()));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        write_archive(entries, output)
    }
    /// Archiv erst vollständig sichern und prüfen, dann sicher importieren. Bestätigung macht der Aufrufer.
    pub fn import<R: Read>(
        &self,
        input: R,
        expected_sha256: &str,
        archive_name: &str,
        stamp: &str,
    ) -> io::Result<ImportResult> {
        safe_path(archive_name)?;
        if archive_name.contains('/') || !archive_name.ends_with(".tar.gz") {
            return Err(invalid("Ungültiger Archivname."));
        }
        validate_stamp(stamp)?;
        let archive_dir = self.root.join(".sync/archiv");
        ensure_tree(&archive_dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(&archive_dir)?;
        let n = io::copy(&mut input.take(MAX_BYTES + 1), &mut tmp)?;
        if n > MAX_BYTES {
            return Err(invalid("Das Archiv ist zu groß."));
        }
        tmp.as_file_mut().sync_all()?;
        if hash_file(tmp.path())? != expected_sha256 {
            return Err(invalid(
                "Das Archiv ist beschädigt. Lade es erneut herunter.",
            ));
        }
        let archive = archive_dir.join(archive_name);
        // Kein vorhandenes Archiv ersetzen.
        tmp.persist_noclobber(&archive).map_err(|e| e.error)?;
        let stage = tempfile::tempdir_in(&archive_dir)?;
        let files = read_archive(File::open(&archive)?, stage.path())?;
        // Alle Zielpfade zuerst prüfen, bevor auch nur eine lokale Datei zurückgespielt wird.
        for p in &files {
            let dest = self.local_path(p)?;
            self.check_parents(&dest)?;
            if dest.exists() {
                no_link(&dest)?;
                if !dest.is_file() {
                    return Err(invalid("Am Ziel steht bereits ein Ordner."));
                }
            }
        }
        let mut conflicts = Vec::new();
        let mut count = 0;
        for p in files {
            let source = stage.path().join(&p);
            let dest = self.local_path(&p)?;
            self.check_parents(&dest)?;
            ensure_tree(dest.parent().unwrap())?;
            let mut target = dest.clone();
            if dest.exists() {
                if hash_file(&dest)? == hash_file(&source)? {
                    continue;
                }
                target = conflict_name(&dest, stamp, 1)?;
                let mut index = 2;
                while target.exists() {
                    target = conflict_name(&dest, stamp, index)?;
                    index += 1;
                }
                conflicts.push(Conflict {
                    original: p.clone(),
                    ruecklaeufer: relative_portable(&self.root, &target)?,
                });
            }
            // create_new ist die letzte, atomare Überschreibsperre.
            let mut out = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&target)?;
            io::copy(&mut File::open(source)?, &mut out)?;
            out.sync_all()?;
            count += 1;
        }
        if !conflicts.is_empty() {
            let list: Vec<_> = conflicts
                .iter()
                .map(
                    |c| serde_json::json!({"original": c.original, "ruecklaeufer": c.ruecklaeufer}),
                )
                .collect();
            let id = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            atomic_json(&self.root.join(format!(".sync/konflikte/{id}.json")), &list)?;
        }
        Ok(ImportResult {
            dateien: count,
            konflikte: conflicts,
            archiv: archive,
        })
    }
    fn local_path(&self, p: &str) -> io::Result<PathBuf> {
        safe_path(p)?;
        let (area, rest) = p
            .split_once('/')
            .ok_or_else(|| invalid("Im Archiv fehlt der Bereich."))?;
        Ok(self.area_path(area)?.join(rest))
    }
    fn check_parents(&self, dest: &Path) -> io::Result<()> {
        let rel = dest
            .strip_prefix(&self.root)
            .map_err(|_| invalid("Dieser Zielpfad ist nicht erlaubt."))?;
        let mut at = self.root.clone();
        no_link(&at)?;
        for part in rel.components() {
            at.push(part);
            match fs::symlink_metadata(&at) {
                Ok(_) => no_link(&at)?,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
fn relative_portable(base: &Path, file: &Path) -> io::Result<String> {
    let rel = file
        .strip_prefix(base)
        .map_err(|_| invalid("Dieser Dateipfad ist nicht erlaubt."))?;
    let p = rel
        .components()
        .map(|x| {
            x.as_os_str()
                .to_str()
                .ok_or_else(|| invalid("Der Dateiname ist nicht lesbar."))
        })
        .collect::<io::Result<Vec<_>>>()?
        .join("/");
    safe_path(&p)?;
    Ok(p)
}
fn validate_stamp(s: &str) -> io::Result<()> {
    let b = s.as_bytes();
    if b.len() != 12
        || b[2] != b'.'
        || b[5] != b'.'
        || b[6] != b' '
        || b[9] != b'-'
        || !b
            .iter()
            .enumerate()
            .all(|(i, c)| [2, 5, 6, 9].contains(&i) || c.is_ascii_digit())
    {
        return Err(invalid("Zeitangabe muss TT.MM. HH-MM sein."));
    }
    Ok(())
}
fn conflict_name(dest: &Path, stamp: &str, index: usize) -> io::Result<PathBuf> {
    let stem = dest
        .file_stem()
        .and_then(|x| x.to_str())
        .ok_or_else(|| invalid("Der Dateiname ist nicht lesbar."))?;
    let ext = dest
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| format!(".{x}"))
        .unwrap_or_default();
    let extra = if index > 1 {
        format!(" {index}")
    } else {
        String::new()
    };
    Ok(dest.with_file_name(format!("{stem} (R2 {stamp}){extra}{ext}")))
}
fn octal(field: &[u8]) -> io::Result<u64> {
    let end = field.iter().position(|x| *x == 0).unwrap_or(field.len());
    let text = std::str::from_utf8(&field[..end])
        .map_err(|_| invalid("Ungültiger Archivkopf."))?
        .trim();
    if text.is_empty() || !text.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return Err(invalid("Ungültige Archivgröße."));
    }
    u64::from_str_radix(text, 8).map_err(|_| invalid("Ungültige Archivgröße."))
}
fn text(field: &[u8]) -> io::Result<&str> {
    let end = field.iter().position(|x| *x == 0).unwrap_or(field.len());
    if field[end..].iter().any(|b| *b != 0) {
        return Err(invalid("Ungültiger Archivkopf."));
    }
    std::str::from_utf8(&field[..end]).map_err(|_| invalid("Der Archivpfad ist nicht lesbar."))
}
fn number(h: &mut [u8], at: usize, len: usize, value: u64) -> io::Result<()> {
    let s = format!("{value:0width$o}", width = len - 1);
    if s.len() >= len {
        return Err(invalid("Die Datei ist zu groß."));
    }
    h[at..at + len - 1].copy_from_slice(s.as_bytes());
    h[at + len - 1] = 0;
    Ok(())
}
fn header(p: &str, size: u64) -> io::Result<[u8; 512]> {
    safe_path(p)?;
    let mut h = [0; 512];
    let (prefix, name) = if p.len() <= 100 {
        ("", p)
    } else {
        let cut = p
            .match_indices('/')
            .map(|(i, _)| i)
            .rev()
            .find(|i| *i <= 155 && p.len() - i - 1 <= 100)
            .ok_or_else(|| invalid("Der Archivpfad ist zu lang."))?;
        (&p[..cut], &p[cut + 1..])
    };
    h[..name.len()].copy_from_slice(name.as_bytes());
    h[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
    number(&mut h, 100, 8, 0o644)?;
    number(&mut h, 108, 8, 0)?;
    number(&mut h, 116, 8, 0)?;
    number(&mut h, 124, 12, size)?;
    number(&mut h, 136, 12, 0)?;
    h[148..156].fill(32);
    h[156] = b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let sum = h.iter().map(|x| *x as u64).sum();
    number(&mut h, 148, 7, sum)?;
    h[155] = 32;
    Ok(h)
}
/// Minimaler ustar-Schreiber, gzip als Stream; Dateien werden nie komplett eingelesen.
pub fn write_archive<W: Write>(
    entries: Vec<(String, PathBuf)>,
    output: W,
) -> io::Result<BTreeMap<String, String>> {
    if entries.len() > MAX_ENTRIES {
        return Err(invalid("Zu viele Dateien."));
    }
    let mut zip = GzEncoder::new(output, Compression::default());
    let mut manifest = BTreeMap::new();
    let mut total = 0;
    for (p, file) in entries {
        safe_path(&p)?;
        if let Some(parent) = file.parent() {
            ensure_tree(parent)?;
        }
        no_link(&file)?;
        let mut input = File::open(&file)?;
        let before = input.metadata()?;
        if !before.is_file()
            || hardlinked(&before)
            || before.len() > MAX_FILE
            || total + before.len() > MAX_BYTES
            || manifest.contains_key(&p)
        {
            return Err(invalid(
                "Die Auswahl enthält zu große oder doppelte Dateien.",
            ));
        }
        total += before.len();
        zip.write_all(&header(&p, before.len())?)?;
        let mut hash = Sha256::new();
        let mut left = before.len();
        let mut b = [0; 65536];
        while left > 0 {
            let n = input.read(&mut b[..left.min(65536) as usize])?;
            if n == 0 {
                return Err(invalid("Die Datei hat sich verändert. Versuche es erneut."));
            }
            zip.write_all(&b[..n])?;
            hash.update(&b[..n]);
            left -= n as u64;
        }
        let after = input.metadata()?;
        if before.len() != after.len() || before.modified()? != after.modified()? {
            return Err(invalid("Die Datei hat sich verändert. Versuche es erneut."));
        }
        let padding = (512 - before.len() % 512) % 512;
        zip.write_all(&[0; 512][..padding as usize])?;
        manifest.insert(
            p,
            hash.finalize().iter().map(|b| format!("{b:02x}")).collect(),
        );
    }
    zip.write_all(&[0; 1024])?;
    zip.finish()?;
    Ok(manifest)
}
/// Entpackt ausschließlich in einen neuen privaten Zwischenordner. Keine Links, PAX oder Spezialdateien.
pub fn read_archive<R: Read>(input: R, stage: &Path) -> io::Result<Vec<String>> {
    let mut zip = GzDecoder::new(input);
    let mut seen = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut files = Vec::new();
    let mut total = 0;
    loop {
        let mut h = [0; 512];
        zip.read_exact(&mut h)?;
        if h.iter().all(|b| *b == 0) {
            zip.read_exact(&mut h)?;
            if h.iter().any(|b| *b != 0) {
                return Err(invalid("Das Archivende ist beschädigt."));
            }
            let mut extra = Vec::new();
            zip.take(10241).read_to_end(&mut extra)?;
            if extra.len() > 10240 || extra.iter().any(|b| *b != 0) {
                return Err(invalid("Das Archiv enthält unerwartete Daten."));
            }
            break;
        }
        if seen.len() >= MAX_ENTRIES {
            return Err(invalid("Zu viele Dateien im Archiv."));
        }
        let sum: u64 = h
            .iter()
            .enumerate()
            .map(|(i, b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    *b as u64
                }
            })
            .sum();
        if octal(&h[148..156])? != sum
            || &h[257..263] != b"ustar\0"
            || &h[263..265] != b"00"
            || ![0, b'0', b'5'].contains(&h[156])
            || !text(&h[157..257])?.is_empty()
        {
            return Err(invalid("Dieser Archivtyp ist nicht erlaubt."));
        }
        let name = text(&h[..100])?;
        let prefix = text(&h[345..500])?;
        let name = if h[156] == b'5' {
            name.strip_suffix('/').unwrap_or(name)
        } else {
            name
        };
        let p = if prefix.is_empty() {
            name.into()
        } else {
            format!("{prefix}/{name}")
        };
        safe_path(&p)?;
        if !seen.insert(p.clone()) || !folded.insert(p.to_lowercase()) {
            return Err(invalid("Doppelter Pfad im Archiv."));
        }
        let size = octal(&h[124..136])?;
        if size > MAX_BYTES || total + size > MAX_BYTES || (h[156] == b'5' && size != 0) {
            return Err(invalid("Das Archiv ist zu groß."));
        }
        total += size;
        let target = stage.join(&p);
        ensure_tree(target.parent().unwrap())?;
        if h[156] == b'5' {
            ensure_tree(&target)?;
            continue;
        }
        let mut out = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&target)?;
        let copied = io::copy(&mut zip.by_ref().take(size), &mut out)?;
        if copied != size {
            return Err(invalid("Das Archiv ist unvollständig."));
        }
        out.sync_all()?;
        let mut padding = [0; 512];
        let n = ((512 - size % 512) % 512) as usize;
        zip.read_exact(&mut padding[..n])?;
        if padding[..n].iter().any(|b| *b != 0) {
            return Err(invalid("Das Archiv ist beschädigt."));
        }
        files.push(p);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gemeinsame_klassenfaelle() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../../../shared/sync/faelle.json")).unwrap();
        for c in cases.as_array().unwrap() {
            let regular = c["art"].as_str().unwrap_or("datei") == "datei";
            let actual = serde_json::to_value(classify(
                c["pfad"].as_str().unwrap(),
                c["bytes"].as_u64().unwrap(),
                regular,
            ))
            .unwrap();
            assert_eq!(actual, c["klasse"], "{}", c["pfad"]);
        }
    }
    #[test]
    fn auswahl_stream_import_ohne_ueberschreiben() {
        let tmp = tempfile::tempdir().unwrap();
        let local = LocalFolder::create(tmp.path().join("cocopi")).unwrap();
        let project = local.root.join("Projekte/Buch");
        ensure_tree(&project.join("target")).unwrap();
        fs::write(project.join("text.md"), "Lokal").unwrap();
        fs::write(project.join("target/build.o"), "Build").unwrap();
        let items = local.selection().unwrap();
        assert_eq!(items[0].dateien, 1);
        assert_eq!(items[0].fluechtig_dateien, 1);
        let mut upload = Vec::new();
        let manifest = local
            .upload(
                &Auswahl {
                    bereich: "Projekte".into(),
                    name: "Buch".into(),
                },
                &mut upload,
            )
            .unwrap();
        assert_eq!(manifest.len(), 1);
        assert!(manifest.contains_key("text.md"));
        let remote = tmp.path().join("remote.md");
        fs::write(&remote, "Von R2").unwrap();
        let mut archive = Vec::new();
        write_archive(vec![("Projekte/Buch/text.md".into(), remote)], &mut archive).unwrap();
        let hash = hash_reader(archive.as_slice()).unwrap();
        let result = local
            .import(
                archive.as_slice(),
                &hash,
                "2026-10-28_2200_account01.tar.gz",
                "28.10. 22-00",
            )
            .unwrap();
        assert_eq!(result.konflikte.len(), 1);
        assert_eq!(
            fs::read_to_string(project.join("text.md")).unwrap(),
            "Lokal"
        );
        assert_eq!(
            fs::read_to_string(project.join("text (R2 28.10. 22-00).md")).unwrap(),
            "Von R2"
        );
        assert!(local.root.join(".sync/zustand.json").exists());
        assert_eq!(local.load_state().unwrap().v, 1);
    }
    #[test]
    fn pfadangriffe_hash_und_links() {
        for p in [
            "../a",
            "/tmp/a",
            "Projekte/../a",
            "C:/a",
            "Projekte/a\\b",
            "a//b",
        ] {
            assert!(safe_path(p).is_err(), "{p}");
        }
        let tmp = tempfile::tempdir().unwrap();
        let local = LocalFolder::create(tmp.path().join("cocopi")).unwrap();
        assert!(local
            .import(&b"kaputt"[..], "falsch", "a.tar.gz", "28.10. 22-00")
            .is_err());
        let mut h = header("Projekte/link", 0).unwrap();
        h[156] = b'2';
        h[148..156].fill(32);
        let sum = h.iter().map(|b| *b as u64).sum();
        number(&mut h, 148, 7, sum).unwrap();
        h[155] = 32;
        let mut zip = GzEncoder::new(Vec::new(), Compression::default());
        zip.write_all(&h).unwrap();
        zip.write_all(&[0; 1024]).unwrap();
        let data = zip.finish().unwrap();
        assert!(read_archive(data.as_slice(), tmp.path()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_ziel_bleibt_unberuehrt() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let local = LocalFolder::create(tmp.path().join("cocopi")).unwrap();
        let outside = tmp.path().join("privat");
        ensure_tree(&outside).unwrap();
        symlink(&outside, local.root.join("Projekte/Link")).unwrap();
        let source = tmp.path().join("source");
        fs::write(&source, "Nicht schreiben").unwrap();
        let mut data = Vec::new();
        write_archive(vec![("Projekte/Link/a.md".into(), source)], &mut data).unwrap();
        let hash = hash_reader(data.as_slice()).unwrap();
        assert!(local
            .import(data.as_slice(), &hash, "a.tar.gz", "28.10. 22-00")
            .is_err());
        assert!(!outside.join("a.md").exists());
    }
}
