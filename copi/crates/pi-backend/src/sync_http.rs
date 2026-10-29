//! HTTP-Sync im App-Worker. Keine automatische Upload-/Archivwiederholung.
//! SHA-256 aus einer geprüften Bibliothek injizieren (keine eigene Kryptografie).
use super::{HubV1, agent, check_packet, identifier, packet_error};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    io::{Read, Write},
};

const MAX_ARCHIVE: u64 = 2 * 1024 * 1024 * 1024;
/// Adapter für z. B. sha2::Sha256. finish liefert rohe 32 Hashbytes.
pub trait Sha256Stream {
    fn update(&mut self, bytes: &[u8]);
    fn finish(self: Box<Self>) -> [u8; 32];
}
/// Leiht den angemeldeten Hub: kein zweiter Token-Speicher, kein Tokenexport.
pub struct SyncHttp<'a> {
    hub: &'a HubV1,
    agent: ureq::Agent,
    sha256: Box<dyn Fn() -> Box<dyn Sha256Stream> + 'a>,
    archives: RefCell<HashMap<String, (String, u64)>>,
}
impl HubV1 {
    /// Nur mit echtem SHA-256 aus geprüfter Bibliothek verwenden.
    pub fn sync_http(&self, sha256: impl Fn() -> Box<dyn Sha256Stream> + 'static) -> SyncHttp<'_> {
        SyncHttp {
            hub: self,
            agent: agent(true),
            sha256: Box::new(sha256),
            archives: RefCell::new(HashMap::new()),
        }
    }
}
impl SyncHttp<'_> {
    fn token(&self) -> Result<String, String> {
        self.hub
            .token
            .as_ref()
            .map(|t| format!("Bearer {t}"))
            .ok_or_else(|| "Bitte melde dich zuerst am Arbeitsplatz an.".into())
    }
    /// tar.gz direkt vom Reader, ohne vollständige RAM-Kopie. Kein stilles Retry.
    pub fn hoch(&self, bereich: &str, name: &str, reader: impl Read) -> Result<Value, String> {
        self.hoch_explicit(bereich, name, reader, false)
    }
    /// Große Dateien nur nach ausdrücklicher Bestätigung durch die Person übertragen.
    /// `false` ist zum bisherigen `hoch` identisch (kein Bestätigungsheader).
    pub fn hoch_explicit(
        &self,
        bereich: &str,
        name: &str,
        mut reader: impl Read,
        gross_bestaetigt: bool,
    ) -> Result<Value, String> {
        if !["Projekte", "Ergebnisse", "Skills", "Verlaeufe"].contains(&bereich)
            || name.is_empty()
            || name.len() > 180
            || name == "."
            || name == ".."
            || name.contains(['/', '\\'])
            || name.chars().any(char::is_control)
        {
            return Err("Wähle einen gültigen Bereich und Dateinamen.".into());
        }
        let mut limited = (&mut reader).take(MAX_ARCHIVE + 1);
        let mut request = self
            .agent
            .post(format!("{}/v1/sync/hoch", self.hub.url))
            .header("Authorization", self.token()?)
            .header("Content-Type", "application/gzip")
            .header("X-Cocopi-Bereich", bereich)
            .header("X-Cocopi-Name", name);
        if gross_bestaetigt {
            request = request.header("X-Cocopi-Gross", "bestaetigt");
        }
        let response = request.send(ureq::SendBody::from_reader(&mut limited))
            .map_err(|_| "Die Übertragung wurde unterbrochen. Prüfe am Arbeitsplatz, was angekommen ist.")?;
        let v = response_packet(response)?;
        if v["typ"] != "sync_hoch_ok" || v["bereich"] != bereich || v["name"] != name {
            return Err("R2 hat die Übertragung nicht passend bestätigt.".into());
        }
        Ok(v)
    }
    pub fn vorschau(&self) -> Result<Value, String> {
        self.token()?;
        let v = self.hub.request("POST", "sync/vorschau", json!({}))?;
        if v["typ"] != "sync_vorschau" {
            return Err("R2 hat keine Dateivorschau gesendet.".into());
        }
        Ok(v)
    }
    /// Liefert erst archiv_bereit. Der Download bleibt ein bewusster eigener Schritt.
    pub fn archiv(&self, auswahl: Value) -> Result<Value, String> {
        self.token()?;
        if !auswahl.is_object() {
            return Err("Die Dateiauswahl ist ungültig.".into());
        }
        let v = self
            .hub
            .request("POST", "sync/archiv", json!({"auswahl":auswahl}))?;
        if v["typ"] != "archiv_bereit" {
            return Err("R2 hat das Archiv noch nicht bestätigt.".into());
        }
        self.merken_archiv(&v)?;
        Ok(v)
    }
    /// Auch für checkout_bereit des authentisierten Empfangs: erwartete Metadaten
    /// vor dem Download merken, niemals nur dem Download-Header vertrauen.
    pub fn merken_archiv(&self, meta: &Value) -> Result<(), String> {
        let id = identifier(meta["archiv_id"].as_str().ok_or("Die Archiv-ID fehlt.")?)?;
        let hash = meta["sha256"]
            .as_str()
            .filter(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            .ok_or("Die Archiv-Prüfsumme ist ungültig.")?;
        let size = meta["bytes"]
            .as_u64()
            .filter(|n| *n <= MAX_ARCHIVE)
            .ok_or("Das Archiv ist zu groß oder unvollständig.")?;
        let mut archives = self.archives.borrow_mut();
        if archives.len() >= 16 && !archives.contains_key(id) {
            return Err("Hole zuerst ein bereitgestelltes Archiv ab.".into());
        }
        if archives
            .get(id)
            .is_some_and(|old| old != &(hash.to_owned(), size))
        {
            return Err("Die Angaben zu diesem Archiv haben sich verändert.".into());
        }
        archives.insert(id.into(), (hash.into(), size));
        Ok(())
    }
    /// Writer muss eine neue temporäre Datei sein: bei Fehler NICHT entpacken oder
    /// erhalten senden. Erst Ok erlaubt atomare Übernahme / pi-sync-Rückspielen.
    pub fn archiv_holen(&self, id: &str, mut writer: impl Write) -> Result<(), String> {
        identifier(id)?;
        let (expected, size) = self
            .archives
            .borrow()
            .get(id)
            .cloned()
            .ok_or("Lade zuerst die bestätigten Archivangaben.")?;
        let mut response = self
            .agent
            .get(format!("{}/v1/sync/archiv/{id}", self.hub.url))
            .header("Authorization", self.token()?)
            .call()
            .map_err(|_| "Das Archiv konnte nicht abgeholt werden. Es bleibt auf R2.")?;
        if response.status().as_u16() != 200 {
            return Err(response_packet(response)
                .err()
                .unwrap_or_else(|| "R2 hat kein Archiv geliefert.".into()));
        }
        if response
            .headers()
            .get("X-Cocopi-Sha256")
            .and_then(|v| v.to_str().ok())
            != Some(expected.as_str())
        {
            return Err("Die Prüfsumme des Downloads passt nicht zum angekündigten Archiv.".into());
        }
        let mut hash = (self.sha256)();
        let mut reader = response.body_mut().as_reader();
        let mut bytes = 0u64;
        let mut buffer = [0; 64 * 1024];
        loop {
            let n = reader.read(&mut buffer).map_err(|_| "Der Archivdownload wurde unterbrochen. Die temporäre Datei darf nicht übernommen werden.")?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            if bytes > size {
                return Err("Das Archiv ist größer als angekündigt.".into());
            }
            hash.update(&buffer[..n]);
            writer.write_all(&buffer[..n]).map_err(
                |_| "Das Archiv konnte lokal nicht gespeichert werden. Prüfe den freien Speicher.",
            )?;
        }
        let actual: String = hash.finish().iter().map(|b| format!("{b:02x}")).collect();
        if bytes != size || actual != expected {
            return Err(
                "Das Archiv ist unvollständig oder verändert. Es wurde nicht bestätigt.".into(),
            );
        }
        writer
            .flush()
            .map_err(|_| "Das Archiv konnte nicht fertig gespeichert werden.")?;
        Ok(())
    }
}
fn response_packet(mut response: ureq::http::Response<ureq::Body>) -> Result<Value, String> {
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_string()
        .map_err(|_| "Die Antwort von R2 ist zu groß oder unvollständig.")?;
    let v: Value =
        serde_json::from_str(&text).map_err(|_| "R2 hat keine gültige Antwort gesendet.")?;
    check_packet(&v)?;
    if !(200..300).contains(&status) || v["typ"] == "fehler" {
        return Err(packet_error(&v));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        path::PathBuf,
        process::{Child, Command, Stdio},
    };
    // Test-only geprüfte Systembibliothek via sha256sum; kein Produktions-Hashersatz.
    struct TestHash(Vec<u8>);
    impl Sha256Stream for TestHash {
        fn update(&mut self, bytes: &[u8]) {
            self.0.extend_from_slice(bytes);
        }
        fn finish(self: Box<Self>) -> [u8; 32] {
            let mut child = Command::new("sha256sum")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&self.0).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success());
            let text = String::from_utf8(out.stdout).unwrap();
            std::array::from_fn(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap())
        }
    }
    struct Fixture {
        child: Child,
        root: PathBuf,
        hello: Value,
    }
    impl Fixture {
        fn new(mode: &str) -> Self {
            let repo = std::env::var("COCOPI_HUB_TEST_DIR")
                .expect("COCOPI_HUB_TEST_DIR auf Hub-S3-Worktree setzen (nur Mock-Hub)");
            let mut random = [0; 8];
            super::super::secure_random(&mut random).unwrap();
            let root =
                std::env::temp_dir().join(format!("sync-http-{:x}", u64::from_le_bytes(random)));
            std::fs::create_dir(&root).unwrap();
            let mut child = Command::new("node")
                .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sync-hub.mjs"))
                .env("COCOPI_HUB_TEST_DIR", repo)
                .env("HOME", &root)
                .env("HUB_TOKEN", "demo")
                .env_remove("HUB_EPOCHE_DATEI")
                .env_remove("HUB_EMPFANG_PUBKEY")
                .env("TEST_BAD_DOWNLOAD", mode)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap();
            let mut line = String::new();
            BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut line)
                .unwrap();
            let hello = serde_json::from_str(&line).expect("Mock-Hub gestartet");
            Self { child, root, hello }
        }
        fn login(&self) -> HubV1 {
            let mut key = super::super::DeviceKey::at(&self.root.join("keys"), "sync").unwrap();
            let mut hub = HubV1::new(self.hello["origin"].as_str().unwrap()).unwrap();
            hub.redeem(
                &mut key,
                self.hello["code"].as_str().unwrap(),
                'e',
                "Test",
                "desktop",
            )
            .unwrap();
            hub.login(&key).unwrap();
            hub
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            drop(self.child.stdin.take());
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn sync_http_real_hub_upload_preview_archive_and_corruption() {
        if std::env::var_os("COCOPI_HUB_TEST_DIR").is_none() {
            eprintln!("Sync-Hub-Test übersprungen: COCOPI_HUB_TEST_DIR nicht gesetzt.");
            return;
        }
        for mode in ["", "header", "body", "oversize"] {
            let f = Fixture::new(mode);
            let hub = f.login();
            let sync = hub.sync_http(|| Box::new(TestHash(Vec::new())));
            assert!(sync.agent.config().proxy().is_none());
            let hex = f.hello["upload"].as_str().unwrap();
            let upload: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(
                sync.hoch("Projekte", "Buch", upload.as_slice()).unwrap()["dateien"],
                1
            );
            assert!(f.root.join("Projekte/Buch/text.md").is_file());
            assert!(
                sync.hoch("Projekte", "../escape", upload.as_slice())
                    .is_err()
            );
            assert!(sync.hoch("fremd", "Buch", upload.as_slice()).is_err());
            let preview = sync.vorschau().unwrap();
            assert!(preview["persoenlich"].as_array().unwrap().len() >= 2);
            let a = sync.archiv(json!({})).unwrap();
            let id = a["archiv_id"].as_str().unwrap();
            assert!(sync.archiv_holen("unbekannt", Vec::new()).is_err());
            let mut out = Vec::new();
            let result = sync.archiv_holen(id, &mut out);
            assert_eq!(result.is_ok(), mode.is_empty(), "{mode}: {result:?}");
            if mode.is_empty() {
                assert_eq!(out.len() as u64, a["bytes"].as_u64().unwrap());
                assert_eq!(&out[..2], &[0x1f, 0x8b]);
            }
        }
    }
    #[test]
    fn explicit_upload_header_is_only_sent_after_confirmation() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut hub = HubV1::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        hub.token = Some("fixture-token".into());
        let server = std::thread::spawn(move || {
            for expected in [false, false, true] {
                let (stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(stream);
                let mut headers = String::new();
                let mut line = String::new();
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    headers.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                }
                assert!(headers.starts_with("POST /v1/sync/hoch "));
                let headers = headers.to_ascii_lowercase();
                assert!(headers.contains("authorization: bearer fixture-token"));
                assert_eq!(headers.contains("x-cocopi-gross: bestaetigt"), expected);
                // SendBody Reader nutzt Chunked-Encoding; leerer Reader endet mit 0.
                let mut end = [0; 5];
                reader.read_exact(&mut end).unwrap();
                assert_eq!(&end, b"0\r\n\r\n");
                let text =
                    json!({"protokoll":1,"typ":"sync_hoch_ok","bereich":"Projekte","name":"Test"})
                        .to_string();
                write!(
                    reader.get_mut(),
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                    text.len()
                )
                .unwrap();
            }
        });
        let sync = hub.sync_http(|| Box::new(TestHash(Vec::new())));
        sync.hoch("Projekte", "Test", std::io::empty()).unwrap();
        sync.hoch_explicit("Projekte", "Test", std::io::empty(), false)
            .unwrap();
        sync.hoch_explicit("Projekte", "Test", std::io::empty(), true)
            .unwrap();
        server.join().unwrap();
    }
    #[test]
    fn slow_archive_and_upload_survive_total_budget_but_not_idle() {
        use std::time::{Duration, Instant};
        // 60 s stetig im Maßstab 1:100; früheres 45-s-Budget = 450 ms.
        for upload in [false, true] {
            for steady in [true, false] {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let mut hub =
                    HubV1::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
                hub.token = Some("mock".into());
                let server = std::thread::spawn(move || {
                    let (stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .unwrap();
                    let mut r = BufReader::new(stream);
                    loop {
                        let mut line = String::new();
                        assert_ne!(r.read_line(&mut line).unwrap(), 0);
                        if line == "\r\n" {
                            break;
                        }
                    }
                    if upload {
                        // Ein Byte je Chunk, komplett framen; kein read==HTTP-Nachricht.
                        loop {
                            let mut line = String::new();
                            if r.read_line(&mut line).unwrap_or(0) == 0 {
                                return;
                            }
                            let n = usize::from_str_radix(line.trim(), 16).unwrap();
                            let mut bytes = vec![0; n + 2];
                            if r.read_exact(&mut bytes).is_err() {
                                return;
                            }
                            if n == 0 {
                                break;
                            }
                        }
                        if !steady {
                            std::thread::sleep(Duration::from_millis(750));
                            return;
                        }
                        let text = json!({"protokoll":1,"typ":"sync_hoch_ok","bereich":"Projekte","name":"Test"}).to_string();
                        let _ = write!(
                            r.get_mut(),
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                            text.len()
                        );
                    } else {
                        let hash = Box::new(TestHash(vec![b'x'; 6])).finish();
                        let hash: String = hash.iter().map(|b| format!("{b:02x}")).collect();
                        write!(r.get_mut(), "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nX-Cocopi-Sha256: {hash}\r\nConnection: close\r\n\r\n").unwrap();
                        if steady {
                            for _ in 0..6 {
                                std::thread::sleep(Duration::from_millis(100));
                                r.get_mut().write_all(b"x").unwrap();
                                r.get_mut().flush().unwrap();
                            }
                        } else {
                            std::thread::sleep(Duration::from_millis(750));
                        }
                    }
                });
                struct SlowBody {
                    left: usize,
                }
                impl Read for SlowBody {
                    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                        if self.left == 0 || b.is_empty() {
                            return Ok(0);
                        }
                        std::thread::sleep(Duration::from_millis(100));
                        self.left -= 1;
                        b[0] = b'x';
                        Ok(1)
                    }
                }
                let mut sync = hub.sync_http(|| Box::new(TestHash(Vec::new())));
                sync.agent = super::super::idle_transport::agent(Duration::from_millis(450));
                let started = Instant::now();
                let result = if upload {
                    // Ein lokaler Reader kann selbst blockieren; der Read-Leerlauf gilt
                    // fürs Netz. Idle-Upload daher über stumme Serverantwort testen.
                    sync.hoch("Projekte", "Test", SlowBody { left: 6 })
                        .map(|_| ())
                } else {
                    let hash = Box::new(TestHash(vec![b'x'; 6])).finish();
                    let hash: String = hash.iter().map(|b| format!("{b:02x}")).collect();
                    sync.merken_archiv(&json!({"archiv_id":"slow","sha256":hash,"bytes":6}))
                        .unwrap();
                    sync.archiv_holen("slow", Vec::new())
                };
                if steady {
                    assert!(
                        result.is_ok(),
                        "upload={upload}, steady={steady}: {result:?}"
                    );
                    assert!(started.elapsed() >= Duration::from_millis(600));
                } else {
                    assert!(result.is_err());
                    assert!(
                        started.elapsed() < Duration::from_millis(if upload { 1300 } else { 700 })
                    );
                }
                server.join().unwrap();
            }
        }
    }
    #[test]
    fn sync_metadata_and_unauthenticated_calls_fail_before_network() {
        let hub = HubV1::new("http://127.0.0.1:1").unwrap();
        let sync = hub.sync_http(|| Box::new(TestHash(Vec::new())));
        assert!(sync.vorschau().is_err());
        assert!(sync.archiv(json!({})).is_err());
        assert!(sync.hoch("Projekte", "test", &b""[..]).is_err());
        assert!(
            sync.merken_archiv(&json!({"archiv_id":"../escape","sha256":"0".repeat(64),"bytes":1}))
                .is_err()
        );
        assert!(
            sync.merken_archiv(
                &json!({"archiv_id":"test","sha256":"0".repeat(64),"bytes":MAX_ARCHIVE+1})
            )
            .is_err()
        );
        sync.merken_archiv(&json!({"archiv_id":"test","sha256":"0".repeat(64),"bytes":1}))
            .unwrap();
        assert!(
            sync.merken_archiv(&json!({"archiv_id":"test","sha256":"1".repeat(64),"bytes":1}))
                .is_err()
        );
    }
}
