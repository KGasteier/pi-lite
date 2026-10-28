//! Empfang in App-Workern. Fehlende Sync-HTTP-APIs werden nicht umgangen.
//! Ohne bestätigten Empfangsschlüssel und eigenen Archivschlüssel kein Check-in.
use super::{checkin::*, hub::HubAdapter, Account, DesktopApi, InviteError, Reply, Request};
use pi_backend::hub_v1::{empfang::Empfang, oidc, DeviceKey};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

type Sha256 = fn(&[u8]) -> [u8; 32];

/// Geprüfte SHA-256-Implementierung (RustCrypto `sha2`, bereits über pi-sync im Lockfile).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(data).into()
}
/// Bereits ausdrücklich bestätigte Konfiguration; niemals aus Peer-JSON ableiten.
pub struct Config {
    pub origin: String,
    pub issuer: String,
    pub empfang_pubkey: String,
    pub archiv_pubkey: String,
    pub net_pubkey: String,
    pub device_name: String,
    pub key_dir: PathBuf,
    pub sha256: Option<Sha256>,
}
struct State {
    empfang: Empfang,
    key: DeviceKey,
    attempt: u64,
    queue: Option<String>,
    receipt: Option<Receipt>,
    login: Option<mpsc::Receiver<Result<Value, String>>>,
    browser_url: Option<String>,
    hub: Option<Arc<HubAdapter>>,
    checkout: Option<Value>,
    cancelled: bool,
    checkin_pending: bool,
}
pub struct FlowAdapter {
    config: Config,
    state: Mutex<State>,
    cancelled_attempt: AtomicU64,
}
impl FlowAdapter {
    pub fn new(config: Config) -> Result<Self, String> {
        if config.empfang_pubkey.is_empty() || config.archiv_pubkey.is_empty() {
            return Err("Bestätige zuerst den Empfangsschlüssel und richte den eigenen Archivschlüssel ein. Es wurde kein Platz vergeben.".into());
        }
        let empfang = Empfang::new(&config.origin)?;
        let key = DeviceKey::at(&config.key_dir, "seminar-desktop")?;
        Ok(Self {
            config,
            cancelled_attempt: AtomicU64::new(0),
            state: Mutex::new(State {
                empfang,
                key,
                attempt: 0,
                queue: None,
                receipt: None,
                login: None,
                browser_url: None,
                hub: None,
                checkout: None,
                cancelled: false,
                checkin_pending: false,
            }),
        })
    }
    fn checkin(&self, s: &mut State, proof: Value) -> Result<FlowReply, String> {
        let mut body = json!({
            "nachweis":proof,
            "geraet":{"pubkey":s.key.public_key(),"name":self.config.device_name,"art":"desktop"},
            "archiv_pubkey":self.config.archiv_pubkey,
        });
        if !self.config.net_pubkey.is_empty() {
            body["net_pubkey"] = json!(self.config.net_pubkey);
        }
        if s.cancelled || self.cancelled_attempt.load(Ordering::SeqCst) == s.attempt {
            return Ok(FlowReply::Cancelled);
        }
        // Eine verlorene Antwort kann bereits einen Slot belegen: nicht neu versuchen.
        s.checkin_pending = true;
        let result = s.empfang.checkin(body)?;
        self.accept(s, result)
    }
    fn accept(&self, s: &mut State, result: Value) -> Result<FlowReply, String> {
        if result["typ"] == "warteschlange" {
            let id = field(&result, "warte_id")?;
            s.queue = Some(id.clone());
            s.checkin_pending = false;
            return Ok(FlowReply::Queue {
                attempt: s.attempt,
                id,
                position: result["platz"].as_u64().ok_or("Der Warteplatz fehlt.")? as usize,
            });
        }
        s.empfang
            .uebernehmen(&result, &mut s.key, &self.config.empfang_pubkey, now_ms())?;
        let id = field(&result, "checkin_id")?;
        let slot = field(&result, "slot")?;
        let account = Account {
            person: field(&result, "person")?,
            slot: slot.clone(),
            device_id: s.key.geraet_id.clone().ok_or("Die Geräte-ID fehlt.")?,
            device_name: self.config.device_name.clone(),
        };
        let receipt = Receipt {
            checkin_id: id,
            account,
            since: "jetzt".into(),
            pickup: None,
        };
        s.receipt = Some(receipt.clone());
        s.checkin_pending = false;
        s.queue = None;
        if s.cancelled || self.cancelled_attempt.load(Ordering::SeqCst) == s.attempt {
            // N6: Rückgabe vor Slot-Login, kein Upload nach Vergabe.
            s.checkout = Some(s.empfang.checkout(&receipt.checkin_id)?);
            if s.checkout
                .as_ref()
                .is_some_and(|v| v["typ"] == "ausgecheckt")
            {
                s.receipt = None;
                return Ok(FlowReply::Cancelled);
            }
            return Err("Der Platz wurde während des Abbruchs vergeben. Die sichere Rückgabe wurde ohne Upload angefordert; der Empfang hat noch keine archivfreie Freigabe bestätigt.".into());
        }
        // Empfangsignatur und Gerätebindung sind vor Slot-Login geprüft.
        let mut hub = pi_backend::hub_v1::HubV1::new(&format!(
            "{}/slot/{slot}",
            self.config.origin.trim_end_matches('/')
        ))?;
        hub.login(&s.key)?;
        let key = DeviceKey::at(&self.config.key_dir, "seminar-desktop")?;
        s.hub = Some(Arc::new(HubAdapter::authenticated(
            hub,
            key,
            self.config.device_name.clone(),
        )));
        Ok(FlowReply::CheckedIn(receipt))
    }
    fn invoke(&self, request: Request) -> Result<Reply, String> {
        // Muss auch während eines blockierenden Check-in-HTTP-Aufrufs greifen.
        if let Request::CancelFlow(target) = &request {
            self.cancelled_attempt
                .store(target.attempt, Ordering::SeqCst);
        }
        let mut s = self
            .state
            .lock()
            .map_err(|_| "Der Empfangsablauf wurde unterbrochen.")?;
        let reply = match request {
            Request::Checkin { attempt, method } => {
                if attempt == 0 || self.cancelled_attempt.load(Ordering::SeqCst) == attempt {
                    return Ok(Reply::Flow(FlowReply::Cancelled));
                }
                if s.checkin_pending {
                    return Err("Die vorherige Check-in-Antwort fehlt oder konnte nicht geprüft werden. Prüfe den Empfang; kein zweiter Check-in wird gesendet.".into());
                }
                if s.receipt.is_some() || s.queue.is_some() || s.login.is_some() {
                    return Err("Beende zuerst den laufenden Check-in. Es wurde kein zweiter Platz angefordert.".into());
                }
                s.attempt = attempt;
                s.cancelled = false;
                let hello = s.empfang.hallo()?;
                if hello["empfang_pubkey"] != self.config.empfang_pubkey
                    || hello["vertrag"] != "cocopi-pool-v1"
                {
                    return Err(
                        "Der Empfang passt nicht zum bestätigten Schlüssel oder Vertrag.".into(),
                    );
                }
                match method {
                    CheckinMethod::Guest(code) => {
                        self.checkin(&mut s, json!({"art":"einladung","code":code}))?
                    }
                    CheckinMethod::Udk => {
                        if hello["oidc"]["issuer"] != self.config.issuer {
                            return Err("Der Empfang meldet einen anderen Anmeldedienst.".into());
                        }
                        let hash = self.config.sha256.ok_or("Der geprüfte SHA-256-Callback ist in dieser App noch nicht angebunden. Keine Browseranmeldung gestartet.")?;
                        let discovery = oidc::Discovery::load(&self.config.issuer)?;
                        let login = oidc::Login::begin(
                            discovery,
                            &field(&hello["oidc"], "client_id")?,
                            &["openid", "profile", "groups"],
                            hash,
                        )?;
                        let mut url = String::new();
                        login.open_browser(|u| {
                            validate_browser_url(&self.config.issuer, u)?;
                            url = u.into();
                            Ok(())
                        })?;
                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            let _ = tx.send(login.finish(Duration::from_secs(120)));
                        });
                        s.login = Some(rx);
                        s.browser_url = Some(url.clone());
                        FlowReply::Browser { attempt, url }
                    }
                }
            }
            Request::Wait(WaitTarget::Browser { attempt }) => {
                if attempt != s.attempt {
                    return Err("Diese Anmeldung ist nicht mehr aktuell.".into());
                }
                let rx = s
                    .login
                    .as_ref()
                    .ok_or("Die Browseranmeldung ist nicht mehr aktiv.")?;
                let result = match rx.try_recv() {
                    Ok(result) => result,
                    Err(mpsc::TryRecvError::Empty) => return Ok(Reply::Flow(FlowReply::Waiting)),
                    Err(_) => return Err("Die Browseranmeldung wurde unterbrochen.".into()),
                };
                s.login = None;
                s.browser_url = None;
                self.checkin(&mut s, result?)?
            }
            Request::AuthCallback(callback) => {
                if callback.attempt != s.attempt {
                    return Err("Diese Anmeldeantwort ist veraltet.".into());
                }
                if callback.code.is_empty() || callback.state.is_empty() {
                    return Err("Die Anmeldeantwort ist unvollständig.".into());
                }
                return Err("Externe Code-/State-Angaben werden nicht akzeptiert. Die Anmeldung muss über den geprüften lokalen OIDC-Rückweg eintreffen.".into());
            }
            Request::Wait(WaitTarget::Queue { attempt, id }) => {
                if attempt != s.attempt || s.queue.as_ref() != Some(&id) {
                    return Err("Dieser Warteauftrag ist nicht mehr aktuell.".into());
                }
                let result = s.empfang.warten(&id)?;
                self.accept(&mut s, result)?
            }
            Request::CancelFlow(target) => {
                if target.attempt != s.attempt {
                    return Err("Dieser Abbruch gehört zu einem anderen Versuch.".into());
                }
                s.cancelled = true;
                s.login = None;
                s.browser_url = None;
                if let Some(id) = target
                    .checkin_id
                    .or_else(|| s.receipt.as_ref().map(|r| r.checkin_id.clone()))
                {
                    require_receipt(&s, &id)?;
                    s.checkout = Some(s.empfang.checkout(&id)?);
                    if s.checkout
                        .as_ref()
                        .is_some_and(|v| v["typ"] == "ausgecheckt")
                    {
                        s.receipt = None;
                        s.hub = None;
                        return Ok(Reply::Flow(FlowReply::Cancelled));
                    }
                    return Err("Rückgabe ohne weiteren Upload angefordert. Ohne verifiziertes Rückspielen oder explizite Leerfreigabe wird kein Check-out bestätigt.".into());
                }
                if s.checkin_pending {
                    return Err("Der Check-in-Ausgang ist unklar. Ohne Check-in-ID kann keine sichere Rückgabe bestätigt werden; bitte Empfang prüfen.".into());
                }
                if s.queue.is_some() || target.transfer_id.is_some() {
                    return Err("Der Empfang bietet noch keinen bestätigten Wartelisten-/Transferabbruch. Die Belegung bleibt sichtbar; es wird keine Freigabe behauptet.".into());
                }
                FlowReply::Cancelled
            }
            Request::SyncPlan(id) => {
                require_receipt(&s, &id)?;
                return Err("Die App bindet pi-sync noch nicht ein. Es wurde keine Dateiauswahl erfunden und nichts übertragen.".into());
            }
            Request::Upload {
                checkin_id,
                selection,
            } => {
                require_receipt(&s, &checkin_id)?;
                if s.cancelled
                    || self.cancelled_attempt.load(Ordering::SeqCst) == s.attempt
                    || selection.is_empty()
                {
                    return Err("Kein Upload: abgebrochen oder keine bestätigte Auswahl.".into());
                }
                return Err(
                    "HubV1 bietet noch keinen Streaming-Sync-Upload. Es wurde nichts übertragen."
                        .into(),
                );
            }
            Request::CheckoutPreview(id) => {
                require_receipt(&s, &id)?;
                return Err("HubV1 bietet noch keine Sync-Vorschau. Ohne aktuelle Vorschau wird kein Rückgabe-Archiv bestätigt.".into());
            }
            Request::Checkout {
                checkin_id,
                preview_id,
            } => {
                require_receipt(&s, &checkin_id)?;
                if preview_id.is_empty() {
                    return Err("Die bestätigte Rückgabevorschau fehlt.".into());
                }
                return Err("Archivdownload und pi-sync-Import fehlen in dieser App. Es wird ausdrücklich kein erhalten gesendet und kein Platz gelöscht.".into());
            }
            Request::Collect {
                checkin_id,
                pickup_id,
            } => {
                require_receipt(&s, &checkin_id)?;
                if pickup_id.is_empty() {
                    return Err("Das Abholfach fehlt.".into());
                }
                return Err("Die vorhandene Empfangs-API bietet keinen Abholfachabruf. Es wurde nichts als abgeholt bestätigt.".into());
            }
            Request::Wait(WaitTarget::Transfer { checkin_id, .. }) => {
                require_receipt(&s, &checkin_id)?;
                return Err("Es ist kein bestätigter Transfer aktiv.".into());
            }
            other => {
                let hub = s.hub.clone().ok_or("Du bist noch nicht eingecheckt.")?;
                drop(s);
                return Ok(hub.call(other));
            }
        };
        Ok(Reply::Flow(reply))
    }
}
impl DesktopApi for FlowAdapter {
    fn open_browser(&self, url: &str) -> Result<(), String> {
        validate_browser_url(&self.config.issuer, url)?;
        let s = self
            .state
            .lock()
            .map_err(|_| "Die Anmeldung wurde unterbrochen.")?;
        if s.login.is_none() || s.browser_url.as_deref() != Some(url) {
            return Err("Dieser Link gehört nicht zum aktuellen Anmeldeversuch.".into());
        }
        pi_backend::cli::open_url(url)
    }
    fn call(&self, request: Request) -> Reply {
        self.invoke(request)
            .unwrap_or_else(|e| Reply::Error(InviteError::Other(e)))
    }
    fn watch(&self, tx: mpsc::Sender<Reply>) {
        let hub = self.state.lock().ok().and_then(|s| s.hub.clone());
        if let Some(hub) = hub {
            hub.watch(tx);
        }
    }
}
fn require_receipt(s: &State, id: &str) -> Result<(), String> {
    if s.receipt.as_ref().is_some_and(|r| r.checkin_id == id) {
        Ok(())
    } else {
        Err("Dieser Auftrag gehört nicht zum aktuellen Check-in.".into())
    }
}
fn field(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("Die Empfangsantwort enthält kein {key}."))
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn browser_is_bound_to_https_issuer_origin() {
        for url in [
            "https://id.example/authorize?state=a",
            "https://id.example/token",
        ] {
            assert!(validate_browser_url("https://id.example/application/o/cocopi", url).is_ok());
        }
        for url in [
            "http://id.example/authorize",
            "https://id.example.evil/",
            "https://id.example@evil/",
            "file:///tmp/x",
            "https://id.example/\\evil",
            "https://id.example/#x",
            "https://id.example/\n",
        ] {
            assert!(
                validate_browser_url("https://id.example/issuer", url).is_err(),
                "{url:?}"
            );
        }
        assert!(
            validate_browser_url("http://127.0.0.1:1234", "http://127.0.0.1:1234/authorize")
                .is_err()
        );
    }

    struct Fixture {
        child: std::process::Child,
        root: PathBuf,
        info: Value,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    impl Fixture {
        fn start() -> Option<Self> {
            if Command::new("node").arg("--version").output().is_err() {
                eprintln!("SKIP: node fehlt; Empfang/Fake-OIDC nicht gestartet.");
                return None;
            }
            let source = std::env::var_os("COCOPI_PI_HUB")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/medienhaus/Work/pi-hub"));
            assert!(
                source.join("empfang/test/fake-oidc.mjs").is_file(),
                "Read-only Testquellen fehlen"
            );
            let root = std::env::temp_dir().join(format!(
                "cocopi-i2b-s2-{}-{}-{}",
                std::process::id(),
                now_ms(),
                NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(root.join("home")).unwrap();
            let script = r#"
import fs from 'node:fs'; import path from 'node:path'; import http from 'node:http';
import crypto from 'node:crypto'; import {pathToFileURL} from 'node:url';
const [source,root]=process.argv.slice(1);
const {createEmpfang}=await import(pathToFileURL(path.join(source,'empfang/empfang.mjs')));
const {Oidc}=await import(pathToFileURL(path.join(source,'empfang/oidc.mjs')));
const {createFakeOidc}=await import(pathToFileURL(path.join(source,'empfang/test/fake-oidc.mjs')));
const {createV1}=await import(pathToFileURL(path.join(source,'lib/hub-v1.mjs')));
const fake=await createFakeOidc(); let reception,hub,origin;
const server=http.createServer((req,res)=>{
  if(req.url.startsWith('/slot/account01/')) {
    req.url=req.url.slice('/slot/account01'.length);
    return hub.handle(req,res,new URL(req.url,origin));
  }
  return reception.handle(req,res);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
origin='http://127.0.0.1:'+server.address().port;
const work=path.join(root,'home/arbeit'); fs.mkdirSync(work,{recursive:true,mode:0o700});
const calls=[];
fs.mkdirSync(path.join(root,'epochen'),{recursive:true}); fs.writeFileSync(path.join(root,'epochen/account01.epoche'),'0\n');
reception=createEmpfang({dir:path.join(root,'empfang'),origin,slots:{account01:origin},epochenDir:path.join(root,'epochen'),
 oidc:new Oidc({issuer:fake.issuer,clientId:fake.clientId,allowHttp:true}),
 adapter:{register:async(_slot,belegung)=>{ if(fs.existsSync(path.join(root,'delay'))){fs.writeFileSync(path.join(root,'registering'),'yes');await new Promise(resolve=>setTimeout(resolve,200));} const response=await fetch(origin+'/slot/account01/v1/belegung',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({protokoll:1,belegung})}); if(!response.ok)throw Error('register'); },
 release:async()=>{throw Error('Freigabe ohne Import verboten');},
 archive:async()=>{calls.push('archiv');fs.writeFileSync(path.join(root,'calls.json'),JSON.stringify(calls));return {archiv_id:crypto.randomUUID(),sha256:'a'.repeat(64),bytes:0};}},
 clearSlot:async()=>{throw Error('Echtes Leeren verboten');}});
hub=createV1({dir:path.join(root,'hub'),slot:'account01',hubUrl:origin,roots:[work],inRoots:p=>p===work?p:null,
 empfangPubkey:reception.ausweis.pubkey,epochFile:path.join(root,'epochen/account01.epoche'),createInstance:()=>{throw Error('Modellstart verboten');}});
const codes=['test-invitation-one','test-invitation-two'];
fs.writeFileSync(path.join(root,'empfang/nachweise.json'),JSON.stringify({tokens:[],einladungen:codes.map(code=>({hash:crypto.createHash('sha256').update(code).digest('hex'),person:'Mira',bis:Date.now()+300000}))}),{mode:0o600});
const archive=crypto.generateKeyPairSync('x25519');
console.log(JSON.stringify({origin,issuer:fake.issuer,pubkey:reception.ausweis.pubkey,
 archiv_pubkey:archive.publicKey.export({format:'der',type:'spki'}).subarray(-32).toString('base64url'),
 proof:{art:'oidc',id_token:fake.token({sub:'rust-adapter'}),nonce:fake.nonce}}));
"#;
            let child = Command::new("node")
                .args(["--input-type=module", "-e", script])
                .arg(source)
                .arg(&root)
                .env("HOME", root.join("home"))
                .env_remove("HUB_EMPFANG_PUBKEY")
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap();
            let mut f = Self {
                child,
                root,
                info: Value::Null,
            };
            let mut line = String::new();
            BufReader::new(f.child.stdout.take().unwrap())
                .read_line(&mut line)
                .unwrap();
            f.info =
                serde_json::from_str(&line).expect("Lokaler Empfang muss seine Testadresse melden");
            Some(f)
        }
        fn adapter(&self, name: &str) -> FlowAdapter {
            FlowAdapter::new(Config {
                origin: field(&self.info, "origin").unwrap(),
                issuer: field(&self.info, "issuer").unwrap(),
                empfang_pubkey: field(&self.info, "pubkey").unwrap(),
                archiv_pubkey: field(&self.info, "archiv_pubkey").unwrap(),
                net_pubkey: String::new(),
                device_name: name.into(),
                key_dir: self.root.join(name),
                sha256: None,
            })
            .unwrap()
        }
    }
    #[test]
    fn local_node_empfang_guest_queue_cancel_and_no_fake_transfer_success() {
        let Some(f) = Fixture::start() else {
            return;
        };
        let first = f.adapter("one");
        let reply = first.call(Request::Checkin {
            attempt: 1,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        });
        let Reply::Flow(FlowReply::CheckedIn(receipt)) = reply else {
            panic!("{reply:?}")
        };
        assert_eq!(receipt.account.person, "Mira");
        assert!(matches!(first.call(Request::Sessions), Reply::Sessions(_)));
        assert!(matches!(
            first.call(Request::SyncPlan(receipt.checkin_id.clone())),
            Reply::Error(_)
        ));
        assert!(matches!(
            first.call(Request::Upload {
                checkin_id: receipt.checkin_id.clone(),
                selection: vec!["anything".into()]
            }),
            Reply::Error(_)
        ));
        assert!(matches!(
            first.call(Request::Collect {
                checkin_id: receipt.checkin_id.clone(),
                pickup_id: "box".into()
            }),
            Reply::Error(_)
        ));
        assert!(matches!(
            first.call(Request::CancelFlow(CancelTarget {
                attempt: 1,
                checkin_id: Some(receipt.checkin_id),
                transfer_id: None,
                abandon_checkin: false
            })),
            Reply::Error(_)
        ));
        assert_eq!(
            std::fs::read_to_string(f.root.join("calls.json")).unwrap(),
            "[\"archiv\"]"
        );
        let second = f.adapter("two");
        let reply = second.call(Request::Checkin {
            attempt: 2,
            method: CheckinMethod::Guest("test-invitation-two".into()),
        });
        let Reply::Flow(FlowReply::Queue { id, position, .. }) = reply else {
            panic!("{reply:?}")
        };
        assert_eq!(position, 1);
        assert!(matches!(
            second.call(Request::Wait(WaitTarget::Queue { attempt: 2, id })),
            Reply::Flow(FlowReply::Queue { position: 1, .. })
        ));
        assert!(matches!(
            second.call(Request::CancelFlow(CancelTarget {
                attempt: 2,
                checkin_id: None,
                transfer_id: None,
                abandon_checkin: true
            })),
            Reply::Error(_)
        ));
        assert!(second.state.lock().unwrap().cancelled);
        assert!(matches!(
            second.call(Request::Checkin {
                attempt: 3,
                method: CheckinMethod::Guest("reuse".into())
            }),
            Reply::Error(_)
        ));
    }
    #[test]
    fn cancel_arriving_during_slot_assignment_requests_checkout_before_login() {
        let Some(f) = Fixture::start() else {
            return;
        };
        std::fs::write(f.root.join("delay"), b"delay").unwrap();
        let adapter = Arc::new(f.adapter("race"));
        let worker = adapter.clone();
        let checkin = std::thread::spawn(move || {
            worker.call(Request::Checkin {
                attempt: 1,
                method: CheckinMethod::Guest("test-invitation-one".into()),
            })
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !f.root.join("registering").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "Check-in never reached assignment"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let worker = adapter.clone();
        let cancel = std::thread::spawn(move || {
            worker.call(Request::CancelFlow(CancelTarget {
                attempt: 1,
                checkin_id: None,
                transfer_id: None,
                abandon_checkin: true,
            }))
        });
        assert!(matches!(checkin.join().unwrap(), Reply::Error(_)));
        assert!(matches!(cancel.join().unwrap(), Reply::Error(_)));
        let state = adapter.state.lock().unwrap();
        assert!(state.hub.is_none(), "No slot login after cancellation");
        assert!(state.checkout.is_some(), "N6 checkout requested");
        assert!(
            state.receipt.is_some(),
            "No fake release without server confirmation"
        );
        assert!(std::fs::read_to_string(f.root.join("calls.json"))
            .unwrap()
            .contains("archiv"));
    }
    #[test]
    fn ambiguous_checkin_never_retries_or_claims_cancelled() {
        let Some(f) = Fixture::start() else {
            return;
        };
        let adapter = f.adapter("uncertain");
        assert!(matches!(
            adapter.call(Request::Checkin {
                attempt: 1,
                method: CheckinMethod::Guest("invalid-code".into())
            }),
            Reply::Error(_)
        ));
        // Fehlertext wird nicht als Beweis interpretiert, dass kein Slot vergeben wurde.
        assert!(adapter.state.lock().unwrap().checkin_pending);
        assert!(matches!(
            adapter.call(Request::Checkin {
                attempt: 2,
                method: CheckinMethod::Guest("test-invitation-one".into())
            }),
            Reply::Error(_)
        ));
        assert!(matches!(
            adapter.call(Request::CancelFlow(CancelTarget {
                attempt: 1,
                checkin_id: None,
                transfer_id: None,
                abandon_checkin: true
            })),
            Reply::Error(_)
        ));
        assert!(adapter.state.lock().unwrap().receipt.is_none());
    }
    #[test]
    fn local_fake_oidc_proof_is_verified_by_real_empfang() {
        let Some(f) = Fixture::start() else {
            return;
        };
        let adapter = f.adapter("oidc");
        let mut state = adapter.state.lock().unwrap();
        state.attempt = 1;
        let reply = adapter
            .checkin(&mut state, f.info["proof"].clone())
            .unwrap();
        assert!(matches!(reply, FlowReply::CheckedIn(_)));
        assert_eq!(state.receipt.as_ref().unwrap().account.person, "Testperson");
    }
}

/// Strikte HTTPS-Originbindung; kein Prefixvergleich mit täuschenden Hostnamen.
pub(super) fn validate_browser_url(issuer: &str, url: &str) -> Result<(), String> {
    fn authority(url: &str) -> Option<&str> {
        let rest = url.strip_prefix("https://")?;
        let host = rest.split(['/', '?']).next()?;
        if host.is_empty()
            || host.contains(['@', '\\', '#'])
            || url.chars().any(|c| c.is_whitespace() || c.is_control())
            || url.contains(['\\', '#'])
        {
            None
        } else {
            Some(host)
        }
    }
    if authority(issuer)
        .zip(authority(url))
        .is_some_and(|(a, b)| a == b)
    {
        Ok(())
    } else {
        Err("Der Browserlink gehört nicht zum konfigurierten HTTPS-Anmeldedienst.".into())
    }
}
