//! cocopi HTTP/SSE v1. Keine lokalen Agenten, keine automatische Promptwiederholung.
//! Privatschlüssel bleiben unter `copi_config_dir/devices/` (I1: Datei, noch keine Keychain).
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    pub art: char,
    pub code: String,
    pub hub_url: Option<String>,
}

/// Fragment, Query oder Rohcode. Links legen niemals heimlich einen neuen Hub fest:
/// `redeem` verlangt die Übereinstimmung mit der bewusst konfigurierten Adresse.
pub fn parse_invitation(input: &str, context: char) -> Result<Invitation, String> {
    let text = input.trim();
    let (payload, hub_url) = if text.starts_with("http://") || text.starts_with("https://") {
        let end = text.find(['?', '#']).unwrap_or(text.len());
        let prefix = &text[..end];
        let origin = prefix.strip_suffix("/c").ok_or("Dieser Link ist kein cocopi-Einladungslink.")?;
        validate_url(origin)?;
        let payload = if let Some((_, fragment)) = text.split_once('#') {
            fragment.to_string()
        } else {
            text.split_once('?')
                .and_then(|(_, query)| query.split('&').find_map(|part| part.strip_prefix("c=")))
                .ok_or("Der Einladungslink enthält keinen Code.")?
                .replace("%2E", ".")
                .replace("%2e", ".")
                .replace("%2D", "-")
                .replace("%2d", "-")
        };
        (payload, Some(origin.trim_end_matches('/').to_string()))
    } else {
        (text.to_string(), None)
    };
    let (art, raw) = if let Some(rest) = payload.strip_prefix("1.") {
        let (kind, code) = rest.split_once('.').ok_or("Der Code ist unvollständig.")?;
        if kind.len() != 1 {
            return Err("Diese Codeart wird nicht unterstützt.".into());
        }
        (kind.chars().next().unwrap(), code)
    } else {
        (context, payload.as_str())
    };
    if !matches!(art, 'e' | 'k' | 'b') {
        return Err("Diese Codeart wird nicht unterstützt.".into());
    }
    let canonical: String = raw.chars().filter(|c| *c != '-').map(|c| c.to_ascii_uppercase()).collect();
    if canonical.len() != 16 || !canonical.bytes().all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b)) {
        return Err("Dieser Code ist unvollständig. Prüfe die 16 Zeichen.".into());
    }
    let code = canonical
        .as_bytes()
        .chunks(4)
        .map(|s| std::str::from_utf8(s).unwrap())
        .collect::<Vec<_>>()
        .join("-");
    Ok(Invitation { art, code, hub_url })
}
fn validate_url(url: &str) -> Result<(), String> {
    // Keine Credentials, Pfade, Steuerzeichen oder Klartext-Verbindungen außer Loopback.
    let authority = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("Die Hub-Adresse muss mit https:// beginnen.")?;
    if authority.is_empty()
        || authority.contains(['/', '?', '#', '@', '\\'])
        || authority.chars().any(char::is_whitespace)
        || authority.chars().any(char::is_control)
    {
        return Err("Die Hub-Adresse ist ungültig. Gib nur den Ursprung ohne Zugangsdaten ein.".into());
    }
    if url.starts_with("http://")
        && !(authority == "127.0.0.1" || authority.starts_with("127.0.0.1:") || authority == "[::1]" || authority.starts_with("[::1]:"))
    {
        return Err("Eine unverschlüsselte Verbindung ist nur im lokalen Test erlaubt.".into());
    }
    Ok(())
}

// Kein Debug/Clone für Geheimnisse. Nur öffentliche Schlüssel gehen über HTTP.
pub struct DeviceKey {
    signing: SigningKey,
    path: PathBuf,
    pub geraet_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct KeyFile {
    secret: String,
    geraet_id: Option<String>,
}
impl DeviceKey {
    /// `name` ist ein lokaler Speichername, niemals ein Pfad aus dem Netzwerk.
    pub fn load_or_create(name: &str) -> Result<Self, String> {
        let root = crate::copi_config_dir().ok_or("Kein Konfigurationsordner verfügbar.")?.join("devices");
        Self::at(&root, name)
    }
    pub fn at(root: &Path, name: &str) -> Result<Self, String> {
        if name.is_empty() || name.len() > 80 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            return Err("Ungültiger lokaler Gerätename.".into());
        }
        std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        }
        let path = root.join(format!("{name}.json"));
        match std::fs::read(&path) {
            Ok(bytes) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?.file_type().is_symlink()
                        || std::fs::metadata(&path).map_err(|e| e.to_string())?.permissions().mode() & 0o077 != 0
                    {
                        return Err("Die Schlüsseldatei ist nicht privat. Prüfe ihre Zugriffsrechte.".into());
                    }
                }
                let f: KeyFile = serde_json::from_slice(&bytes).map_err(|_| "Die Geräte-Schlüsseldatei ist beschädigt. Sie wurde nicht überschrieben.")?;
                let seed: [u8; 32] = B64
                    .decode(f.secret)
                    .map_err(|_| "Ungültiger Geräteschlüssel")?
                    .try_into()
                    .map_err(|_| "Ungültiger Geräteschlüssel")?;
                Ok(Self {
                    signing: SigningKey::from_bytes(&seed),
                    path,
                    geraet_id: f.geraet_id,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut seed = [0; 32];
                os_random(&mut seed)?;
                let key = Self {
                    signing: SigningKey::from_bytes(&seed),
                    path,
                    geraet_id: None,
                };
                key.save_new()?;
                Ok(key)
            }
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn public_key(&self) -> String {
        B64.encode(self.signing.verifying_key().as_bytes())
    }
    fn data(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(&KeyFile {
            secret: B64.encode(self.signing.as_bytes()),
            geraet_id: self.geraet_id.clone(),
        })
        .map_err(|e| e.to_string())
    }
    fn save_new(&self) -> Result<(), String> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&self.path).map_err(|e| e.to_string())?;
        f.write_all(&self.data()?).and_then(|_| f.sync_all()).map_err(|e| e.to_string())
    }
    fn remember(&mut self, id: &str) -> Result<(), String> {
        self.geraet_id = Some(id.to_string());
        let temp = self.path.with_extension(format!("{}.new", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&temp).map_err(|e| e.to_string())?;
        f.write_all(&self.data()?).and_then(|_| f.sync_all()).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, &self.path).map_err(|e| e.to_string())
    }
}
#[cfg(unix)]
fn os_random(bytes: &mut [u8]) -> Result<(), String> {
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(bytes))
        .map_err(|_| "Der sichere Zufallsgenerator ist nicht verfügbar.".into())
}
#[cfg(windows)]
fn os_random(bytes: &mut [u8]) -> Result<(), String> {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(handle: *mut std::ffi::c_void, buffer: *mut u8, length: u32, flags: u32) -> i32;
    }
    // BCRYPT_USE_SYSTEM_PREFERRED_RNG; dokumentierte Betriebssystem-API, keine eigene Kryptografie.
    let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), bytes.len() as u32, 2) };
    if status >= 0 {
        Ok(())
    } else {
        Err("Der sichere Zufallsgenerator ist nicht verfügbar.".into())
    }
}
#[cfg(not(any(unix, windows)))]
fn os_random(_: &mut [u8]) -> Result<(), String> {
    Err("Kein sicherer Zufallsgenerator auf dieser Plattform.".into())
}

pub struct HubV1 {
    url: String,
    token: Option<String>,
    pub token_until: Option<u64>,
    agent: ureq::Agent,
}
fn agent(stream: bool) -> ureq::Agent {
    let b = ureq::Agent::config_builder()
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(10)));
    if stream {
        b.timeout_recv_body(None).build().into()
    } else {
        b.timeout_global(Some(Duration::from_secs(15))).build().into()
    }
}
impl HubV1 {
    pub fn new(url: &str) -> Result<Self, String> {
        let url = url.trim_end_matches('/').to_string();
        validate_url(&url)?;
        Ok(Self {
            url,
            token: None,
            token_until: None,
            agent: agent(false),
        })
    }
    fn request(&self, method: &str, route: &str, body: Value) -> Result<Value, String> {
        let url = format!("{}/v1/{}", self.url, route);
        let mut response = if method == "GET" {
            let mut r = self.agent.get(&url);
            if let Some(t) = &self.token {
                r = r.header("Authorization", format!("Bearer {t}"));
            }
            r.call()
        } else {
            let mut r = self.agent.post(&url).header("Content-Type", "application/json");
            if let Some(t) = &self.token {
                r = r.header("Authorization", format!("Bearer {t}"));
            }
            let mut b = body;
            b["protokoll"] = json!(1);
            r.send(serde_json::to_vec(&b).map_err(|_| "Die Anfrage ist ungültig.")?)
        }
        .map_err(|_| "R2 ist gerade nicht erreichbar. Prüfe deine Internetverbindung.")?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .with_config()
            .limit(2 * 1024 * 1024)
            .read_to_string()
            .map_err(|_| "Die Antwort von R2 konnte nicht gelesen werden.")?;
        let value: Value = serde_json::from_str(&text).map_err(|_| "R2 hat keine gültige Protokollantwort gesendet.")?;
        check_packet(&value)?;
        if !(200..300).contains(&status) || value["typ"] == "fehler" {
            return Err(packet_error(&value));
        }
        Ok(value)
    }
    pub fn hello(&self, app: &str) -> Result<Value, String> {
        self.request("POST", "hallo", json!({"typ":"hallo","app":app}))
    }
    pub fn redeem(&self, key: &mut DeviceKey, input: &str, context: char, name: &str, kind: &str) -> Result<Value, String> {
        let invite = parse_invitation(input, context)?;
        if invite.hub_url.as_ref().is_some_and(|u| *u != self.url) {
            return Err("Der Link gehört zu einem anderen R2. Prüfe zuerst die Hub-Adresse.".into());
        }
        let result = self.request(
            "POST",
            "einloesen",
            json!({"typ":"einloesen","art":invite.art.to_string(),"code":invite.code,"pubkey":key.public_key(),"geraetename":name,"geraeteart":kind}),
        )?;
        if let Some(id) = result["geraet_id"].as_str() {
            key.remember(id)?;
        }
        Ok(result)
    }
    pub fn pairing_status(&self, key: &mut DeviceKey, id: &str) -> Result<Value, String> {
        let result = self.request("GET", &format!("koppeln/{}", identifier(id)?), Value::Null)?;
        if let Some(id) = result["geraet_id"].as_str() {
            key.remember(id)?;
        }
        Ok(result)
    }
    pub fn login(&mut self, key: &DeviceKey) -> Result<Value, String> {
        let id = key
            .geraet_id
            .as_ref()
            .ok_or("Dieses Gerät ist noch nicht registriert. Löse eine Einladung ein.")?;
        let c = self.request("POST", "anmelden/challenge", json!({"geraet_id":id}))?;
        let nonce = c["nonce"].as_str().ok_or("R2 hat keine Anmeldeprüfung gesendet.")?;
        let signature = key.signing.sign(nonce.as_bytes());
        let result = self.request(
            "POST",
            "anmelden",
            json!({"challenge_id":c["challenge_id"],"signatur":B64.encode(signature.to_bytes())}),
        )?;
        self.token = Some(result["zugangstoken"].as_str().ok_or("R2 hat die Anmeldung nicht bestätigt.")?.to_string());
        self.token_until = result["gueltig_bis"].as_u64();
        Ok(result)
    }
    pub fn sessions(&self) -> Result<Value, String> {
        self.request("GET", "sessions", Value::Null)
    }
    pub fn open_session(&self, session_id: Option<&str>, title: Option<&str>, workspace: Option<&str>) -> Result<Value, String> {
        self.request("POST", "sessions/oeffnen", json!({"session_id":session_id,"titel":title,"workspace":workspace}))
    }
    pub fn send(&self, session: &str, text: &str) -> Result<Value, String> {
        self.request("POST", &format!("sessions/{}/senden", identifier(session)?), json!({"text":text}))
    }
    pub fn stop(&self, session: &str) -> Result<Value, String> {
        self.request("POST", &format!("sessions/{}/stopp", identifier(session)?), json!({}))
    }
    pub fn rest(&self, session: &str) -> Result<Value, String> {
        self.request("POST", &format!("sessions/{}/ruhen", identifier(session)?), json!({}))
    }
    pub fn view(&self, session: &str) -> Result<Value, String> {
        self.request("POST", &format!("sessions/{}/ansehen", identifier(session)?), json!({}))
    }
    pub fn answer(&self, session: &str, question: &str, value: Value) -> Result<Value, String> {
        self.request(
            "POST",
            &format!("sessions/{}/antwort", identifier(session)?),
            json!({"frage_id":question,"wert":value}),
        )
    }
    pub fn start_pairing(&self) -> Result<Value, String> {
        self.request("POST", "koppeln/start", json!({}))
    }
    pub fn approve_pairing(&self, id: &str, allow: bool) -> Result<Value, String> {
        self.request("POST", "koppeln/bestaetigen", json!({"anfrage_id":id,"erlauben":allow}))
    }
    pub fn devices(&self) -> Result<Value, String> {
        self.request("GET", "geraete", Value::Null)
    }
    pub fn revoke(&self, id: &str) -> Result<Value, String> {
        self.request("POST", &format!("geraete/{}/widerrufen", identifier(id)?), json!({}))
    }
    /// Blocking-Reader für einen UI-Worker. EOF bedeutet getrennt, nicht automatisch erneut senden.
    pub fn events(&self, last_id: Option<&str>) -> Result<EventStream, String> {
        let token = self.token.as_ref().ok_or("Dieses Gerät ist nicht angemeldet.")?;
        let a = agent(true);
        let mut request = a.get(format!("{}/v1/ereignisse", self.url)).header("Authorization", format!("Bearer {token}"));
        if let Some(id) = last_id {
            request = request.header("Last-Event-ID", id);
        }
        let response = request.call().map_err(|_| "Die Verbindung zu R2 wurde unterbrochen.")?;
        if response.status().as_u16() != 200 {
            return Err("Die Ereignisverbindung wurde abgewiesen. Melde dich erneut an.".into());
        }
        Ok(EventStream {
            reader: BufReader::new(response.into_body().into_reader()),
        })
    }
}
fn identifier(id: &str) -> Result<&str, String> {
    if id.is_empty() || id.len() > 128 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        Err("Ungültige Geräte- oder Anfrage-ID.".into())
    } else {
        Ok(id)
    }
}
fn check_packet(value: &Value) -> Result<(), String> {
    if value["protokoll"] != 1 {
        return Err("cocopi und R2 sprechen unterschiedliche Protokollversionen. Bitte aktualisieren.".into());
    }
    if !value["typ"].is_string() {
        return Err("Die Antwort von R2 ist unvollständig.".into());
    }
    Ok(())
}
fn packet_error(v: &Value) -> String {
    format!(
        "{} {}",
        v["was"].as_str().unwrap_or("R2 hat die Anfrage abgewiesen."),
        v["tun"].as_str().unwrap_or("Versuche es erneut.")
    )
}

pub struct HubEvent {
    pub id: Option<String>,
    pub packet: Value,
}
pub struct EventStream {
    reader: BufReader<ureq::BodyReader<'static>>,
}
impl EventStream {
    pub fn next_event(&mut self) -> Result<Option<HubEvent>, String> {
        let mut id = None;
        let mut data = String::new();
        let mut line = String::new();
        let mut size = 0;
        loop {
            line.clear();
            if self
                .reader
                .by_ref()
                .take(2 * 1024 * 1024 + 1)
                .read_line(&mut line)
                .map_err(|_| "Die Verbindung zu R2 wurde unterbrochen.")?
                == 0
            {
                return Ok(None);
            }
            size += line.len();
            if size > 2 * 1024 * 1024 {
                return Err("Das Ereignis von R2 ist zu groß.".into());
            }
            let line = line.trim_end_matches(['\n', '\r']);
            if line.is_empty() {
                if data.is_empty() {
                    size = 0;
                    id = None;
                    continue;
                }
                let packet: Value = serde_json::from_str(&data).map_err(|_| "R2 hat ein ungültiges Ereignis gesendet.")?;
                check_packet(&packet)?;
                return Ok(Some(HubEvent { id, packet }));
            }
            if let Some(value) = line.strip_prefix("id:") {
                id = Some(value.trim_start().to_string());
            }
            if let Some(value) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.trim_start());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invitation_fragment_query_and_raw() {
        let code = "K7QM-2XDA-9FHT-W3NB";
        for link in [
            format!("https://r2.example/c#1.e.{code}"),
            format!("https://r2.example/c?c=1.e.{code}"),
            code.to_string(),
        ] {
            assert_eq!(parse_invitation(&link, 'e').unwrap().code, code);
        }
        assert_eq!(parse_invitation(code, 'k').unwrap().art, 'k');
        assert!(parse_invitation("https://evil.example/c#2.e.x", 'e').is_err());
        assert!(parse_invitation("http://evil.example/c#1.e.K7QM-2XDA-9FHT-W3NB", 'e').is_err());
        assert!(HubV1::new("https://user:password@r2.example").is_err());
    }
    #[test]
    fn client_against_started_node_hub_mock() {
        use std::process::{Command, Stdio};
        if Command::new("node").arg("--version").output().is_err() {
            eprintln!("Übersprungen: node fehlt.");
            return;
        }
        let hub = Path::new("/home/medienhaus/Work/pi-hub-wt/s3");
        if !hub.join("server.mjs").is_file() {
            eprintln!("Übersprungen: S3-Hub-Worktree fehlt.");
            return;
        }
        let dir = std::env::temp_dir().join(format!("hub-v1-client-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = dir.join("state");
        let envs = [
            ("HUB_SLOT", "account01".to_string()),
            ("HUB_STATE", state.to_string_lossy().into_owned()),
            ("HUB_TOKEN", "demo".to_string()),
            ("HUB_ROOTS", dir.to_string_lossy().into_owned()),
            ("HUB_PORT", "0".to_string()),
            ("HUB_HOST", "127.0.0.1".to_string()),
            ("HUB_NTFY_URL", String::new()),
            ("HUB_V1_HARNESS", "mock".to_string()),
            ("PI_SESSIONS_DIR", dir.join("sessions").to_string_lossy().into_owned()),
        ];
        let invite = Command::new("node")
            .current_dir(hub)
            .args(["server.mjs", "admin", "einladung", "--slot", "account01", "--name", "Mira"])
            .envs(envs.iter().map(|(k, v)| (*k, v)))
            .output()
            .unwrap();
        assert!(invite.status.success());
        let link = String::from_utf8(invite.stdout).unwrap();
        let code = parse_invitation(link.trim(), 'e').unwrap().code;
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = ChildGuard(
            Command::new("node")
                .current_dir(hub)
                .arg("server.mjs")
                .envs(envs.iter().map(|(k, v)| (*k, v)))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut first = String::new();
        BufReader::new(child.0.stdout.take().unwrap()).read_line(&mut first).unwrap();
        let url = first.trim().strip_prefix("pi-hub läuft auf ").unwrap();
        let mut client = HubV1::new(url).unwrap();
        assert_eq!(client.hello("cocopi-test").unwrap()["slot"], "account01");
        let mut key = DeviceKey::at(&dir.join("devices"), "desktop").unwrap();
        let registered = client.redeem(&mut key, &code, 'e', "Testrechner", "desktop").unwrap();
        assert_eq!(registered["person"], "Mira");
        assert!(client.redeem(&mut key, &code, 'e', "Testrechner", "desktop").unwrap_err().contains("benutzt"));
        client.login(&key).unwrap();
        assert_eq!(client.sessions().unwrap()["sessions"].as_array().unwrap().len(), 0);
        let mut events = client.events(None).unwrap();
        assert_eq!(events.next_event().unwrap().unwrap().packet["typ"], "sessions_liste");
        let session = client.open_session(None, Some("Entwurf"), None).unwrap();
        assert_eq!(session["zustand"], "bereit");
        // Öffnen erzeugt einen vom Hub bestätigten Zustand, keinen lokalen pi-Prozess.
        assert_eq!(events.next_event().unwrap().unwrap().packet["typ"], "session_zustand");
        drop(events);
        let pairing = client.start_pairing().unwrap();
        let mut phone_key = DeviceKey::at(&dir.join("devices"), "handy").unwrap();
        let mut phone = HubV1::new(url).unwrap();
        let pending = phone
            .redeem(&mut phone_key, pairing["code"].as_str().unwrap(), 'k', "Testhandy", "handy")
            .unwrap();
        assert_eq!(client.devices().unwrap()["geraete"].as_array().unwrap().len(), 1);
        let id = pending["anfrage_id"].as_str().unwrap();
        client.approve_pairing(id, true).unwrap();
        phone.pairing_status(&mut phone_key, id).unwrap();
        phone.login(&phone_key).unwrap();
        assert_eq!(phone.sessions().unwrap()["sessions"].as_array().unwrap().len(), 1);
        client.revoke(phone_key.geraet_id.as_ref().unwrap()).unwrap();
        assert!(phone.sessions().is_err());
        assert_eq!(client.devices().unwrap()["geraete"].as_array().unwrap().len(), 2);
        drop(child);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_key_not_replaced_and_foreign_link_rejected() {
        let dir = std::env::temp_dir().join(format!("hub-v1-corrupt-{}", std::process::id()));
        let mut key = DeviceKey::at(&dir, "desktop").unwrap();
        let client = HubV1::new("https://good.example").unwrap();
        assert!(client
            .redeem(&mut key, "https://other.example/c#1.e.K7QM-2XDA-9FHT-W3NB", 'e', "Test", "desktop")
            .unwrap_err()
            .contains("anderen R2"));
        std::fs::write(&key.path, "kaputt").unwrap();
        assert!(DeviceKey::at(&dir, "desktop").is_err());
        assert_eq!(std::fs::read_to_string(&key.path).unwrap(), "kaputt");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn device_key_private_roundtrip_and_path_guard() {
        let dir = std::env::temp_dir().join(format!("hub-v1-key-{}", std::process::id()));
        let key = DeviceKey::at(&dir, "rechner").unwrap();
        let second = DeviceKey::at(&dir, "rechner").unwrap();
        assert_eq!(key.public_key(), second.public_key());
        assert!(DeviceKey::at(&dir, "../escape").is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&key.path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
