//! Empfang in App-Workern. Fehlende Sync-HTTP-APIs werden nicht umgangen.
//! Ohne bestätigten Empfangsschlüssel und eigenen Archivschlüssel kein Check-in.
use super::{
    abholfach_flow, checkin::*, hub::HubAdapter, sync_flow, Account, DesktopApi, InviteError,
    Reply, Request,
};
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

/// N8: reine Zeitsteuerung, monotone Uhr wird vom Desktop bzw. Test eingespeist.
/// 30 s lassen Reserve für Worker und langsame Frames gegenüber der 60-s-Grenze.
#[derive(Default)]
pub(super) struct AktivTakt {
    last: Option<Duration>,
    connected: bool,
}
impl AktivTakt {
    pub fn next(&mut self, now: Duration, connected: bool) -> Option<bool> {
        let due = connected
            && self
                .last
                .is_none_or(|last| now.saturating_sub(last) >= Duration::from_secs(30));
        if due || self.connected != connected {
            self.connected = connected;
            self.last = Some(now);
            Some(connected)
        } else {
            None
        }
    }
}
type AktivSignal = Arc<dyn Fn(bool) -> Result<(), String> + Send + Sync>;

/// Nur die bereits geprüfte Belegung; keine Transfer-Sperre und kein Hub-Bearer.
struct EmpfangZugang {
    origin: String,
    key_dir: PathBuf,
    pin: String,
    result: Value,
}
impl EmpfangZugang {
    fn oeffnen(&self) -> Result<Empfang, String> {
        let mut empfang = Empfang::new(&self.origin)?;
        let mut key = DeviceKey::at(&self.key_dir, "seminar-desktop")?;
        empfang.uebernehmen(&self.result, &mut key, &self.pin, now_ms())?;
        Ok(empfang)
    }
}
type HintergrundZugang = Arc<Mutex<Option<EmpfangZugang>>>;
fn empfang_oeffnen(zugang: &HintergrundZugang) -> Result<Option<Empfang>, String> {
    // Schlüsselübernahme serialisieren, danach HTTP ohne irgendeine State-Sperre.
    zugang
        .lock()
        .map_err(|_| "Die Empfangsverbindung wurde unterbrochen.".to_owned())?
        .as_ref()
        .map(EmpfangZugang::oeffnen)
        .transpose()
}

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
    pub device_name: String,
    pub key_dir: PathBuf,
    pub sha256: Option<Sha256>,
    pub local_root: PathBuf,
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
    receipt_unconfirmed: bool,
    imported: Option<ReturnSummary>,
    cancelled: bool,
    pickup_imported: Option<(String, String, ReturnSummary)>,
    pickup_unconfirmed: bool,
    pickup: Option<Pickup>,
    checkin_pending: bool,
    plan: Option<sync_flow::Plan>,
    preview: Option<(String, Value)>,
    ausweis: Option<Value>,
    seminar_hub: Option<pi_backend::hub_v1::HubV1>,
}
pub struct FlowAdapter {
    config: Config,
    state: Mutex<State>,
    cancelled_attempt: AtomicU64,
    current_attempt: AtomicU64,
    transfer: sync_flow::Transfer,
    archive_key: pi_backend::hub_v1::abholfach::Schluessel,
    identity: pi_net::identity::Identity,
    aktiv_signal: Mutex<Option<AktivSignal>>,
    hintergrund: HintergrundZugang,
    signal_gate: Arc<Mutex<()>>,
    eigene_adressen: Arc<Mutex<Vec<String>>>,
}
impl FlowAdapter {
    pub fn new(config: Config) -> Result<Self, String> {
        if config.empfang_pubkey.is_empty() {
            return Err(
                "Bestätige zuerst den Empfangsschlüssel. Es wurde kein Platz vergeben.".into(),
            );
        }
        let archive_key =
            pi_backend::hub_v1::abholfach::Schluessel::laden_oder_erzeugen(&config.key_dir)?;
        let empfang = Empfang::new(&config.origin)?;
        let key = DeviceKey::at(&config.key_dir, "seminar-desktop")?;
        let identity =
            pi_net::identity::Identity::load_or_create(&config.key_dir.join("seminar-net.key"))?;
        let hintergrund = Arc::new(Mutex::new(None));
        let signal_gate = Arc::new(Mutex::new(()));
        let eigene_adressen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let adressen_signal = eigene_adressen.clone();
        let adapter = Self {
            eigene_adressen,
            config,
            archive_key,
            identity,
            hintergrund: hintergrund.clone(),
            signal_gate: signal_gate.clone(),
            aktiv_signal: Mutex::new(None),
            transfer: sync_flow::Transfer::default(),
            cancelled_attempt: AtomicU64::new(0),
            current_attempt: AtomicU64::new(0),
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
                receipt_unconfirmed: false,
                imported: None,
                cancelled: false,
                pickup_imported: None,
                pickup_unconfirmed: false,
                pickup: None,
                checkin_pending: false,
                plan: None,
                preview: None,
                ausweis: None,
                seminar_hub: None,
            }),
        };
        adapter.set_aktiv_signal(move |connected| {
            // Ordnungsbarriere gegen einen verspäteten true-Puls beim Check-out.
            let _signal = signal_gate
                .lock()
                .map_err(|_| "Das Aktiv-Signal wurde unterbrochen.")?;
            if let Some(empfang) = empfang_oeffnen(&hintergrund)? {
                if connected {
                    let adressen = adressen_signal
                        .lock()
                        .map(|a| a.clone())
                        .unwrap_or_default();
                    empfang.aktiv_mit_adressen(true, &adressen)
                } else {
                    empfang.aktiv(false)
                }
            } else if connected {
                Err("Das Aktiv-Signal wartet auf den bestätigten Check-in.".into())
            } else {
                Ok(())
            }
        });
        Ok(adapter)
    }
    /// Authentifizierter, kurzlebiger Empfangszugang, unabhängig vom Transfer-State.
    /// Der Callback muss unabhängig vom Transfer-State bleiben: Uploads dürfen
    /// Aktiv-Signale nicht hinter dem langen State-Mutex blockieren.
    pub fn set_aktiv_signal(
        &self,
        signal: impl Fn(bool) -> Result<(), String> + Send + Sync + 'static,
    ) {
        *self.aktiv_signal.lock().unwrap() = Some(Arc::new(signal));
    }
    fn empfang_beenden(&self) -> Result<(), String> {
        let _signal = self
            .signal_gate
            .lock()
            .map_err(|_| "Das Aktiv-Signal wurde unterbrochen.")?;
        // Erst lokal schließen: auch bei HTTP-Fehler darf kein verspäteter true-Puls folgen.
        let zugang = self
            .hintergrund
            .lock()
            .map_err(|_| "Die Empfangsverbindung wurde unterbrochen.")?
            .take();
        if let Some(zugang) = zugang {
            zugang.oeffnen()?.aktiv(false)?;
        }
        Ok(())
    }
    fn login_begin(&self, s: &State, hello: &Value) -> Result<oidc::Login, String> {
        let hash = self.config.sha256.ok_or("Der geprüfte SHA-256-Callback ist in dieser App noch nicht angebunden. Keine Browseranmeldung gestartet.")?;
        let discovery = oidc::Discovery::load(&self.config.issuer)?;
        let nonce = s.empfang.nonce(&s.key.public_key())?;
        oidc::Login::begin_with_nonce(
            discovery,
            &field(&hello["oidc"], "client_id")?,
            &["openid", "profile", "groups"],
            &nonce,
            hash,
        )
    }
    fn checkin(&self, s: &mut State, proof: Value) -> Result<FlowReply, String> {
        let mut body = json!({
            "nachweis":proof,
            "geraet":{"pubkey":s.key.public_key(),"name":self.config.device_name,"art":"desktop"},
            "archiv_pubkey":self.archive_key.public_b64url,
        });
        body["net_pubkey"] = json!(self.identity.peer().0);
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
        *self
            .hintergrund
            .lock()
            .map_err(|_| "Die Empfangsverbindung wurde unterbrochen.")? = Some(EmpfangZugang {
            origin: self.config.origin.clone(),
            key_dir: self.config.key_dir.clone(),
            pin: self.config.empfang_pubkey.clone(),
            result: result.clone(),
        });
        s.ausweis = Some(result["geraeteausweis"].clone());
        let id = field(&result, "checkin_id")?;
        let slot = field(&result, "slot")?;
        let account = Account {
            person: field(&result, "person")?,
            slot: slot.clone(),
            device_id: s.key.geraet_id.clone().ok_or("Die Geräte-ID fehlt.")?,
            device_name: self.config.device_name.clone(),
        };
        let mut receipt = Receipt {
            checkin_id: id,
            account,
            since: "jetzt".into(),
            pickup: None,
        };
        s.receipt = Some(receipt.clone());
        s.checkout = None;
        s.receipt_unconfirmed = false;
        s.imported = None;
        s.plan = None;
        s.preview = None;
        s.checkin_pending = false;
        s.queue = None;
        if s.cancelled || self.cancelled_attempt.load(Ordering::SeqCst) == s.attempt {
            // N6: Rückgabe vor Slot-Login, kein Upload nach Vergabe.
            self.empfang_beenden()?;
            s.checkout = Some(s.empfang.checkout(&receipt.checkin_id)?);
            if s.checkout
                .as_ref()
                .is_some_and(|v| v["typ"] == "ausgecheckt")
            {
                *self.hintergrund.lock().unwrap() = None;
                s.receipt = None;
                return Ok(FlowReply::Cancelled);
            }
            return Err("Der Platz wurde während des Abbruchs vergeben. Die sichere Rückgabe wurde ohne Upload angefordert; der Empfang hat noch keine archivfreie Freigabe bestätigt.".into());
        }
        // Empfangsignatur und Gerätebindung sind vor Slot-Login geprüft.
        // Erst nach geprüftem Check-in ist der Empfang mit dem Geräteausweis authentifiziert.
        // Ein nicht abrufbares Abholfach darf den Check-in nie verhindern (N-4).
        receipt.pickup = abholfach_flow::angebot(&s.empfang).unwrap_or(None);
        s.pickup = receipt.pickup.clone();
        s.receipt = Some(receipt.clone());
        let mut hub = pi_backend::hub_v1::HubV1::new(&format!(
            "{}/slot/{slot}",
            self.config.origin.trim_end_matches('/')
        ))?;
        hub.login(&s.key)?;
        let mut seminar_hub = pi_backend::hub_v1::HubV1::new(&format!(
            "{}/slot/{slot}",
            self.config.origin.trim_end_matches('/')
        ))?;
        seminar_hub.login(&s.key)?;
        s.seminar_hub = Some(seminar_hub);
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
            if !target.abandon_checkin {
                if target.attempt == 0
                    || target.attempt != self.current_attempt.load(Ordering::SeqCst)
                {
                    return Err("Dieser Abbruch gehört zu keinem aktuellen Versuch.".into());
                }
                self.transfer.cancel.store(true, Ordering::SeqCst);
                // Niemals nach dem Ende eines Transfers als Check-out interpretieren.
                // Ein bereits autoritativ bestätigtes CheckedOut darf trotzdem folgen.
                return Ok(Reply::Flow(FlowReply::Cancelled));
            }
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
                self.current_attempt.store(attempt, Ordering::SeqCst);
                self.transfer.cancel.store(false, Ordering::SeqCst);
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
                        let login = self.login_begin(&s, &hello)?;
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
            #[cfg(test)]
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
                    self.empfang_beenden()?;
                    s.checkout = Some(s.empfang.checkout(&id)?);
                    if s.checkout
                        .as_ref()
                        .is_some_and(|v| v["typ"] == "ausgecheckt")
                    {
                        *self.hintergrund.lock().unwrap() = None;
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
                let plan = sync_flow::Plan::new(&self.config.local_root)?;
                self.transfer.cancel.store(false, Ordering::SeqCst);
                let reply = plan.ui();
                s.plan = Some(plan);
                FlowReply::Plan(reply)
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
                let plan = s
                    .plan
                    .as_ref()
                    .ok_or("Prüfe zuerst die lokale Dateiauswahl.")?;
                let hub = s.hub.as_ref().ok_or("Die Slotverbindung fehlt.")?;
                let _transfer = self.transfer.scope(TransferKind::Upload, &checkin_id, 0);
                let result =
                    hub.with_hub(|h| plan.upload(h, &checkin_id, &selection, &self.transfer));
                self.transfer.end();
                result?;
                FlowReply::Uploaded
            }
            Request::CheckoutPreview(id) => {
                require_receipt(&s, &id)?;
                let hub = s.hub.as_ref().ok_or("Die Slotverbindung fehlt.")?;
                let value = hub.with_hub(|h| h.sync_http(sync_flow::hash).vorschau())?;
                let preview_id = format!("vorschau-{}", now_ms());
                let preview = sync_flow::preview(&value, preview_id.clone())?;
                s.preview = Some((preview_id, value));
                self.transfer.cancel.store(false, Ordering::SeqCst);
                FlowReply::Preview(preview)
            }
            Request::Checkout {
                checkin_id,
                preview_id,
                large_files,
            } => {
                require_receipt(&s, &checkin_id)?;
                if s.receipt_unconfirmed {
                    return Err("Die Empfangsbestätigung ist unklar. Prüfe den Empfang; erhalten wird nicht erneut gesendet und das Archiv nicht erneut importiert.".into());
                }
                if preview_id.is_empty() {
                    return Err("Die bestätigte Rückgabevorschau fehlt.".into());
                }
                let (current, preview) = s
                    .preview
                    .as_ref()
                    .ok_or("Prüfe zuerst die Rückgabevorschau.")?;
                if current != &preview_id {
                    return Err("Diese Rückgabevorschau ist veraltet.".into());
                }
                let allowed: Vec<&str> = preview["gross"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|f| f["pfad"].as_str())
                    .collect();
                if large_files.iter().any(|p| !allowed.contains(&p.as_str())) {
                    return Err(
                        "Diese Großdateiauswahl gehört nicht zur bestätigten Vorschau.".into(),
                    );
                }
                // Abgewählte persönliche Großdateien niemals durch Check-out verlieren.
                // Erst mit allen Einzelzustimmungen darf der Empfang leeren.
                if allowed.iter().any(|p| !large_files.iter().any(|s| s == p)) {
                    return Err("Bestätige jede große Datei einzeln, bevor der Arbeitsplatz geleert wird. Es wurde nichts gelöscht.".into());
                }
                let _transfer = self.transfer.scope(TransferKind::Checkout, &checkin_id, 0);
                self.transfer.check()?;
                // Vor der Freigabe melden, solange die Geräte-Challenge noch gilt.
                self.empfang_beenden()?;
                // Empfang baut das autoritative Archiv; kein zweites, abweichendes Clientarchiv.
                let meta = match &s.checkout {
                    Some(v) => v.clone(),
                    None => s.empfang.checkout_mit_auswahl(
                        &checkin_id,
                        json!({"zusaetzlich":large_files,"ausschliessen":[]}),
                    )?,
                };
                s.checkout = Some(meta.clone());
                if meta["typ"] == "ausgecheckt" {
                    self.transfer.end();
                    let folder = pi_sync::LocalFolder::create(&self.config.local_root)
                        .map_err(|e| e.to_string())?;
                    sync_flow::journal(
                        &folder,
                        json!({"ereignis":"ausgecheckt","checkin_id":checkin_id,"direkt":true}),
                    )?;
                    *self.hintergrund.lock().unwrap() = None;
                    s.receipt = None;
                    s.hub = None;
                    s.seminar_hub = None;
                    s.preview = None;
                    return Ok(Reply::Flow(FlowReply::CheckedOut(ReturnSummary {
                        files: 0,
                        bytes: 0,
                        conflicts: 0,
                    })));
                }
                let folder = pi_sync::LocalFolder::create(&self.config.local_root)
                    .map_err(|e| e.to_string())?;
                let hub = s.hub.as_ref().ok_or("Die Slotverbindung fehlt.")?;
                let summary = match &s.imported {
                    Some(summary) => summary.clone(),
                    None => {
                        let imported = hub
                            .with_hub(|h| sync_flow::receive(&folder, h, &meta, &self.transfer))?;
                        s.imported = Some(imported.clone());
                        imported
                    }
                };
                self.transfer.check()?;
                // Keine Wiederholung nach verlorener Antwort: serverseitig fehlt ein Tombstone.
                self.empfang_beenden()?;
                s.receipt_unconfirmed = true;
                let confirmed = s
                    .empfang
                    .erhalten(&field(&meta, "checkout_id")?, &field(&meta, "sha256")?);
                if !confirmed.is_ok_and(|v| v["typ"] == "ausgecheckt") {
                    // Kein zweites erhalten, kein erneuter Import, kein falscher Erfolg.
                    return Ok(Reply::Flow(FlowReply::ClearUnconfirmed(summary)));
                }
                sync_flow::journal(
                    &folder,
                    json!({"ereignis":"ausgecheckt","checkin_id":checkin_id,"sha256":meta["sha256"]}),
                )?;
                *self.hintergrund.lock().unwrap() = None;
                s.receipt = None;
                s.hub = None;
                s.seminar_hub = None;
                s.preview = None;
                FlowReply::CheckedOut(summary)
            }
            Request::Collect {
                checkin_id,
                pickup_id,
            } => {
                require_receipt(&s, &checkin_id)?;
                if s.pickup.as_ref().is_none_or(|p| p.id != pickup_id) {
                    return Err("Dieses Abholfach gehört nicht zum geprüften Angebot.".into());
                }
                if s.pickup_unconfirmed {
                    return Err("Die Abholbestätigung ist unklar. Die Dateien sind lokal; prüfe den Empfang. Keine erneute Bestätigung oder doppelter Import.".into());
                }
                self.transfer.cancel.store(false, Ordering::SeqCst);
                let _scope = self.transfer.scope(TransferKind::Pickup, &pickup_id, 0);
                let folder = pi_sync::LocalFolder::create(&self.config.local_root)
                    .map_err(|e| e.to_string())?;
                let (sha, summary) = match &s.pickup_imported {
                    Some((id, sha, summary)) if id == &pickup_id => (sha.clone(), summary.clone()),
                    _ => {
                        let (sha, summary) = abholfach_flow::holen(
                            &s.empfang,
                            &self.archive_key,
                            &folder,
                            &pickup_id,
                            &self.transfer,
                        )?;
                        s.pickup_imported = Some((pickup_id.clone(), sha.clone(), summary.clone()));
                        (sha, summary)
                    }
                };
                self.transfer.check()?;
                s.pickup_unconfirmed = true;
                let ack = s.empfang.abholfach_erhalten(&pickup_id, &sha)?;
                if ack["typ"] != "abholfach_erhalten" {
                    return Err("Der Empfang hat die Abholung nicht bestätigt. Deine Dateien sind bereits lokal.".into());
                }
                s.pickup = None;
                if let Some(receipt) = &mut s.receipt {
                    receipt.pickup = None;
                }
                s.pickup_unconfirmed = false;
                FlowReply::Collected(summary)
            }
            Request::Wait(WaitTarget::Transfer { checkin_id, .. }) => {
                require_receipt(&s, &checkin_id)?;
                return Err("Es ist kein bestätigter Transfer aktiv.".into());
            }
            other => {
                let hub = s.hub.clone().ok_or("Du bist noch nicht eingecheckt.")?;
                drop(s);
                let reply = hub.call(other);
                // Auf dem Worker anmelden, nie auf dem UI-Thread. Neuer expliziter
                // Sessionbestand bekommt eine frische, gleich gebundene Runtime.
                if matches!(&reply, Reply::Sessions(_) | Reply::Opened(_, _)) {
                    let mut s = self
                        .state
                        .lock()
                        .map_err(|_| "Die Seminarverbindung wurde unterbrochen.")?;
                    if s.seminar_hub.is_none() {
                        if let Some(receipt) = &s.receipt {
                            let mut h = pi_backend::hub_v1::HubV1::new(&format!(
                                "{}/slot/{}",
                                self.config.origin.trim_end_matches('/'),
                                receipt.account.slot
                            ))?;
                            h.login(&s.key)?;
                            s.seminar_hub = Some(h);
                        }
                    }
                }
                return Ok(reply);
            }
        };
        Ok(Reply::Flow(reply))
    }
}
impl DesktopApi for FlowAdapter {
    fn trennen(&self) -> Result<(), String> {
        self.empfang_beenden()
    }
    fn seminar_verzeichnis(
        &self,
    ) -> Result<Option<pi_backend::hub_v1::empfang::Verzeichnis>, String> {
        empfang_oeffnen(&self.hintergrund)?
            .map(|empfang| empfang.seminar_verzeichnis())
            .transpose()
    }
    fn set_eigene_adressen(&self, adressen: Vec<String>) {
        if let Ok(mut a) = self.eigene_adressen.lock() {
            *a = adressen.into_iter().take(8).collect();
        }
    }
    fn aktiv_signal(&self, connected: bool) -> Result<(), String> {
        let signal = self
            .aktiv_signal
            .lock()
            .map_err(|_| "Das Aktiv-Signal wurde unterbrochen.")?
            .clone();
        signal.ok_or("Der Empfang erhält noch kein Aktiv-Signal. Bitte die Seminarleitung um die aktualisierte Verbindung.")?(connected)
    }
    fn progress(&self) -> Option<Progress> {
        self.transfer.progress()
    }
    fn seminar(
        &self,
        sessions: &[super::Session],
    ) -> Result<Option<super::seminar::Seminar>, String> {
        let mut s = self
            .state
            .try_lock()
            .map_err(|_| "Die Seminaranmeldung wird noch vorbereitet.")?;
        let receipt = s.receipt.as_ref().ok_or("Du bist nicht eingecheckt.")?;
        let ausweis = serde_json::from_value(s.ausweis.clone().ok_or("Der Geräteausweis fehlt.")?)
            .map_err(|e| format!("Der Geräteausweis ist ungültig: {e}"))?;
        let identity = pi_net::identity::Identity {
            secret: self.identity.secret.clone(),
        };
        let state = super::seminar::SeminarState::new(
            identity.peer(),
            &receipt.account.person,
            self.config.empfang_pubkey.clone(),
            pi_net::model::SpaceId("seminar".into()),
        );
        let hub = s
            .seminar_hub
            .take()
            .ok_or("Die Seminarverbindung wird noch vorbereitet.")?;
        let own = sessions
            .iter()
            .filter(|s| !s.local)
            .map(|s| {
                (
                    pi_net::remote::SessionRef {
                        label: s.title.clone(),
                        cwd: s.id.clone(),
                    },
                    s.id.clone(),
                )
            })
            .collect();
        super::seminar::production(state, identity, ausweis, hub, own).map(Some)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn aktiv_signal_fake_clock_connection_period_disconnect_and_reconnect() {
        let mut takt = AktivTakt::default();
        let mut signals = vec![];
        let mut tick = |seconds, connected| {
            if let Some(signal) = takt.next(Duration::from_secs(seconds), connected) {
                signals.push((seconds, signal));
            }
        };
        tick(0, false);
        tick(1, true);
        for seconds in 2..=121 {
            tick(seconds, true);
        }
        tick(122, false);
        tick(500, false);
        tick(501, true);
        assert_eq!(
            signals,
            vec![
                (1, true),
                (31, true),
                (61, true),
                (91, true),
                (121, true),
                (122, false),
                (501, true)
            ]
        );
        assert!(signals
            .windows(2)
            .filter(|pair| pair[0].1 && pair[1].1)
            .all(|pair| pair[1].0 - pair[0].0 <= 60));
    }

    #[test]
    fn aktiv_callback_runs_without_transfer_state_lock() {
        let f = Fixture::start().unwrap();
        let adapter = f.adapter("aktiv");
        let signals = Arc::new(Mutex::new(vec![]));
        let output = signals.clone();
        adapter.set_aktiv_signal(move |connected| {
            output.lock().unwrap().push(connected);
            Ok(())
        });
        let _transfer_lock = adapter.state.lock().unwrap();
        adapter.aktiv_signal(true).unwrap();
        adapter.aktiv_signal(false).unwrap();
        assert_eq!(*signals.lock().unwrap(), [true, false]);
    }

    #[test]
    fn login_begin_uses_real_empfang_nonce_bound_to_own_device() {
        let f = Fixture::start().expect("Node-Fixture erforderlich");
        let mut adapter = f.adapter("server-nonce");
        adapter.config.issuer = format!("{}/oidc", f.info["origin"].as_str().unwrap());
        let state = adapter.state.lock().unwrap();
        let hello = state.empfang.hallo().unwrap();
        let login = adapter.login_begin(&state, &hello).unwrap();
        let issued: Value =
            serde_json::from_slice(&std::fs::read(f.root.join("server-nonce.json")).unwrap())
                .unwrap();
        assert_eq!(issued["geraet_pubkey"], state.key.public_key());
        login
            .open_browser(|url| {
                assert!(url
                    .split('?')
                    .nth(1)
                    .unwrap()
                    .split('&')
                    .any(|p| p == format!("nonce={}", issued["nonce"].as_str().unwrap())));
                assert!(url.contains("code_challenge_method=S256"));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn node_empfang_activity_and_directory_reach_desktop_without_transfer_lock() {
        let f = Fixture::start().expect("Node-Fixture erforderlich");
        let adapter = Arc::new(f.adapter("presence"));
        let Reply::Flow(FlowReply::CheckedIn(receipt)) = adapter.call(Request::Checkin {
            attempt: 1,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        }) else {
            panic!("Check-in fehlt")
        };
        // Derselbe Mutex wie beim langen Upload bleibt absichtlich gehalten.
        let transfer = adapter.state.lock().unwrap();
        let mut desktop = super::super::Desktop::new(true);
        desktop.demo = false;
        desktop.flow.receipt = Some(receipt.clone());
        desktop.connected = true;
        desktop.set_api(adapter.clone());
        // Andere lokale Identität, damit der echte Empfangseintrag nicht als Du entfällt.
        let viewer =
            pi_net::identity::Identity::load_or_create(&f.root.join("viewer.key")).unwrap();
        desktop.seminar = Some(super::super::seminar::Seminar::new(
            super::super::seminar::SeminarState::new(
                viewer.peer(),
                "Verzeichnis-Test",
                f.info["pubkey"].as_str().unwrap().into(),
                pi_net::model::SpaceId("seminar".into()),
            ),
            Box::new(super::super::seminar::MemoryNet::default()),
        ));
        desktop.empfang_tick();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while desktop.aktiv_pending || desktop.verzeichnis_pending {
            let reply = desktop
                .rx
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap();
            desktop.apply(reply);
        }
        let pool = || -> Value {
            serde_json::from_slice(&std::fs::read(f.root.join("empfang/pool.json")).unwrap())
                .unwrap()
        };
        assert_eq!(pool()["belegt"][0]["verbunden"], true);
        let seminar = desktop.seminar.as_ref().unwrap();
        assert!(seminar.error.is_none(), "{:?}", seminar.error);
        assert!(seminar
            .state
            .people
            .iter()
            .any(|p| p.peer == adapter.identity.peer()));
        assert!(seminar
            .state
            .chats
            .contacts
            .iter()
            .any(|p| p.peer == adapter.identity.peer()));
        // Zweiter Puls mit logischer Zeit, aber echter authentifizierter HTTP-Anfrage.
        desktop.aktiv_clock = std::time::Instant::now() - Duration::from_secs(61);
        desktop.empfang_tick();
        while desktop.aktiv_pending {
            let reply = desktop.rx.recv_timeout(Duration::from_secs(5)).unwrap();
            desktop.apply(reply);
        }
        assert_eq!(
            std::fs::read_to_string(f.root.join("aktiv-anfragen"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        // Falls das Verzeichnis vor der Seminar-Runtime eintrifft, bleibt es anwendbar.
        let seminar = desktop.seminar.take().unwrap();
        desktop.verzeichnis_anwenden();
        desktop.seminar = Some(seminar);
        desktop.verzeichnis_anwenden();
        desktop.verzeichnis_zuletzt = Some(std::time::Instant::now() - Duration::from_secs(61));
        desktop.empfang_tick();
        while desktop.verzeichnis_pending {
            let reply = desktop.rx.recv_timeout(Duration::from_secs(5)).unwrap();
            desktop.apply(reply);
        }
        assert_eq!(desktop.seminar.as_ref().unwrap().state.people.len(), 1);
        // Ein alter Worker darf nach Wechsel des Verbindungszyklus nichts anwenden.
        let old_epoch = desktop.connection_epoch;
        desktop.connection_epoch += 1;
        desktop.apply(Reply::ConnectionEvent {
            epoch: old_epoch,
            reply: Box::new(Reply::VerzeichnisCompleted(Err("alter Fehler".into()))),
        });
        assert!(desktop.error.is_none());
        // 60-s-Verzeichnisrefresh und Fehler bleiben nicht blockierend sichtbar.
        std::fs::write(f.root.join("directory-error"), b"yes").unwrap();
        desktop.verzeichnis_zuletzt = Some(std::time::Instant::now() - Duration::from_secs(61));
        desktop.empfang_tick();
        while desktop.verzeichnis_pending {
            let reply = desktop.rx.recv_timeout(Duration::from_secs(5)).unwrap();
            desktop.apply(reply);
        }
        assert!(desktop
            .error
            .as_ref()
            .unwrap()
            .contains("Du kannst weiterarbeiten"));
        assert!(desktop
            .seminar
            .as_ref()
            .unwrap()
            .state
            .people
            .iter()
            .any(|p| p.peer == adapter.identity.peer()));
        desktop.connected = false;
        desktop.empfang_tick();
        while desktop.aktiv_pending {
            let reply = desktop.rx.recv_timeout(Duration::from_secs(5)).unwrap();
            desktop.apply(reply);
        }
        assert_eq!(pool()["belegt"][0]["verbunden"], false);
        adapter.aktiv_signal(true).unwrap();
        assert_eq!(pool()["belegt"][0]["verbunden"], true);
        // Fensterende sendet false und sperrt einen verspäteten true-Puls.
        drop(desktop);
        assert_eq!(pool()["belegt"][0]["verbunden"], false);
        assert!(adapter.aktiv_signal(true).is_err());
        assert_eq!(pool()["belegt"][0]["verbunden"], false);
        drop(transfer);
    }

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
            Self::start_mode(false)
        }
        fn start_mode(real_sync: bool) -> Option<Self> {
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
const [source,root,mode]=process.argv.slice(1);
const {createEmpfang}=await import(pathToFileURL(path.join(source,'empfang/empfang.mjs')));
const {Oidc}=await import(pathToFileURL(path.join(source,'empfang/oidc.mjs')));
const {createFakeOidc}=await import(pathToFileURL(path.join(source,'empfang/test/fake-oidc.mjs')));
const {createV1}=await import(pathToFileURL(path.join(source,'lib/hub-v1.mjs')));
const fake=await createFakeOidc(); let reception,hub,origin;
const server=http.createServer((req,res)=>{
  if(req.url==='/oidc/.well-known/openid-configuration') {
    res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({issuer:origin+'/oidc',authorization_endpoint:origin+'/oidc/authorize',token_endpoint:origin+'/oidc/token',code_challenge_methods_supported:['S256']}));return;
  }
  if(req.url==='/v1/empfang/nonce') {
    (async()=>{let body='';for await(const c of req)body+=c;const input=JSON.parse(body);const value=reception.issueNonce(input.geraet_pubkey);fs.writeFileSync(path.join(root,'server-nonce.json'),JSON.stringify({...value,geraet_pubkey:input.geraet_pubkey}));res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify(value));})();return;
  }
  if(req.url.startsWith('/fixture/proof/')) {
    const pubkey=decodeURIComponent(req.url.slice('/fixture/proof/'.length));
    (async()=>{const n=await (await fetch(origin+'/v1/empfang/nonce',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({protokoll:1,geraet_pubkey:pubkey})})).json();
    const proof=JSON.stringify({art:'oidc',id_token:fake.token({sub:'rust-adapter',jti:crypto.randomUUID(),nonce:n.nonce}),nonce:n.nonce});
    res.writeHead(200,{'content-type':'application/json','content-length':Buffer.byteLength(proof)});res.end(proof);})();return;
  }
  if(req.url==='/v1/empfang/aktiv')fs.appendFileSync(path.join(root,'aktiv-anfragen'),'1\n');
  if(req.url==='/v1/empfang/seminar'&&fs.existsSync(path.join(root,'directory-error'))) {res.writeHead(503,{'content-type':'application/json'});res.end(JSON.stringify({protokoll:1,typ:'fehler',code:'intern',was:'Testausfall',tun:'Später versuchen'}));return;}
  if(req.url.startsWith('/fixture/force/')) {
    reception.forceCheckout(req.url.split('/').at(-1)).then(value=>{res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify(value));}).catch(error=>{res.writeHead(500);res.end(String(error));});return;
  }
  if(req.url.endsWith('/erhalten')&&fs.existsSync(path.join(root,'lose-erhalten'))) {
    fs.appendFileSync(path.join(root,'ack-attempts'),'1\n');
    res.writeHead(500,{'content-type':'application/json'});res.end(JSON.stringify({protokoll:1,typ:'fehler',code:'intern',was:'Antwort unklar',tun:'Empfang prüfen'}));return;
  }
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
reception=createEmpfang({dir:path.join(root,'empfang'),origin,slots:{account01:mode==='sync'?origin+'/slot/account01':origin},epochenDir:path.join(root,'epochen'),
 oidc:new Oidc({issuer:fake.issuer,clientId:fake.clientId,allowHttp:true}),
 adapter:mode==='sync'?undefined:{register:async(_slot,belegung)=>{ if(fs.existsSync(path.join(root,'delay'))){fs.writeFileSync(path.join(root,'registering'),'yes');await new Promise(resolve=>setTimeout(resolve,200));} const response=await fetch(origin+'/slot/account01/v1/belegung',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({protokoll:1,belegung})}); if(!response.ok)throw Error('register'); },
 release:async()=>{throw Error('Freigabe ohne Import verboten');},
 archive:async()=>{calls.push('archiv');fs.writeFileSync(path.join(root,'calls.json'),JSON.stringify(calls));return {archiv_id:crypto.randomUUID(),sha256:'a'.repeat(64),bytes:0};}},
 clearSlot:async(_slot,action)=>{
   if(mode!=='sync')throw Error('Echtes Leeren verboten');
   const local=path.join(root,'local-e2e/Projekte/Buch');
   if(!fs.existsSync(path.join(root,'empty-checkout'))) {
     const conflicts=fs.readdirSync(local).filter(n=>/^text \(R2 \d\d\.\d\d\. \d\d-\d\d\)\.md$/.test(n));
     if(conflicts.length!==1||fs.readFileSync(path.join(local,'text.md'),'utf8')!=='lokal unverändert')throw Error('Import nicht nachgewiesen');
     if(fs.readFileSync(path.join(local,conflicts[0]),'utf8')!=='R2 geändert')throw Error('Konfliktfassung fehlt');
   }
   calls.push(action);fs.writeFileSync(path.join(root,'calls.json'),JSON.stringify(calls));
   if(action==='leeren')for(const p of [work,path.join(root,'home/.pi')]){fs.rmSync(p,{recursive:true,force:true});fs.mkdirSync(p,{recursive:true});}
   if(action==='pruefen')fs.writeFileSync(path.join(root,'pool-frei'),'bestätigt');
   if(action==='leeren') {
     fs.writeFileSync(path.join(root,'pool-frei'),'bestätigt');
     const epoch=Number(fs.readFileSync(path.join(root,'epochen/account01.epoche'),'utf8'))+1;
     fs.writeFileSync(path.join(root,'epochen/account01.epoche'),String(epoch)+'\n');
     return {zustand:'leer',nachweis:'vor_hubstart',slot:'account01',epoche:epoch};
   }
 }});
hub=createV1({dir:path.join(root,'hub'),slot:'account01',hubUrl:origin,roots:[work],inRoots:p=>p===work?p:null,
 empfangPubkey:reception.ausweis.pubkey,epochFile:path.join(root,'epochen/account01.epoche'),createInstance:()=>{throw Error('Modellstart verboten');}});
const codes=['test-invitation-one','test-invitation-two'];
fs.writeFileSync(path.join(root,'empfang/nachweise.json'),JSON.stringify({tokens:[],einladungen:codes.map(code=>({hash:crypto.createHash('sha256').update(code).digest('hex'),person:'Mira',bis:Date.now()+300000}))}),{mode:0o600});
const archive=crypto.generateKeyPairSync('x25519');
console.log(JSON.stringify({origin,issuer:fake.issuer,pubkey:reception.ausweis.pubkey,
 archiv_pubkey:archive.publicKey.export({format:'der',type:'spki'}).subarray(-32).toString('base64url'),
 proof2:{art:'oidc',id_token:fake.token({sub:'rust-adapter',jti:'second-login'}),nonce:fake.nonce},
 proof:{art:'oidc',id_token:fake.token({sub:'rust-adapter'}),nonce:fake.nonce}}));
"#;
            let child = Command::new("node")
                .args(["--input-type=module", "-e", script])
                .arg(source)
                .arg(&root)
                .arg(if real_sync { "sync" } else { "legacy" })
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
        fn proof(&self, adapter: &FlowAdapter) -> Value {
            use std::io::{Read, Write};
            let origin = self.info["origin"].as_str().unwrap();
            let key = adapter.state.lock().unwrap().key.public_key();
            let mut stream =
                std::net::TcpStream::connect(origin.strip_prefix("http://").unwrap()).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            write!(
                stream,
                "GET /fixture/proof/{key} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 200"), "{response}");
            // Node sendet hier chunked; explizit Content-Length in der Fixture setzen.
            serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap()
        }
        fn force(&self, id: &str) {
            use std::io::{Read, Write};
            let origin = self.info["origin"].as_str().unwrap();
            let mut stream =
                std::net::TcpStream::connect(origin.strip_prefix("http://").unwrap()).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(20)))
                .unwrap();
            write!(stream, "POST /fixture/force/{id} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        }
        fn adapter(&self, name: &str) -> FlowAdapter {
            FlowAdapter::new(Config {
                origin: field(&self.info, "origin").unwrap(),
                issuer: field(&self.info, "issuer").unwrap(),
                empfang_pubkey: field(&self.info, "pubkey").unwrap(),
                device_name: name.into(),
                key_dir: self.root.join(name),
                sha256: Some(sha256),
                local_root: self.root.join(format!("local-{name}")),
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
            Reply::Flow(FlowReply::Plan(_))
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
            Reply::Flow(FlowReply::Cancelled)
        ));
        assert!(
            !f.root.join("calls.json").exists(),
            "Transferabbruch darf keinen Archivauftrag senden"
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
    fn local_node_end_to_end_upload_conflict_import_then_release() {
        let f = Fixture::start_mode(true).expect("Node-Fixture erforderlich");
        let root = f.root.join("local-e2e");
        let folder = pi_sync::LocalFolder::create(&root).unwrap();
        let book = root.join("Projekte/Buch");
        std::fs::create_dir(&book).unwrap();
        for (name, content) in [
            ("text.md", "lokal unverändert"),
            ("skizze.svg", "<svg/>"),
            ("notiz.md", "Notiz"),
        ] {
            std::fs::write(book.join(name), content).unwrap();
        }
        let big = std::fs::File::create(book.join("film.mp4")).unwrap();
        big.set_len(201 * 1024 * 1024).unwrap();
        let mut adapter = f.adapter("e2e");
        adapter.config.local_root = root.clone();
        let reply = adapter.call(Request::Checkin {
            attempt: 1,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        });
        let Reply::Flow(FlowReply::CheckedIn(receipt)) = reply else {
            panic!("{reply:?}")
        };
        let Reply::Flow(FlowReply::Plan(plan)) =
            adapter.call(Request::SyncPlan(receipt.checkin_id.clone()))
        else {
            panic!("Plan fehlt")
        };
        assert!(plan
            .items
            .iter()
            .any(|i| i.name.contains("film.mp4") && !i.selected));
        assert!(
            plan.items.iter().all(|i| !i.selected),
            "Kein zuletzt gewähltes Projekt: nichts vorausgewählt"
        );
        let id = plan
            .items
            .iter()
            .find(|i| i.name == "Projekte / Buch")
            .unwrap()
            .id
            .clone();
        let uploaded = adapter.call(Request::Upload {
            checkin_id: receipt.checkin_id.clone(),
            selection: vec![id],
        });
        assert!(
            matches!(uploaded, Reply::Flow(FlowReply::Uploaded)),
            "{uploaded:?}"
        );
        let repeated = adapter.state.lock().unwrap().plan.as_ref().unwrap().ui();
        assert_eq!(repeated.items.iter().filter(|i| i.selected).count(), 1);
        assert!(repeated
            .items
            .iter()
            .any(|i| i.name == "Projekte / Buch" && i.selected));
        let remote = f.root.join("home/arbeit/Projekte/Buch");
        assert!(!remote.join("film.mp4").exists());
        assert_eq!(std::fs::read_dir(&remote).unwrap().count(), 3);
        std::fs::write(remote.join("text.md"), "R2 geändert").unwrap();
        let Reply::Flow(FlowReply::Preview(preview)) =
            adapter.call(Request::CheckoutPreview(receipt.checkin_id.clone()))
        else {
            panic!("Vorschau fehlt")
        };
        assert_eq!(
            preview.personal.len(),
            1,
            "Unveränderte Uploads kommen nicht zurück"
        );
        let reply = adapter.call(Request::Checkout {
            checkin_id: receipt.checkin_id.clone(),
            preview_id: preview.id,
            large_files: vec![],
        });
        let Reply::Flow(FlowReply::CheckedOut(summary)) = reply else {
            panic!("{reply:?}")
        };
        assert_eq!(summary.conflicts, 1);
        assert_eq!(
            std::fs::read_to_string(book.join("text.md")).unwrap(),
            "lokal unverändert"
        );
        assert!(
            f.root.join("pool-frei").exists(),
            "Leeren-Fixture prüft Import vor Freigabe"
        );
        assert!(adapter.state.lock().unwrap().receipt.is_none());
        let journal = std::fs::read_to_string(folder.root.join(".sync/protokoll.jsonl")).unwrap();
        let events: Vec<Value> = journal
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(
            events
                .iter()
                .map(|v| v["ereignis"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["hochgeladen", "importiert", "ausgecheckt"]
        );
        assert!(std::fs::read_dir(root.join(".sync/archiv"))
            .unwrap()
            .next()
            .is_some());
    }
    #[test]
    fn local_node_empty_checkout_handles_n6_without_archive() {
        let f = Fixture::start_mode(true).unwrap();
        std::fs::write(f.root.join("empty-checkout"), b"yes").unwrap();
        let adapter = f.adapter("empty");
        let Reply::Flow(FlowReply::CheckedIn(receipt)) = adapter.call(Request::Checkin {
            attempt: 1,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        }) else {
            panic!("Check-in fehlt")
        };
        adapter.aktiv_signal(true).unwrap();
        let Reply::Flow(FlowReply::Preview(p)) =
            adapter.call(Request::CheckoutPreview(receipt.checkin_id.clone()))
        else {
            panic!("Vorschau fehlt")
        };
        let reply = adapter.call(Request::Checkout {
            checkin_id: receipt.checkin_id,
            preview_id: p.id,
            large_files: vec![],
        });
        assert!(
            matches!(
                reply,
                Reply::Flow(FlowReply::CheckedOut(ReturnSummary { files: 0, .. }))
            ),
            "{reply:?}"
        );
        assert!(f.root.join("pool-frei").exists());
        assert_eq!(
            std::fs::read_to_string(f.root.join("aktiv-anfragen"))
                .unwrap()
                .lines()
                .count(),
            2,
            "true-Puls und false vor Check-out"
        );
        assert!(
            adapter.aktiv_signal(true).is_err(),
            "kein verspäteter Puls nach Freigabe"
        );
        assert!(adapter.state.lock().unwrap().checkout.as_ref().unwrap()["typ"] == "ausgecheckt");
    }
    #[test]
    fn cancelled_transfer_never_requests_checkout_even_after_stream_ended() {
        let f = Fixture::start_mode(true).unwrap();
        let adapter = f.adapter("cancel");
        let Reply::Flow(FlowReply::CheckedIn(r)) = adapter.call(Request::Checkin {
            attempt: 7,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        }) else {
            panic!("Check-in fehlt")
        };
        let guard = adapter.state.lock().unwrap();
        // Kein Warten auf denselben Mutex und kein stiller Check-out nach dem Transfer.
        assert!(matches!(
            adapter.call(Request::CancelFlow(CancelTarget {
                attempt: 7,
                checkin_id: Some(r.checkin_id),
                transfer_id: None,
                abandon_checkin: false
            })),
            Reply::Flow(FlowReply::Cancelled)
        ));
        assert!(adapter.transfer.check().is_err());
        assert!(guard.checkout.is_none());
        assert!(!f.root.join("pool-frei").exists());
        assert!(adapter.transfer.progress().is_none());
    }
    #[cfg(unix)]
    #[test]
    fn failed_staged_import_never_acknowledges_or_frees_slot() {
        let f = Fixture::start_mode(true).unwrap();
        let adapter = f.adapter("unsafe-import");
        let Reply::Flow(FlowReply::CheckedIn(r)) = adapter.call(Request::Checkin {
            attempt: 1,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        }) else {
            panic!("Check-in fehlt")
        };
        let folder = pi_sync::LocalFolder::create(&adapter.config.local_root).unwrap();
        let remote = f.root.join("home/arbeit/Projekte/Buch");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(remote.join("text.md"), "untrusted").unwrap();
        std::os::unix::fs::symlink(&f.root, folder.root.join("Projekte/Buch")).unwrap();
        let Reply::Flow(FlowReply::Preview(p)) =
            adapter.call(Request::CheckoutPreview(r.checkin_id.clone()))
        else {
            panic!("Vorschau fehlt")
        };
        let reply = adapter.call(Request::Checkout {
            checkin_id: r.checkin_id,
            preview_id: p.id,
            large_files: vec![],
        });
        assert!(matches!(reply, Reply::Error(_)), "{reply:?}");
        assert!(adapter.state.lock().unwrap().receipt.is_some());
        assert!(!f.root.join("pool-frei").exists());
        assert!(remote.join("text.md").exists());
        assert!(!f.root.join("text.md").exists());
        assert!(
            adapter.transfer.progress().is_none(),
            "Fehler beendet Fortschritt per Guard"
        );
    }
    #[test]
    fn p2p_identity_persists_and_is_used_instead_of_config_key() {
        let f = Fixture::start_mode(true).unwrap();
        let adapter = f.adapter("identity");
        let original = adapter.identity.peer();
        let again = f.adapter("identity");
        assert_eq!(again.identity.peer(), original);
        let reply = adapter.call(Request::Checkin {
            attempt: 1,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        });
        assert!(
            matches!(reply, Reply::Flow(FlowReply::CheckedIn(_))),
            "{reply:?}"
        );
        let s = adapter.state.lock().unwrap();
        assert_eq!(s.ausweis.as_ref().unwrap()["net_pubkey"], original.0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(adapter.config.key_dir.join("seminar-net.key"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn ambiguous_receipt_is_not_resent_or_reimported() {
        let f = Fixture::start_mode(true).unwrap();
        std::fs::write(f.root.join("lose-erhalten"), b"yes").unwrap();
        let adapter = f.adapter("ack");
        let Reply::Flow(FlowReply::CheckedIn(r)) = adapter.call(Request::Checkin {
            attempt: 1,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        }) else {
            panic!("Check-in fehlt")
        };
        let remote = f.root.join("home/arbeit/Projekte/Buch");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(remote.join("text.md"), "einmal importieren").unwrap();
        let Reply::Flow(FlowReply::Preview(p)) =
            adapter.call(Request::CheckoutPreview(r.checkin_id.clone()))
        else {
            panic!("Vorschau fehlt")
        };
        for attempt in 0..2 {
            let reply = adapter.call(Request::Checkout {
                checkin_id: r.checkin_id.clone(),
                preview_id: p.id.clone(),
                large_files: vec![],
            });
            if attempt == 0 {
                assert!(matches!(
                    reply,
                    Reply::Flow(FlowReply::ClearUnconfirmed(ReturnSummary { files: 1, .. }))
                ));
            } else {
                assert!(matches!(reply, Reply::Error(_)));
            }
        }
        assert_eq!(
            std::fs::read_to_string(f.root.join("ack-attempts")).unwrap(),
            "1\n"
        );
        assert_eq!(
            std::fs::read_dir(adapter.config.local_root.join("Projekte/Buch"))
                .unwrap()
                .count(),
            1
        );
        assert!(adapter.state.lock().unwrap().receipt.is_some());
        assert!(!f.root.join("pool-frei").exists());
    }
    #[test]
    fn forced_checkout_locker_persistent_key_decrypt_conflict_import_then_ack() {
        let f = Fixture::start_mode(true).unwrap();
        std::fs::write(
            f.root.join("empty-checkout"),
            b"fixture skips normal checkout assertion",
        )
        .unwrap();
        let first = f.adapter("locker");
        first.state.lock().unwrap().attempt = 1;
        let proof = f.proof(&first);
        let FlowReply::CheckedIn(r) = first
            .checkin(&mut first.state.lock().unwrap(), proof)
            .unwrap()
        else {
            panic!("Check-in fehlt")
        };
        let public = first.archive_key.public_b64url.clone();
        let remote = f.root.join("home/arbeit/Projekte/Buch");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(remote.join("text.md"), "Abholfach R2").unwrap();
        f.force(&r.checkin_id);
        drop(first);
        let adapter = f.adapter("locker");
        assert_eq!(public, adapter.archive_key.public_b64url);
        adapter.state.lock().unwrap().attempt = 2;
        let proof = f.proof(&adapter);
        let FlowReply::CheckedIn(r) = adapter
            .checkin(&mut adapter.state.lock().unwrap(), proof)
            .unwrap()
        else {
            panic!("Zweiter Check-in fehlt")
        };
        let pickup = r.pickup.unwrap();
        let boxes = f.root.join("empfang/abholfach");
        assert_eq!(std::fs::read_dir(&boxes).unwrap().count(), 1);
        // Später abholen führt nur zur Auswahl: weder Import noch Bestätigung.
        assert!(matches!(
            adapter.call(Request::SyncPlan(r.checkin_id.clone())),
            Reply::Flow(FlowReply::Plan(_))
        ));
        assert_eq!(std::fs::read_dir(&boxes).unwrap().count(), 1);
        assert!(!adapter
            .config
            .local_root
            .join("Projekte/Buch/text.md")
            .exists());
        let local = adapter.config.local_root.join("Projekte/Buch");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("text.md"), "Lokale Änderung").unwrap();
        // Falscher Schlüssel und unsicheres Ziel dürfen das Abholfach nicht quittieren.
        let wrong = pi_backend::hub_v1::abholfach::Schluessel::laden_oder_erzeugen(
            f.root.join("wrong-key"),
        )
        .unwrap();
        let state = adapter.state.lock().unwrap();
        let folder = pi_sync::LocalFolder::create(&adapter.config.local_root).unwrap();
        assert!(abholfach_flow::holen(
            &state.empfang,
            &wrong,
            &folder,
            &pickup.id,
            &adapter.transfer
        )
        .is_err());
        drop(state);
        assert_eq!(std::fs::read_dir(&boxes).unwrap().count(), 1);
        #[cfg(unix)]
        {
            std::fs::remove_file(local.join("text.md")).unwrap();
            std::os::unix::fs::symlink(&f.root, local.join("text.md")).unwrap();
            assert!(matches!(
                adapter.call(Request::Collect {
                    checkin_id: r.checkin_id.clone(),
                    pickup_id: pickup.id.clone()
                }),
                Reply::Error(_)
            ));
            assert_eq!(
                std::fs::read_dir(&boxes).unwrap().count(),
                1,
                "Fehlgeschlagener Import quittiert nichts"
            );
            std::fs::remove_file(local.join("text.md")).unwrap();
            std::fs::write(local.join("text.md"), "Lokale Änderung").unwrap();
        }
        let reply = adapter.call(Request::Collect {
            checkin_id: r.checkin_id,
            pickup_id: pickup.id,
        });
        let Reply::Flow(FlowReply::Collected(summary)) = reply else {
            panic!("{reply:?}")
        };
        assert_eq!(summary.files, 1);
        assert_eq!(summary.conflicts, 1);
        assert_eq!(
            std::fs::read_to_string(local.join("text.md")).unwrap(),
            "Lokale Änderung"
        );
        let conflict = std::fs::read_dir(&local)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.file_name().unwrap().to_string_lossy().contains(" (R2 "))
            .unwrap();
        assert_eq!(std::fs::read_to_string(conflict).unwrap(), "Abholfach R2");
        assert_eq!(std::fs::read_dir(&boxes).unwrap().count(), 0);
        assert!(
            adapter.state.lock().unwrap().receipt.is_some(),
            "Neue Belegung bleibt bestehen"
        );
        assert!(adapter.transfer.progress().is_none());
    }

    #[test]
    fn locker_ambiguous_ack_is_never_repeated_or_reimported() {
        let f = Fixture::start_mode(true).unwrap();
        std::fs::write(
            f.root.join("empty-checkout"),
            b"fixture skips normal checkout assertion",
        )
        .unwrap();
        let first = f.adapter("locker-ack");
        first.state.lock().unwrap().attempt = 1;
        let proof = f.proof(&first);
        let FlowReply::CheckedIn(r) = first
            .checkin(&mut first.state.lock().unwrap(), proof)
            .unwrap()
        else {
            panic!("Check-in fehlt")
        };
        let remote = f.root.join("home/arbeit/Projekte/Buch");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(remote.join("text.md"), "Nur einmal importieren").unwrap();
        f.force(&r.checkin_id);
        drop(first);
        let adapter = f.adapter("locker-ack");
        adapter.state.lock().unwrap().attempt = 2;
        let proof = f.proof(&adapter);
        let FlowReply::CheckedIn(r) = adapter
            .checkin(&mut adapter.state.lock().unwrap(), proof)
            .unwrap()
        else {
            panic!("Check-in fehlt")
        };
        let pickup_id = r.pickup.unwrap().id;
        std::fs::write(f.root.join("lose-erhalten"), b"yes").unwrap();
        for _ in 0..2 {
            assert!(matches!(
                adapter.call(Request::Collect {
                    checkin_id: r.checkin_id.clone(),
                    pickup_id: pickup_id.clone()
                }),
                Reply::Error(_)
            ));
        }
        assert_eq!(
            std::fs::read_to_string(f.root.join("ack-attempts")).unwrap(),
            "1\n"
        );
        assert_eq!(
            std::fs::read_to_string(adapter.config.local_root.join("Projekte/Buch/text.md"))
                .unwrap(),
            "Nur einmal importieren"
        );
        assert_eq!(
            std::fs::read_dir(adapter.config.local_root.join(".sync/archiv"))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            std::fs::read_dir(f.root.join("empfang/abholfach"))
                .unwrap()
                .count(),
            1
        );
        assert!(adapter.state.lock().unwrap().pickup_unconfirmed);
        assert!(adapter.transfer.progress().is_none());
    }

    #[test]
    fn large_file_requires_individual_upload_and_checkout_confirmation() {
        let f = Fixture::start_mode(true).unwrap();
        std::fs::write(
            f.root.join("empty-checkout"),
            b"separate large-file assertions",
        )
        .unwrap();
        let adapter = f.adapter("large");
        let folder = pi_sync::LocalFolder::create(&adapter.config.local_root).unwrap();
        let project = folder.root.join("Projekte/Film");
        std::fs::create_dir_all(&project).unwrap();
        let size = 200 * 1024 * 1024 + 1;
        std::fs::File::create(project.join("film.mov"))
            .unwrap()
            .set_len(size)
            .unwrap();
        std::fs::File::create(project.join("nicht-gewaehlt.mov"))
            .unwrap()
            .set_len(size)
            .unwrap();
        let Reply::Flow(FlowReply::CheckedIn(r)) = adapter.call(Request::Checkin {
            attempt: 1,
            method: CheckinMethod::Guest("test-invitation-one".into()),
        }) else {
            panic!("Check-in fehlt")
        };
        let Reply::Flow(FlowReply::Plan(plan)) =
            adapter.call(Request::SyncPlan(r.checkin_id.clone()))
        else {
            panic!("Plan fehlt")
        };
        let large = plan
            .items
            .iter()
            .find(|i| i.id.starts_with("gross-") && i.name.contains(" / film.mov"))
            .unwrap();
        assert!(!large.selected);
        let reply = adapter.call(Request::Upload {
            checkin_id: r.checkin_id.clone(),
            selection: vec![
                plan.items
                    .iter()
                    .find(|i| i.name == "Projekte / Film")
                    .unwrap()
                    .id
                    .clone(),
                large.id.clone(),
            ],
        });
        assert!(
            matches!(reply, Reply::Flow(FlowReply::Uploaded)),
            "{reply:?}"
        );
        let remote = f.root.join("home/arbeit/Projekte/Film");
        assert_eq!(
            std::fs::metadata(remote.join("film.mov")).unwrap().len(),
            size
        );
        assert!(!remote.join("nicht-gewaehlt.mov").exists());
        // Eine neue große R2-Datei muss in der Vorschau einzeln bestätigt werden.
        std::fs::rename(remote.join("film.mov"), remote.join("ergebnis.mov")).unwrap();
        let Reply::Flow(FlowReply::Preview(p)) =
            adapter.call(Request::CheckoutPreview(r.checkin_id.clone()))
        else {
            panic!("Vorschau fehlt")
        };
        assert!(p
            .personal
            .iter()
            .any(|f| f.large && !f.selected && f.path.ends_with("ergebnis.mov")));
        assert!(matches!(
            adapter.call(Request::Checkout {
                checkin_id: r.checkin_id.clone(),
                preview_id: p.id.clone(),
                large_files: vec![]
            }),
            Reply::Error(_)
        ));
        assert!(remote.join("ergebnis.mov").exists());
        assert!(!f.root.join("pool-frei").exists());
        let large_files = p
            .personal
            .iter()
            .filter(|f| f.large)
            .map(|f| f.path.clone())
            .collect();
        let reply = adapter.call(Request::Checkout {
            checkin_id: r.checkin_id,
            preview_id: p.id,
            large_files,
        });
        assert!(
            matches!(reply, Reply::Flow(FlowReply::CheckedOut(_))),
            "{reply:?}"
        );
        assert_eq!(
            std::fs::metadata(project.join("ergebnis.mov"))
                .unwrap()
                .len(),
            size
        );
        assert_eq!(
            std::fs::metadata(project.join("film.mov")).unwrap().len(),
            size
        );
        assert!(f.root.join("pool-frei").exists());
    }

    #[test]
    fn local_fake_oidc_proof_is_verified_by_real_empfang() {
        let Some(f) = Fixture::start() else {
            return;
        };
        let adapter = f.adapter("oidc");
        let proof = f.proof(&adapter);
        let mut state = adapter.state.lock().unwrap();
        state.attempt = 1;
        let reply = adapter.checkin(&mut state, proof).unwrap();
        assert!(matches!(reply, FlowReply::CheckedIn(_)));
        assert_eq!(state.receipt.as_ref().unwrap().account.person, "Testperson");
    }
}
