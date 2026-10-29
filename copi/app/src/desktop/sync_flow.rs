//! Bewusster Dateifluss, ausschließlich im Worker. Kein Tokenexport, kein RAM-Archiv.
use super::checkin::*;
use pi_backend::hub_v1::{HubV1, sync_http::Sha256Stream};
use pi_sync::{Auswahl, LocalFolder, SelectionItem};
use serde_json::{Value, json};
use sha2::Digest;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) struct Hash(sha2::Sha256);
impl Sha256Stream for Hash {
    fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    fn finish(self: Box<Self>) -> [u8; 32] {
        self.0.finalize().into()
    }
}
pub(super) fn hash() -> Box<dyn Sha256Stream> {
    Box::new(Hash(sha2::Sha256::new()))
}

#[derive(Default)]
pub(super) struct Transfer {
    pub cancel: AtomicBool,
    progress: Mutex<Option<Progress>>,
}
impl Transfer {
    pub fn progress(&self) -> Option<Progress> {
        self.progress.lock().ok()?.clone()
    }
    pub fn begin(&self, kind: TransferKind, id: &str, total: u64) {
        *self.progress.lock().unwrap() = Some(Progress {
            id: id.into(),
            kind,
            bytes: 0,
            total,
        });
    }
    pub fn scope(&self, kind: TransferKind, id: &str, total: u64) -> TransferScope<'_> {
        self.begin(kind, id, total);
        TransferScope(self)
    }
    pub fn end(&self) {
        *self.progress.lock().unwrap() = None;
    }
    pub fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::SeqCst) {
            Err("Übertragung abgebrochen. Der Arbeitsplatz bleibt belegt; nichts wurde als erhalten bestätigt.".into())
        } else {
            Ok(())
        }
    }
    fn advance(&self, n: usize) {
        if let Some(p) = self.progress.lock().unwrap().as_mut() {
            p.bytes += n as u64;
        }
    }
}
pub(super) struct TransferScope<'a>(&'a Transfer);
impl Drop for TransferScope<'_> {
    fn drop(&mut self) {
        self.0.end();
    }
}
pub(super) struct Meter<'a, T> {
    pub io: T,
    pub transfer: &'a Transfer,
}
impl<T: Read> Read for Meter<'_, T> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.transfer.check().map_err(std::io::Error::other)?;
        let n = self.io.read(b)?;
        self.transfer.advance(n);
        Ok(n)
    }
}
impl<T: Write> Write for Meter<'_, T> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.transfer.check().map_err(std::io::Error::other)?;
        let n = self.io.write(b)?;
        self.transfer.advance(n);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.io.flush()
    }
}
struct Temp {
    path: PathBuf,
    file: File,
}
impl Temp {
    fn new(folder: &LocalFolder) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = folder.root.join(format!(
            ".sync/transfer-{}-{}-{}.tmp",
            std::process::id(),
            epoch(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let mut options = OpenOptions::new();
        options.write(true).read(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(|e| e.to_string())?;
        Ok(Self { path, file })
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(super) struct Plan {
    pub folder: LocalFolder,
    entries: Vec<SelectionItem>,
}
impl Plan {
    pub fn new(root: &Path) -> Result<Self, String> {
        let folder = LocalFolder::create(root).map_err(|e| e.to_string())?;
        let entries = folder.selection().map_err(|e| e.to_string())?;
        Ok(Self { folder, entries })
    }
    pub fn ui(&self) -> SyncPlan {
        let previous = self.folder.load_state().ok();
        let selected = previous
            .as_ref()
            .and_then(|s| s.auswahl.iter().find(|a| a.bereich == "Projekte"));
        let mut items = vec![];
        for (i, entry) in self.entries.iter().enumerate() {
            items.push(SyncItem {
                id: format!("eintrag-{i}"),
                name: format!("{} / {}", entry.bereich, entry.name),
                bytes: entry.bytes,
                selected: selected
                    .is_some_and(|a| a.bereich == entry.bereich && a.name == entry.name),
            });
            for (j, large) in entry.gross.iter().enumerate() {
                items.push(SyncItem {
                    id: format!("gross-{i}-{j}"),
                    name: format!(
                        "Große Datei · {} / {} / {} · einzeln bestätigen",
                        entry.bereich, entry.name, large.pfad
                    ),
                    bytes: large.bytes,
                    selected: false,
                });
            }
        }
        SyncPlan {
            items,
            excluded_bytes: self.entries.iter().map(|i| i.fluechtig_bytes).sum(),
        }
    }
    pub fn upload(
        &self,
        hub: &HubV1,
        id: &str,
        selection: &[String],
        transfer: &Transfer,
    ) -> Result<(), String> {
        let ui = self.ui();
        if selection.is_empty()
            || selection
                .iter()
                .any(|id| !ui.items.iter().any(|i| &i.id == id))
        {
            return Err(
                "Diese Auswahl ist nicht mehr aktuell. Prüfe den Arbeitsordner erneut.".into(),
            );
        }
        for (i, entry) in self.entries.iter().enumerate() {
            let has_large = entry
                .gross
                .iter()
                .enumerate()
                .any(|(j, _)| selection.contains(&format!("gross-{i}-{j}")));
            if has_large && !selection.contains(&format!("eintrag-{i}")) {
                return Err("Wähle auch den zugehörigen Eintrag aus. Große Dateien werden nur mit einzelnem Häkchen übertragen.".into());
            }
        }
        let mut state = self.folder.load_state().map_err(|e| e.to_string())?;
        state.letzter_checkin = Some(id.into());
        state.auswahl.clear();
        state.manifest.clear();
        let sync = hub.sync_http(hash);
        for (i, entry) in self.entries.iter().enumerate() {
            let large: Vec<String> = entry
                .gross
                .iter()
                .enumerate()
                .filter(|(j, _)| selection.contains(&format!("gross-{i}-{j}")))
                .map(|(_, f)| f.pfad.clone())
                .collect();
            if !selection.contains(&format!("eintrag-{i}")) && large.is_empty() {
                continue;
            }
            transfer.check()?;
            let chosen = Auswahl {
                bereich: entry.bereich.clone(),
                name: entry.name.clone(),
            };
            let mut temp = Temp::new(&self.folder)?;
            let manifest = self
                .folder
                .upload_explicit(
                    &chosen,
                    &large,
                    Meter {
                        io: &mut temp.file,
                        transfer,
                    },
                )
                .map_err(|e| e.to_string())?;
            transfer.check()?;
            temp.file.sync_all().map_err(|e| e.to_string())?;
            temp.file
                .seek(SeekFrom::Start(0))
                .map_err(|e| e.to_string())?;
            let size = temp.file.metadata().map_err(|e| e.to_string())?.len();
            // Abbruchmarke hier nicht zurücksetzen: Abbruch kann beim Archivbau eintreffen.
            *transfer.progress.lock().unwrap() = Some(Progress {
                id: id.into(),
                kind: TransferKind::Upload,
                bytes: 0,
                total: size,
            });
            let result = sync.hoch_explicit(
                &entry.bereich,
                &entry.name,
                Meter {
                    io: &mut temp.file,
                    transfer,
                },
                !large.is_empty(),
            )?;
            transfer.check()?;
            state.auswahl.push(chosen);
            for (path, hash) in manifest {
                state
                    .manifest
                    .insert(format!("{}/{}/{path}", entry.bereich, entry.name), hash);
            }
            self.folder.save_state(&state).map_err(|e| e.to_string())?;
            journal(
                &self.folder,
                json!({"ereignis":"hochgeladen","checkin_id":id,"bereich":entry.bereich,"name":entry.name,"dateien":result["dateien"],"bytes":result["bytes"]}),
            )?;
        }
        Ok(())
    }
}
pub(super) fn preview(v: &Value, id: String) -> Result<CheckoutPreview, String> {
    let files = v["persoenlich"]
        .as_array()
        .ok_or("Die Dateivorschau ist unvollständig.")?;
    let mut personal = vec![];
    for file in files {
        let status = file["status"].as_str().ok_or("Der Dateistatus fehlt.")?;
        if status == "unveraendert" {
            continue;
        }
        personal.push(ReturnFile {
            path: file["pfad"].as_str().ok_or("Der Dateipfad fehlt.")?.into(),
            bytes: file["bytes"].as_u64().ok_or("Die Dateigröße fehlt.")?,
            status: status.into(),
            large: false,
            selected: true,
        });
    }
    for file in v["gross"].as_array().into_iter().flatten() {
        personal.push(ReturnFile {
            path: file["pfad"]
                .as_str()
                .ok_or("Der Großdateipfad fehlt.")?
                .into(),
            bytes: file["bytes"].as_u64().ok_or("Die Dateigröße fehlt.")?,
            status: "groß · einzeln bestätigen".into(),
            large: true,
            selected: false,
        });
    }
    Ok(CheckoutPreview {
        id,
        personal,
        volatile_bytes: v["fluechtig"]["bytes"]
            .as_u64()
            .ok_or("Die Löschvorschau fehlt.")?,
        volatile_files: v["fluechtig"]["dateien"]
            .as_u64()
            .ok_or("Die Löschvorschau fehlt.")? as usize,
        examples: v["fluechtig"]["beispiele"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_owned))
            .collect(),
    })
}
pub(super) fn receive(
    folder: &LocalFolder,
    hub: &HubV1,
    meta: &Value,
    transfer: &Transfer,
) -> Result<ReturnSummary, String> {
    let sync = hub.sync_http(hash);
    sync.merken_archiv(meta)?;
    let id = meta["archiv_id"].as_str().ok_or("Die Archiv-ID fehlt.")?;
    let sha = meta["sha256"].as_str().ok_or("Die Prüfsumme fehlt.")?;
    let mut temp = Temp::new(folder)?;
    transfer.check()?;
    *transfer.progress.lock().unwrap() = Some(Progress {
        id: id.into(),
        kind: TransferKind::Checkout,
        bytes: 0,
        total: meta["bytes"].as_u64().unwrap_or(0),
    });
    sync.archiv_holen(
        id,
        Meter {
            io: &mut temp.file,
            transfer,
        },
    )?;
    temp.file.sync_all().map_err(|e| e.to_string())?;
    transfer.check()?;
    temp.file
        .seek(SeekFrom::Start(0))
        .map_err(|e| e.to_string())?;
    // LocalFolder::import prüft SHA-256 erneut, read_archive vollständig in Stage,
    // prüft sämtliche Ziele vor dem Import und schreibt ausschließlich create_new.
    // Kontrollierter Dateiname: die serverseitige opake ID ist kein lokaler Pfad.
    let archive_name = format!("{}-{}.tar.gz", epoch(), sha256_name(id.as_bytes()));
    let result = folder
        .import(
            Meter {
                io: &mut temp.file,
                transfer,
            },
            sha,
            &archive_name,
            &stamp(),
        )
        .map_err(|e| e.to_string())?;
    journal(
        folder,
        json!({"ereignis":"importiert","archiv_id":id,"sha256":sha,"dateien":result.dateien,"konflikte":result.konflikte.len()}),
    )?;
    transfer.check()?;
    Ok(ReturnSummary {
        files: result.dateien,
        bytes: meta["bytes"].as_u64().unwrap_or(0),
        conflicts: result.konflikte.len(),
    })
}
pub(super) fn journal(folder: &LocalFolder, mut event: Value) -> Result<(), String> {
    let path = folder.root.join(".sync/protokoll.jsonl");
    let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("Das lokale Sync-Protokoll ist keine reguläre Datei.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() != 1 {
            return Err("Das Sync-Protokoll ist verknüpft.".into());
        }
    }
    let mut options = OpenOptions::new();
    options.append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    event["zeit"] = json!(epoch());
    serde_json::to_writer(&mut file, &event).map_err(|e| e.to_string())?;
    file.write_all(b"\n")
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}
fn sha256_name(bytes: &[u8]) -> String {
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub(super) fn zeitpunkt(ms: u64) -> String {
    let seconds = ms / 1000;
    // Unix-Zeit in einen plattformneutralen gregorianischen Kalendertag umrechnen.
    let z = (seconds / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    format!(
        "{d:02}.{m:02}. {:02}:{:02} UTC",
        seconds / 3600 % 24,
        seconds / 60 % 60
    )
}
pub(super) fn stamp() -> String {
    // libc ist bereits direkte App-Abhängigkeit; keine neue Datumsbibliothek.
    #[cfg(unix)]
    unsafe {
        let t = epoch() as libc::time_t;
        let mut out: libc::tm = std::mem::zeroed();
        if !libc::localtime_r(&t, &mut out).is_null() {
            return format!(
                "{:02}.{:02}. {:02}-{:02}",
                out.tm_mday,
                out.tm_mon + 1,
                out.tm_hour,
                out.tm_min
            );
        }
    }
    // Plattformneutraler UTC-Kalender aus der Systemzeit, nur für den Dateinamen.
    let seconds = epoch();
    let days = (seconds / 86400) as i64;
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    format!(
        "{d:02}.{m:02}. {:02}-{:02}",
        seconds / 3600 % 24,
        seconds / 60 % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn meter_counts_stream_and_aborts_read_and_write() {
        let transfer = Transfer::default();
        {
            let _scope = transfer.scope(TransferKind::Upload, "test", 3);
            let mut reader = Meter {
                io: std::io::Cursor::new(b"abc"),
                transfer: &transfer,
            };
            let mut bytes = [0; 3];
            assert_eq!(reader.read(&mut bytes).unwrap(), 3);
            assert_eq!(transfer.progress().unwrap().bytes, 3);
            transfer.cancel.store(true, Ordering::SeqCst);
            assert!(reader.read(&mut bytes).is_err());
            assert!(
                Meter {
                    io: Vec::new(),
                    transfer: &transfer
                }
                .write(b"abc")
                .is_err()
            );
        }
        assert!(transfer.progress().is_none());
        assert!(
            transfer.check().is_err(),
            "Guard darf Abbruch nicht zurücksetzen"
        );
    }
    #[test]
    fn backend_stream_hash_is_real_sha256() {
        let mut h = hash();
        h.update(b"a");
        h.update(b"bc");
        let actual: String = h.finish().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            actual,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn server_archive_id_is_never_used_as_local_path() {
        let name = sha256_name(b"../../secrets/evil");
        assert_eq!(name.len(), 64);
        assert!(name.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
