//! Seminar-Desktop: eine Oberfläche, ein Modus-Ort, keine lokalen Agentenstarts.
//! Alle Hub-Aktionen laufen über DesktopApi; Fixtures sind ausschließlich Demo/Tests.
mod abholfach_flow;
mod checkin;
pub mod flow_adapter;
mod hub;
pub mod seminar;
mod sync_flow;
mod sync_ui;
#[cfg(test)]
use checkin::AuthCallback;
pub use checkin::{CancelTarget, CheckinMethod, FlowReply, MemoryApi, WaitTarget};
use checkin::{Flow, Stage, TransferKind};
use egui::{Color32, RichText, Ui};
use pi_backend::mode::{self, Mode};
use qrcodegen::{QrCode, QrCodeEcc};
use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

pub const HYBRID_TEXT: &str = "Dann kann cocopi hier einen Agenten starten. Er hat Zugriff auf die Dateien dieses Rechners. Laufende Sessions bleiben, wie sie sind. Du kannst jederzeit zurückschalten.";
pub const LIMIT_TEXT: &str = "Du hast schon 2 offene Sessions. Lass eine ruhen, dann kannst du diese öffnen. Es geht nichts verloren.";
pub const REVOKED_TEXT: &str = "Dieses Gerät wurde abgemeldet. Melde dich mit einer neuen Einladung oder durch Koppeln wieder an.";
const WORKING: Color32 = Color32::from_rgb(0xe4, 0x54, 0xc4);
const QUIET: Color32 = Color32::from_rgb(0xad, 0xb4, 0xbe);

#[derive(Clone, Debug, PartialEq)]
pub enum InviteError {
    Expired,
    Used,
    Unknown,
    RateLimited,
    Revoked,
    Other(String),
}
impl InviteError {
    pub fn text(&self) -> String {
        match self {
            Self::Expired => "Diese Einladung ist abgelaufen. Bitte deine Seminarleitung um eine neue.".into(),
            Self::Used => "Diese Einladung wurde schon benutzt. Jede Einladung gilt nur einmal. Wenn du das nicht warst, sag deiner Seminarleitung Bescheid.".into(),
            Self::Unknown => "Dieser Code passt zu keiner Einladung. Prüfe die Zeichen oder öffne den Link direkt.".into(),
            Self::RateLimited => "Zu viele Versuche. Warte eine Minute und versuche es dann noch einmal.".into(),
            Self::Revoked => REVOKED_TEXT.into(),
            Self::Other(s) => s.clone(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Account {
    pub person: String,
    pub slot: String,
    pub device_name: String,
    pub device_id: String,
}
#[derive(Clone, Debug)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub last_seen: String,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionState {
    Resting,
    Ready,
    Working,
    NeedsYou,
    Done,
    Error,
}
/// N15: nur nichtlokale Hub-Sessions, ohne Inhalte oder Pfade.
fn seminar_ankuendigung(sessions: &[Session]) -> Vec<pi_net::wire::SessionAnkuendigung> {
    sessions
        .iter()
        .filter(|s| !s.local)
        .take(32)
        .map(|s| pi_net::wire::SessionAnkuendigung {
            id: s.id.clone(),
            label: s.title.chars().take(120).collect(),
            zustand: match s.state.text() {
                "" => "bereit".into(),
                t => t.into(),
            },
        })
        .collect()
}
impl SessionState {
    pub fn text(self) -> &'static str {
        match self {
            Self::Resting => "ruht",
            Self::Ready => "",
            Self::Working => "arbeitet",
            Self::NeedsYou => "wartet auf dich",
            Self::Done => "fertig",
            Self::Error => "Fehler",
        }
    }
    fn color(self) -> Color32 {
        if self == Self::Working {
            WORKING
        } else {
            QUIET
        }
    }
}
#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub state: SessionState,
    pub local: bool,
    pub pinned: bool,
}
#[derive(Clone, Debug)]
pub struct PairCode {
    pub url: String,
    pub code: String,
    pub valid_for: Duration,
}
#[derive(Clone, Debug)]
pub enum Request {
    Checkin {
        attempt: u64,
        method: CheckinMethod,
    },
    #[cfg(test)]
    AuthCallback(AuthCallback),
    Wait(WaitTarget),
    SyncPlan(String),
    Upload {
        checkin_id: String,
        selection: Vec<String>,
    },
    CheckoutPreview(String),
    Checkout {
        checkin_id: String,
        preview_id: String,
        large_files: Vec<String>,
    },
    Collect {
        checkin_id: String,
        pickup_id: String,
    },
    CancelFlow(CancelTarget),
    Redeem(String),
    Devices,
    Pair,
    PairStatus(String),
    PairDecision(String, bool),
    Revoke(String),
    Rename,
    Sessions,
    Open(String),
    Rest(String),
    Prompt(String, String),
    #[cfg(test)]
    PromptAs {
        session: String,
        text: String,
        von: pi_net::seminar::Von,
    },
    Stop(String),
}
#[derive(Clone, Debug)]
pub enum Reply {
    ConnectionEvent {
        epoch: u64,
        reply: Box<Reply>,
    },
    Flow(FlowReply),
    FlowCompleted {
        generation: u64,
        reply: Box<Reply>,
    },
    Completed(Box<Reply>),
    Heartbeat,
    AktivCompleted(Result<(), String>),
    VerzeichnisCompleted(Result<Option<pi_backend::hub_v1::empfang::Verzeichnis>, String>),
    Disconnected(String),
    Sent(String, String),
    #[cfg(test)]
    SentAs(String, String, pi_net::seminar::Von),
    #[cfg(test)]
    SeminarPrompt {
        session_id: String,
        prompt: seminar::HubPrompt,
    },
    SessionUpdated(Session),
    PiEvent(String, serde_json::Value),
    SignedIn(Account),
    Devices(Vec<Device>),
    Pair(PairCode),
    PairLogin {
        id: String,
        pair: PairCode,
    },
    PairWaiting {
        id: String,
        name: String,
        words: String,
    },
    PairDone,
    Revoked(String),
    Sessions(Vec<Session>),
    Opened(String, Vec<Session>),
    Ok,
    Limit,
    Error(InviteError),
}
/// Impl darf blockieren: Aufrufe werden immer in einen Worker ausgelagert.
pub trait DesktopApi: Send + Sync {
    fn call(&self, request: Request) -> Reply;
    fn watch(&self, _tx: mpsc::Sender<Reply>) {}
    /// Wird auf einem Worker aufgerufen, nie während eines UI-Frames blockierend.
    fn aktiv_signal(&self, _connected: bool) -> Result<(), String> {
        Ok(())
    }
    fn trennen(&self) -> Result<(), String> {
        self.aktiv_signal(false)
    }
    /// N11: eigene direkte Adressen für das nächste Aktiv-Signal (nur IP:Port).
    fn set_eigene_adressen(&self, _adressen: Vec<String>) {}
    fn seminar_verzeichnis(
        &self,
    ) -> Result<Option<pi_backend::hub_v1::empfang::Verzeichnis>, String> {
        Ok(None)
    }
    fn seminar(&self, _sessions: &[Session]) -> Result<Option<seminar::Seminar>, String> {
        Ok(None)
    }
    fn progress(&self) -> Option<checkin::Progress> {
        None
    }
    fn open_browser(&self, _url: &str) -> Result<(), String> {
        Err("Es gibt keine bestätigte Browseranmeldung.".into())
    }
}
struct Unavailable;
impl DesktopApi for Unavailable {
    fn call(&self, _: Request) -> Reply {
        Reply::Error(InviteError::Other("Die Verbindung ist noch nicht eingerichtet. Bitte deine Seminarleitung um den freigegebenen Hub-Zugang.".into()))
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Section {
    Account,
    Devices,
    Mode,
    Appearance,
    Speech,
    Advanced,
}
const SECTIONS: [(Section, &str); 6] = [
    (Section::Account, "Konto"),
    (Section::Devices, "Geräte"),
    (Section::Mode, "Modus"),
    (Section::Appearance, "Darstellung"),
    (Section::Speech, "Vorlesen"),
    (Section::Advanced, "Erweitert"),
];
struct Pairing {
    code: PairCode,
    expires: Instant,
    texture: Option<egui::TextureHandle>,
}
#[derive(Clone, Debug)]
struct ChatMessage {
    speaker: String,
    text: String,
}
impl ChatMessage {
    fn new(speaker: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            speaker: speaker.into(),
            text: text.into(),
        }
    }
}

pub struct Desktop {
    pub enabled: bool,
    demo: bool,
    api: Arc<dyn DesktopApi>,
    api_configured: bool,
    tx: mpsc::Sender<Reply>,
    rx: mpsc::Receiver<Reply>,
    pending: usize,
    connection_epoch: u64,
    mode: Mode,
    pub account: Option<Account>,
    connected: bool,
    invitation: String,
    welcome_done: bool,
    error: Option<String>,
    settings: Option<Section>,
    hybrid_dialog: bool,
    hybrid_focus: bool,
    devices: Vec<Device>,
    revoke: Option<Device>,
    pairing: Option<Pairing>,
    pairing_login: Option<String>,
    pairing_poll: Option<Instant>,
    pair_request: Option<(String, String, String)>,
    limit: bool,
    sessions: Vec<Session>,
    selected: Option<String>,
    messages: Vec<ChatMessage>,
    prompt: String,
    files: bool,
    seminar_selected: bool,
    seminar: Option<seminar::Seminar>,
    flow: Flow,
    flow_generation: u64,
    flow_pending: bool,
    flow_poll: Option<Instant>,
    flow_retry: Option<Request>,
    flow_failed: bool,
    guest: bool,
    rename: bool,
    device_name: String,
    hub_url: String,
    oidc_issuer: String,
    empfang_pubkey: String,
    confirm_host: Option<String>,
    last_event: Option<Instant>,
    watch_started: bool,
    aktiv_clock: Instant,
    aktiv_takt: flow_adapter::AktivTakt,
    aktiv_pending: bool,
    verzeichnis_pending: bool,
    verzeichnis_zuletzt: Option<Instant>,
    verzeichnis: Option<pi_backend::hub_v1::empfang::Verzeichnis>,
    chat_by_session: std::collections::HashMap<String, Vec<ChatMessage>>,
    streaming: std::collections::HashMap<String, String>,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Profile {
    hub_url: String,
    oidc_issuer: String,
    empfang_pubkey: String,
    device_name: String,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            hub_url: String::new(),
            oidc_issuer: String::new(),
            empfang_pubkey: String::new(),
            device_name: "Mein Rechner".into(),
        }
    }
}
impl Profile {
    fn load() -> Self {
        pi_backend::copi_config_dir()
            .and_then(|p| std::fs::read(p.join("desktop-v3.json")).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    fn save(&self) -> Result<(), String> {
        let root = pi_backend::copi_config_dir()
            .ok_or("Kein Konfigurationsordner. Die Anmeldung wurde nicht gestartet.")?;
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let temp = root.join(format!("desktop-v3-{}.new", std::process::id()));
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        use std::io::Write;
        let mut file = opts.open(&temp).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec(self).map_err(|e| e.to_string())?)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(temp, root.join("desktop-v3.json")).map_err(|e| e.to_string())
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        if !self.demo && self.api_configured {
            // Fensterende: vor Prozessende bestätigen lassen; kein detach-and-forget.
            let _ = self.api.trennen();
        }
    }
}
impl Desktop {
    pub fn new(demo: bool) -> Self {
        let profile = if demo {
            Profile::default()
        } else {
            Profile::load()
        };
        let (tx, rx) = mpsc::channel();
        Self {
            enabled: !cfg!(test) || !demo,
            demo,
            api: Arc::new(Unavailable),
            api_configured: false,
            tx,
            rx,
            pending: 0,
            connection_epoch: 0,
            mode: if demo {
                Mode::RemoteOnly
            } else {
                mode::current()
            },
            account: None,
            connected: false,
            invitation: String::new(),
            welcome_done: false,
            error: None,
            settings: None,
            hybrid_dialog: false,
            hybrid_focus: false,
            devices: vec![],
            revoke: None,
            pairing: None,
            pairing_login: None,
            pairing_poll: None,
            pair_request: None,
            limit: false,
            sessions: vec![],
            selected: None,
            messages: vec![],
            prompt: String::new(),
            files: false,
            seminar_selected: false,
            seminar: None,
            flow: Flow::default(),
            flow_generation: 0,
            flow_pending: false,
            flow_poll: None,
            flow_retry: None,
            flow_failed: false,
            guest: false,
            rename: false,
            device_name: profile.device_name,
            hub_url: profile.hub_url,
            oidc_issuer: profile.oidc_issuer,
            empfang_pubkey: profile.empfang_pubkey,
            confirm_host: None,
            last_event: None,
            watch_started: false,
            aktiv_clock: Instant::now(),
            aktiv_takt: flow_adapter::AktivTakt::default(),
            aktiv_pending: false,
            verzeichnis_pending: false,
            verzeichnis_zuletzt: None,
            verzeichnis: None,
            chat_by_session: Default::default(),
            streaming: Default::default(),
        }
    }
    pub fn set_api(&mut self, api: Arc<dyn DesktopApi>) {
        self.api = api;
        self.api_configured = true;
    }
    fn call(&mut self, request: Request) {
        if self.demo {
            self.error = Some(
                "Vorschau: Es wird keine Verbindung aufgebaut und kein Agent gestartet.".into(),
            );
            return;
        }
        self.pending += 1;
        let (api, tx, epoch) = (self.api.clone(), self.tx.clone(), self.connection_epoch);
        std::thread::spawn(move || {
            let _ = tx.send(Reply::ConnectionEvent {
                epoch,
                reply: Box::new(Reply::Completed(Box::new(api.call(request)))),
            });
        });
    }
    fn apply(&mut self, reply: Reply) {
        match reply {
            Reply::ConnectionEvent { epoch, reply } => {
                if epoch == self.connection_epoch {
                    self.apply(*reply);
                }
            }
            Reply::FlowCompleted { generation, reply } => {
                if generation == self.flow_generation {
                    self.flow_pending = false;
                    self.apply(*reply);
                }
            }
            Reply::Flow(reply) => {
                if matches!(reply, FlowReply::CheckedIn(_)) {
                    self.connected = true;
                    self.verzeichnis_zuletzt = None;
                    self.verzeichnis = None;
                }
                if let FlowReply::Browser { attempt, url } = &reply {
                    if !self.demo
                        && *attempt == self.flow.attempt
                        && self.flow.stage == Stage::Starting
                    {
                        if let Err(e) = self.api.open_browser(url) {
                            self.error = Some(format!(
                                "Der Browser konnte nicht geöffnet werden. Öffne die Anmeldung mit dem Knopf unten. {e}"
                            ));
                        }
                    }
                }
                let next = self.flow.apply(reply);
                if let Some(receipt) = &self.flow.receipt {
                    self.account = Some(receipt.account.clone());
                }
                if self.flow.stage == Stage::Working {
                    self.welcome_done = true;
                    if !self.demo && !self.watch_started {
                        self.start_watch();
                        self.call(Request::Sessions);
                    }
                }
                if self.flow.stage == Stage::Done {
                    if let Some(seminar) = &mut self.seminar {
                        seminar.checkout();
                    }
                    self.seminar = None;
                    self.seminar_selected = false;
                    self.connection_epoch += 1;
                    self.pending = 0;
                    self.watch_started = false;
                    self.last_event = None;
                    self.account = None;
                    self.connected = false;
                    self.verzeichnis = None;
                    self.verzeichnis_zuletzt = None;
                    self.verzeichnis_pending = false;
                    self.welcome_done = false;
                    self.sessions.clear();
                    self.messages.clear();
                    self.chat_by_session.clear();
                    self.streaming.clear();
                    self.selected = None;
                    self.pairing = None;
                    self.pairing_login = None;
                    self.pair_request = None;
                    self.settings = None;
                }
                if let Some(request) = next {
                    self.flow_call(request);
                }
            }
            Reply::Completed(reply) => {
                self.pending = self.pending.saturating_sub(1);
                self.apply(*reply);
            }
            Reply::AktivCompleted(result) => {
                self.aktiv_pending = false;
                if let Err(error) = result {
                    self.error = Some(error);
                }
            }
            Reply::VerzeichnisCompleted(result) => {
                self.verzeichnis_pending = false;
                if self.flow.receipt.is_some() {
                    match result {
                        Ok(Some(verzeichnis)) => {
                            self.verzeichnis = Some(verzeichnis);
                            self.verzeichnis_anwenden();
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.error = Some(format!(
                                "Das Seminarverzeichnis konnte nicht aktualisiert werden. Du kannst weiterarbeiten. {error}"
                            ));
                        }
                    }
                }
            }
            Reply::Heartbeat => {
                self.last_event = Some(Instant::now());
                self.connected = true;
            }
            Reply::Disconnected(e) => {
                self.connected = false;
                self.watch_started = false;
                self.pending = 0;
                self.error = Some(e);
            }
            Reply::Sent(id, text) => {
                if self.prompt == text {
                    self.prompt.clear();
                }
                self.chat_by_session
                    .entry(id)
                    .or_default()
                    .push(ChatMessage::new("Du", text));
            }
            #[cfg(test)]
            Reply::SeminarPrompt { session_id, prompt } => {
                self.send_as(&session_id, prompt.von, prompt.text);
            }
            #[cfg(test)]
            Reply::SentAs(id, text, von) => {
                self.chat_by_session
                    .entry(id)
                    .or_default()
                    .push(ChatMessage::new(von.label(), text));
            }
            Reply::SessionUpdated(session) => {
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == session.id) {
                    let pinned = s.pinned;
                    *s = session;
                    s.pinned = pinned;
                } else {
                    self.sessions.push(session);
                }
            }
            Reply::PiEvent(id, event) => self.pi_event(id, event),
            Reply::SignedIn(account) => {
                self.device_name = account.device_name.clone();
                self.account = Some(account);
                self.pairing_login = None;
                self.pairing = None;
                self.connected = true;
                self.error = None;
                self.start_watch();
                self.call(Request::Devices);
                self.call(Request::Sessions);
            }
            Reply::Devices(devices) => self.devices = devices,
            Reply::PairLogin { id, pair } => {
                self.pairing_login = Some(id);
                self.pairing_poll = Some(Instant::now());
                self.apply(Reply::Pair(pair));
            }
            Reply::Pair(code) => {
                // Der QR-Schritt ist eine eigene Ansicht, kein Dialog im Einstellungsfenster.
                self.settings = None;
                self.pairing = Some(Pairing {
                    expires: Instant::now() + code.valid_for,
                    code,
                    texture: None,
                })
            }
            Reply::PairWaiting { id, name, words } => {
                self.pair_request = Some((id, name, words));
                self.pairing = None;
            }
            Reply::PairDone => {
                self.pair_request = None;
                self.pairing = None;
                self.call(Request::Devices);
            }
            Reply::Revoked(id) => {
                self.devices.retain(|d| d.id != id);
                if self.account.as_ref().is_some_and(|a| a.device_id == id) {
                    self.apply(Reply::Error(InviteError::Revoked));
                }
            }
            Reply::Opened(id, sessions) => {
                self.apply(Reply::Sessions(sessions));
                self.selected = Some(id);
            }
            Reply::Sessions(mut sessions) => {
                for session in &mut sessions {
                    session.pinned = self.sessions.iter().any(|s| s.id == session.id && s.pinned);
                }
                if self.selected.is_none() {
                    self.selected = sessions.first().map(|s| s.id.clone());
                }
                self.sessions = sessions;
                self.connected = true;
                if !self.demo && self.flow.receipt.is_some() {
                    if self.seminar.is_none() {
                        match self.api.seminar(&self.sessions) {
                            Ok(seminar) => {
                                self.seminar = seminar;
                                self.verzeichnis_anwenden();
                            }
                            Err(error) => {
                                self.error = Some(format!("Seminar noch nicht verbunden. {error}"))
                            }
                        }
                    }
                    // N15: eigene Sessions ankündigen, ohne das Seminar neu aufzubauen.
                    let ankuendigung = seminar_ankuendigung(&self.sessions);
                    if let Some(seminar) = &mut self.seminar {
                        if let Err(error) = seminar.sessions_aktualisieren(ankuendigung) {
                            self.error = Some(format!(
                                "Sessions konnten im Seminar nicht angezeigt werden. {error}"
                            ));
                        }
                    }
                }
            }
            Reply::Limit => self.limit = true,
            Reply::Error(e) => {
                if self.flow_retry.is_some() && self.flow.stage != Stage::Working {
                    self.flow_failed = true;
                }
                if e == InviteError::Revoked {
                    self.connected = false;
                }
                if e == InviteError::Revoked {
                    if let Some(seminar) = &mut self.seminar {
                        seminar.checkout();
                    }
                    self.seminar = None;
                    self.connection_epoch += 1;
                    self.pending = 0;
                    self.account = None;
                    self.flow = Flow::default();
                    self.verzeichnis = None;
                    self.verzeichnis_zuletzt = None;
                    self.verzeichnis_pending = false;
                    self.flow_generation += 1;
                    self.flow_pending = false;
                    self.welcome_done = false;
                    self.sessions.clear();
                    self.messages.clear();
                    self.chat_by_session.clear();
                    self.streaming.clear();
                    self.selected = None;
                    self.pairing = None;
                    self.pairing_login = None;
                    self.pair_request = None;
                    self.watch_started = false;
                }
                self.error = Some(e.text());
            }
            Reply::Ok => self.error = None,
        }
    }
    pub fn status(&self) -> String {
        let prefix = if self.mode == Mode::Hybrid {
            "⇄ Hybrid"
        } else {
            "☁ Nur Remote"
        };
        let account = self
            .flow
            .receipt
            .as_ref()
            .map(|r| &r.account)
            .or(self.account.as_ref());
        match account {
            Some(a) => format!(
                "{prefix} · {} · {} @ R2 · {}",
                a.person,
                a.slot,
                if self.connected {
                    if let Some(receipt) = &self.flow.receipt {
                        return format!(
                            "{prefix} · {} · {} @ R2 · eingecheckt seit {}",
                            a.person, a.slot, receipt.since
                        );
                    }
                    "verbunden"
                } else {
                    "offline"
                }
            ),
            None => format!("{prefix} · nicht eingecheckt"),
        }
    }
    fn redeem(&mut self) {
        self.error = None;
        if self.demo {
            self.call(Request::Redeem(self.invitation.clone()));
            return;
        }
        if self.hub_url.trim().is_empty() {
            match pi_backend::hub_v1::parse_invitation(&self.invitation, 'e') {
                Ok(invite) if invite.hub_url.is_some() => {
                    self.confirm_host = invite.hub_url;
                    return;
                }
                Ok(_) => {}
                Err(_) => {
                    self.error = Some(InviteError::Unknown.text());
                    return;
                }
            }
            self.error = Some("Zu diesem Code fehlt die Hub-Adresse. Bitte deine Seminarleitung um den vollständigen Einladungslink. Die Adresse kann auch unter Einstellungen › Erweitert › Verbindung bestätigt werden.".into());
            return;
        }
        match hub::HubAdapter::new(self.hub_url.trim(), &self.device_name) {
            Ok(api) => {
                let profile = Profile {
                    hub_url: self.hub_url.clone(),
                    oidc_issuer: self.oidc_issuer.clone(),
                    empfang_pubkey: self.empfang_pubkey.clone(),
                    device_name: self.device_name.clone(),
                };
                if let Err(e) = profile.save() {
                    self.error = Some(e);
                    return;
                }
                self.set_api(Arc::new(api));
                self.call(Request::Redeem(self.invitation.trim().into()));
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn start_watch(&mut self) {
        if self.demo || self.watch_started {
            return;
        }
        self.watch_started = true;
        self.last_event = Some(Instant::now());
        let (api, tx, epoch) = (self.api.clone(), self.tx.clone(), self.connection_epoch);
        let (watch_tx, watch_rx) = mpsc::channel();
        std::thread::spawn(move || api.watch(watch_tx));
        std::thread::spawn(move || {
            while let Ok(reply) = watch_rx.recv() {
                if tx
                    .send(Reply::ConnectionEvent {
                        epoch,
                        reply: Box::new(reply),
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }
    fn pi_event(&mut self, id: String, event: serde_json::Value) {
        match event["type"].as_str() {
            Some("message_update") => {
                let e = &event["assistantMessageEvent"];
                if e["type"] == "text_delta" {
                    self.streaming
                        .entry(id)
                        .or_default()
                        .push_str(e["delta"].as_str().unwrap_or_default());
                }
            }
            Some("message_end") => {
                self.streaming.remove(&id);
                let message = &event["message"];
                if message["role"] == "assistant" {
                    let text = message["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| {
                            if v["type"] == "text" {
                                v["text"].as_str()
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() {
                        self.chat_by_session
                            .entry(id)
                            .or_default()
                            .push(ChatMessage::new("cocopi · R2", text));
                    }
                }
            }
            Some("extension_ui_request") => {
                self.error = Some("Die Session wartet auf dich. Diese Rückfrage kann in I1 noch nicht im Desktop beantwortet werden. Es wird nichts automatisch bestätigt.".into());
            }
            _ => {}
        }
    }
    fn switch(&mut self, mode: Mode) {
        let result = if self.demo { Ok(()) } else { mode::set(mode) };
        match result {
            Ok(()) => {
                self.mode = mode;
                self.hybrid_dialog = false;
            }
            Err(e) => self.error = Some(format!("Der Modus bleibt unverändert. {e}")),
        }
    }
    pub fn draw(&mut self, ui: &mut Ui) {
        while let Ok(reply) = self.rx.try_recv() {
            self.apply(reply);
        }
        self.flow_tick(ui.ctx());
        if let Some(seminar) = &mut self.seminar {
            // Ablauf und Netzereignisse sind unabhängig vom gerade sichtbaren Tab.
            seminar.tick();
            ui.ctx().request_repaint_after(Duration::from_secs(1));
            let active = seminar
                .state
                .remote
                .grants
                .iter()
                .any(|g| matches!(g.state, pi_net::remote::GrantState::Active { .. }));
            if active {
                ui.painter().rect_stroke(
                    ui.max_rect().shrink(1.0),
                    0.0,
                    egui::Stroke::new(2.0, Color32::from_rgb(0x4f, 0xb8, 0xd8)),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if let Some(seminar) = &mut self.seminar {
            // SlotHub hat bereits gesendet: ausschließlich markieren, NICHT erneut senden.
            for prompt in seminar.marked_prompts.drain(..) {
                if let Some(session) = self.sessions.iter().find(|s| {
                    !s.local && s.id == prompt.session.cwd && s.title == prompt.session.label
                }) {
                    self.chat_by_session
                        .entry(session.id.clone())
                        .or_default()
                        .push(ChatMessage::new(prompt.von.label(), prompt.text));
                }
            }
        }
        if let Some(id) = self.pairing_login.clone() {
            if self.pending == 0
                && self
                    .pairing
                    .as_ref()
                    .is_some_and(|p| p.expires > Instant::now())
                && self
                    .pairing_poll
                    .is_none_or(|t| t.elapsed() > Duration::from_secs(2))
            {
                self.pairing_poll = Some(Instant::now());
                self.call(Request::PairStatus(id));
            }
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
        if self.connected
            && self
                .last_event
                .is_some_and(|t| t.elapsed() > Duration::from_secs(45))
        {
            self.connected = false;
            self.error = Some("Die Verbindung zu R2 ist veraltet. Prüfe den letzten Auftrag, bevor du erneut sendest.".into());
        }
        self.empfang_tick();
        if self.aktiv_pending || self.verzeichnis_pending || self.flow.receipt.is_some() {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        if self.watch_started {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        if self.pending > 0 {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        // Keine Signalfarbe für Menüs, Cursor oder Auswahl; Magenta gehört nur „arbeitet“.
        ui.ctx().all_styles_mut(|s| {
            s.visuals.selection.bg_fill = Color32::from_rgb(0x3c, 0x49, 0x5a);
            s.visuals.text_cursor.stroke.color = QUIET;
            // Lesbare Knopftexte auf grauen Flächen; disabled bleibt separat gedimmt.
            s.visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(0xf0, 0xf2, 0xf5);
            s.visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
            s.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
        });
        let ctx = ui.ctx().clone();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.settings = Some(Section::Account);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.hybrid_dialog {
                self.hybrid_dialog = false;
            } else if self.confirm_host.is_some() {
                self.confirm_host = None;
            } else if self.revoke.is_some() {
                self.revoke = None;
            } else if self.pairing.is_some() {
                self.pairing = None;
                self.pairing_login = None;
            } else if self.flow.stage != Stage::Start
                && self.flow.stage != Stage::Working
                && self.flow.stage != Stage::Done
            {
                self.cancel_flow();
            } else {
                self.settings = None;
            }
        }
        egui::Panel::top("v3-menu").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("cocopi");
                ui.menu_button("Menü", |ui| {
                    if ui
                        .button(if cfg!(target_os = "macos") {
                            "Einstellungen …    ⌘,"
                        } else {
                            "Einstellungen …    Ctrl+,"
                        })
                        .clicked()
                    {
                        self.settings = Some(Section::Account);
                        ui.close();
                    }
                });
                if self.flow.stage == Stage::Working && self.flow.receipt.is_some() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Auschecken …").clicked() {
                            if let Some(request) = self.flow.preview() {
                                self.flow_call(request);
                            }
                        }
                    });
                }
                if self.demo {
                    ui.label(
                        RichText::new("VORSCHAU · erfundene Daten · keine Verbindung")
                            .small()
                            .color(QUIET),
                    );
                }
            });
        });
        egui::Panel::bottom("v3-fixed-status")
            .exact_size(34.0)
            .show(ui, |ui| {
                if ui
                    .add(egui::Button::new(self.status()).frame(false))
                    .on_hover_text("Einstellungen › Modus")
                    .clicked()
                {
                    self.settings = Some(Section::Mode);
                }
            });
        if self.flow.stage != Stage::Start && self.flow.stage != Stage::Working {
            self.flow_card(ui);
        } else if self.welcome_done && self.account.is_some() {
            self.main(ui);
        } else {
            self.welcome(ui);
        }
        self.settings_ui(&ctx);
        self.dialogs(&ctx);
        // Seminar::ui zeigt den Dialog selbst; sonst global, auch über Einstellungen.
        if !self.seminar_selected || self.settings.is_some() || self.flow.stage != Stage::Working {
            if let Some(seminar) = &mut self.seminar {
                seminar.consent_ui(ui);
            }
        }
    }
    fn welcome(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(125.0);
            egui::Frame::group(ui.style()).inner_margin(32.0).show(ui, |ui| { ui.set_max_width(540.0);
                ui.heading("Willkommen bei cocopi"); ui.add_space(20.0);
                if let Some(a) = &self.account {
                    ui.label(format!("Angemeldet als {} · {} @ R2. Dieser Rechner heißt jetzt ‚{}‘.", a.person, a.slot, a.device_name));
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.button("Name ändern").clicked() {
                            self.rename = true;
                        }
                        if ui.button("Los geht’s").clicked() {
                            self.welcome_done = true;
                        }
                    });
                    if self.rename {
                        ui.text_edit_singleline(&mut self.device_name);
                        if ui.button("Namen speichern").clicked() {
                            self.call(Request::Rename);
                            self.rename = false;
                        }
                    }
                } else {
                    ui.label("Dein Arbeitsplatz auf R2. Melde dich im Browser mit deinem UdK-Konto an.");
                    ui.add_space(18.0);
                    if ui.add_sized([330.0, 42.0], egui::Button::new("Einchecken mit UdK-Konto").fill(Color32::from_rgb(0x38, 0x5a, 0x6d))).clicked() {
                        if let Some(request) = self.flow.begin(CheckinMethod::Udk) { self.flow_call(request); }
                    }
                    ui.add_space(12.0);
                    if ui.small_button("Ich habe einen Einladungscode").clicked() { self.guest = !self.guest; }
                    if self.guest {
                        ui.add_space(12.0);
                        ui.add(egui::TextEdit::singleline(&mut self.invitation).hint_text("Einladungscode der Seminarleitung").desired_width(470.0));
                        if ui.add_enabled(!self.invitation.trim().is_empty(), egui::Button::new("Als Gast einchecken")).clicked() {
                            if let Some(request) = self.flow.begin(CheckinMethod::Guest(self.invitation.trim().into())) { self.flow_call(request); }
                        }
                    }
                    ui.add_space(14.0);
                    ui.label(RichText::new("Kein lokaler Agent. Dateien gehen nur mit deiner Bestätigung nach R2.").color(QUIET));
                }
                if let Some(e) = &self.error { ui.add_space(20.0); ui.label(e); }
            });
        });
    }
    fn main(&mut self, ui: &mut Ui) {
        if !self.seminar_selected {
            if let Some(seminar) = &mut self.seminar {
                seminar.remote_bar(ui);
            }
        }
        egui::Panel::left("v3-sessions")
            .exact_size(360.0)
            .show(ui, |ui| {
                ui.heading("Sessions · R2");
                ui.add_space(12.0);
                if ui.button("Neue Session").clicked() {
                    self.call(Request::Open(String::new()));
                }
                let mut action = None;
                let mut pin = None;
                // Stabile Sortierung: angeheftet zuerst, sonst Hub-Reihenfolge (zuletzt benutzt).
                let mut sessions = self.sessions.iter().collect::<Vec<_>>();
                sessions.sort_by_key(|s| !s.pinned);
                for s in sessions {
                    ui.add_space(8.0);
                    let selected = self.selected.as_ref() == Some(&s.id);
                    let fill = if selected {
                        Color32::from_rgb(0x32, 0x3e, 0x4d)
                    } else {
                        Color32::TRANSPARENT
                    };
                    egui::Frame::new()
                        .fill(fill)
                        .corner_radius(6.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.menu_button("…", |ui| {
                                            if ui
                                                .button(if s.pinned {
                                                    "Nicht mehr anheften"
                                                } else {
                                                    "Anheften"
                                                })
                                                .clicked()
                                            {
                                                pin = Some(s.id.clone());
                                                ui.close();
                                            }
                                            if ui.button("Ruhen lassen").clicked() {
                                                action = Some(Request::Rest(s.id.clone()));
                                                ui.close();
                                            }
                                        });
                                        if s.state != SessionState::Ready {
                                            ui.label(
                                                RichText::new(s.state.text())
                                                    .small()
                                                    .color(s.state.color()),
                                            );
                                        }
                                        let title = if s.local {
                                            format!("{} · Dieser Rechner", s.title)
                                        } else {
                                            s.title.clone()
                                        };
                                        if ui
                                            .add_sized(
                                                [ui.available_width(), 28.0],
                                                egui::Button::new(title).frame(false).truncate(),
                                            )
                                            .on_hover_text(&s.title)
                                            .clicked()
                                        {
                                            self.selected = Some(s.id.clone());
                                            action = Some(Request::Open(s.id.clone()));
                                        }
                                    },
                                );
                            });
                        });
                }
                if let Some(id) = pin {
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                        s.pinned = !s.pinned;
                    }
                }
                if let Some(a) = action {
                    self.call(a);
                }
            });
        if !self.seminar_selected {
            egui::Panel::bottom("v3-input").show(ui, |ui| {
                if !self.connected && !self.demo && ui.button("Verbindung erneut prüfen").clicked()
                {
                    self.start_watch();
                    self.call(Request::Sessions);
                }
                if let Some(e) = &self.error {
                    ui.label(e);
                    if e.contains("Hier startet kein Agent")
                        && ui.button("Einstellungen öffnen").clicked()
                    {
                        self.settings = Some(Section::Mode);
                    }
                }
                let working = self.sessions.iter().any(|s| {
                    Some(&s.id) == self.selected.as_ref() && s.state == SessionState::Working
                });
                let text = if working { "Abbrechen" } else { "Senden" };
                ui.add_sized(
                    [ui.available_width(), 86.0],
                    egui::TextEdit::multiline(&mut self.prompt)
                        .hint_text("Nachricht an die gewählte Session auf R2 …"),
                );
                ui.horizontal(|ui| {
                    ui.label("Modell: Standard");
                    if ui
                        .add_enabled(
                            self.connected && self.selected.is_some() && self.pending == 0,
                            egui::Button::new(text),
                        )
                        .clicked()
                    {
                        let id = self.selected.clone().unwrap();
                        if working {
                            self.call(Request::Stop(id));
                        } else if !self.prompt.trim().is_empty() {
                            self.call(Request::Prompt(id, self.prompt.clone()));
                        }
                    }
                    ui.label(RichText::new("Verbrauch: unbekannt").small().color(QUIET));
                });
            });
        }
        ui.horizontal(|ui| {
            ui.heading("Seminarprojekt");
            if ui
                .selectable_label(!self.files && !self.seminar_selected, "Chat")
                .clicked()
            {
                self.files = false;
                self.seminar_selected = false;
            }
            if ui
                .selectable_label(self.files && !self.seminar_selected, "Dateien")
                .clicked()
            {
                self.files = true;
                self.seminar_selected = false;
            }
            if ui
                .selectable_label(self.seminar_selected, "Seminar")
                .clicked()
            {
                self.seminar_selected = true;
            }
        });
        ui.separator();
        if self.seminar_selected {
            self.seminar_tab(ui);
        } else if self.files {
            ui.label("Dateiübertragung wird in I2 freigeschaltet. Es werden keine lokalen Dateien automatisch gelesen oder übertragen.");
            for text in [
                "Nach R2 übertragen",
                "Änderungen prüfen",
                "Ergebnisse auf den Mac holen",
            ] {
                ui.add_enabled(false, egui::Button::new(text));
            }
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, message) in self.messages.iter().enumerate() {
                    chat_message(ui, message, ("v3-demo", i));
                }
                let messages = self.selected.as_ref().and_then(|id| self.chat_by_session.get(id));
                if let Some(messages) = messages {
                    for (i, message) in messages.iter().enumerate() {
                        chat_message(ui, message, ("v3-message", &self.selected, i));
                    }
                }
                if let Some(text) = self.selected.as_ref().and_then(|id| self.streaming.get(id)) {
                    chat_message(ui, &ChatMessage::new("cocopi · R2", text), "v3-streaming");
                }
                if self.messages.is_empty() && messages.is_none_or(|m| m.is_empty()) { ui.label("Wähle eine Session. Ohne Verbindung wird kein lokaler Ersatzagent gestartet."); }
            });
        }
    }
    fn settings_ui(&mut self, ctx: &egui::Context) {
        let Some(section) = self.settings else { return };
        let mut open = true;
        egui::Window::new("Einstellungen").id(egui::Id::new("v3-settings")).open(&mut open).default_width(740.0).anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (s, label) in SECTIONS {
                    if ui.selectable_label(s == section, label).clicked() {
                        self.settings = Some(s);
                        if s == Section::Devices && self.account.is_some() {
                            self.call(Request::Devices);
                        }
                    }
                }
            });
            ui.separator();
            ui.add_space(18.0);
            match section {
                Section::Mode => { ui.heading("Wo arbeitet der Agent?"); ui.add_space(18.0);
                    if ui.radio(self.mode == Mode::RemoteOnly, "Nur auf R2 (empfohlen)").clicked() { self.switch(Mode::RemoteOnly); }
                    ui.label("Auf diesem Rechner wird kein Agent gestartet. Deine Arbeit läuft auf dem Seminarserver."); ui.add_space(18.0);
                    if ui.radio(self.mode == Mode::Hybrid, "Auch auf diesem Rechner (Hybrid) …").clicked() && self.mode != Mode::Hybrid { self.hybrid_dialog = true; self.hybrid_focus = true; }
                    ui.label("Für Fortgeschrittene. Dafür muss pi auf diesem Rechner installiert sein.");
                    ui.add_space(18.0); ui.label("Ein Wechsel startet und beendet keine Session. Offline wird niemals auf diesen Rechner ausgewichen.");
                    ui.label("I1: Lokale Hybrid-Sessions sind in dieser Oberfläche noch nicht angebunden.");
                }
                Section::Devices => { ui.heading("Deine Geräte"); let mut revoke = None;
                    for d in &self.devices {
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            let own = self.account.as_ref().is_some_and(|a| a.device_id == d.id);
                            ui.label(format!("{}{} · {} · zuletzt aktiv {}", d.name, if own { " (dieser Rechner)" } else { "" }, d.kind, d.last_seen));
                            if !own && ui.button("Abmelden …").clicked() {
                                revoke = Some(d.clone());
                            }
                            if own { ui.label("bis zum Check-out"); }
                        });
                    }
                    if let Some(d) = revoke { self.revoke = Some(d); }
                    ui.add_space(20.0); if ui.add_enabled(self.account.is_some(), egui::Button::new("Handy koppeln")).clicked() { self.call(Request::Pair); }
                    ui.label(RichText::new("I1: Geräteschlüssel lokal als Datei (0600); Systemschlüsselbund folgt.").small().color(QUIET));
                }
                Section::Account => {
                    ui.heading("Konto");
                    if let Some(a) = &self.account {
                        ui.label(format!("{} · {} @ R2", a.person, a.slot));
                        ui.label("Zum Beenden dieses Arbeitsplatzes nutze ‚Auschecken …‘ oben rechts. Dateien werden vorher zur Prüfung angezeigt.");
                    } else {
                        ui.label("Nicht eingecheckt. Melde dich auf der Startkarte mit deinem UdK-Konto an.");
                    }
                }
                Section::Appearance => { ui.heading("Darstellung"); ui.label("Die Oberfläche verwendet die mitgelieferten Schriften. Farben werden immer durch Zustandstext ergänzt."); }
                Section::Speech => { ui.heading("Vorlesen"); ui.label("Vorlesen ist in dieser I1-Oberfläche noch nicht angebunden. Es wird kein Hilfsmodell gestartet."); }
                Section::Advanced => { ui.heading("Verbindung"); ui.label("Bestätige die Hub-Adresse deiner Seminarleitung. Die Adresse wird nie still aus einem Link übernommen. Kein SSH-Bootstrap, Deployment oder automatischer Hub-Neustart."); ui.add(egui::TextEdit::singleline(&mut self.hub_url).hint_text("https://freigegebener-host"));
                    ui.label("Name dieses Rechners"); ui.text_edit_singleline(&mut self.device_name);
                    ui.separator();
                    ui.label("Empfang · HTTPS-Issuer (mit Seminarleitung abgleichen)");
                    ui.text_edit_singleline(&mut self.oidc_issuer);
                    ui.label("Bestätigter Empfangsschlüssel · Base64url, 32 Byte");
                    ui.text_edit_singleline(&mut self.empfang_pubkey);
                    ui.label("Eigener X25519-Archivschlüssel · öffentlicher Anteil");
                    ui.label("Der Archivschlüssel wird vor dem Check-in automatisch lokal erzeugt und sicher aufbewahrt.");
                    ui.label("Mit ihm kann nur dieser Rechner ein Abholfach öffnen. Der Empfangsschlüssel wird nie automatisch übernommen.");
                }
            }
            if let Some(e) = &self.error { ui.add_space(16.0); ui.label(e); }
            ui.add_space(18.0);
        });
        if !open {
            self.settings = None;
        }
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.hybrid_dialog {
            egui::Modal::new(egui::Id::new("v3-hybrid")).show(ctx, |ui| {
                ui.set_max_width(500.0);
                ui.heading("Auch auf diesem Rechner arbeiten?");
                ui.add_space(16.0);
                ui.label(HYBRID_TEXT);
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    let cancel = ui.button("Abbrechen");
                    if self.hybrid_focus {
                        cancel.request_focus();
                        self.hybrid_focus = false;
                    }
                    if cancel.clicked() {
                        self.hybrid_dialog = false;
                    }
                    if ui.button("Hybrid einschalten").clicked() {
                        self.switch(Mode::Hybrid);
                    }
                });
            });
        } else if let Some(host) = self.confirm_host.clone() {
            egui::Modal::new(egui::Id::new("v3-host-confirm")).show(ctx, |ui| {
                ui.set_max_width(500.0); ui.heading("Mit R2 verbinden?");
                ui.label("Prüfe, ob diese Adresse zur Einladung deiner Seminarleitung gehört. Erst nach deiner Bestätigung wird eine Verbindung aufgebaut.");
                ui.monospace(&host); ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.button("Abbrechen").clicked() { self.confirm_host = None; }
                    if ui.button("Adresse bestätigen").clicked() { self.hub_url = host; self.confirm_host = None; self.redeem(); }
                });
            });
        } else if let Some(d) = self.revoke.clone() {
            egui::Modal::new(egui::Id::new("v3-revoke")).show(ctx, |ui| {
                ui.set_max_width(500.0);
                let slot = self.account.as_ref().map(|a| a.slot.as_str()).unwrap_or("dein Konto");
                ui.label(format!("‚{}‘ abmelden? Das Gerät verliert sofort den Zugang zu {slot}. Um es wieder zu nutzen, musst du es neu koppeln.", d.name));
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if ui.button("Abbrechen").clicked() {
                        self.revoke = None;
                    }
                    if ui.button(RichText::new("Abmelden").color(Color32::from_rgb(240, 130, 110))).clicked() {
                        self.call(Request::Revoke(d.id));
                        self.revoke = None;
                    }
                });
            });
        } else if let Some((id, name, words)) = self.pair_request.clone() {
            egui::Modal::new(egui::Id::new("v3-pair-confirm")).show(ctx, |ui| {
                ui.set_max_width(500.0);
                ui.label(format!(
                    "Neues Gerät möchte sich verbinden: ‚{name}‘. Ist das dein Handy?"
                ));
                if words.is_empty() { ui.label("Prüfwort wird in I1 noch nicht übertragen (optional). Prüfe den Gerätenamen und bestätige nur dein eigenes Handy."); }
                else { ui.label(format!("Prüfwort: {words}")); }
                ui.horizontal(|ui| {
                    if ui.button("Ablehnen").clicked() {
                        self.call(Request::PairDecision(id.clone(), false));
                    }
                    if ui.button("Erlauben").clicked() {
                        self.call(Request::PairDecision(id, true));
                    }
                });
            });
        } else if let Some(pair) = &mut self.pairing {
            let left = pair
                .expires
                .saturating_duration_since(Instant::now())
                .as_secs();
            let mut cancel = false;
            let confirming = self.pairing_login.is_some();
            egui::Modal::new(egui::Id::new("v3-pair")).show(ctx, |ui| {
                ui.set_max_width(500.0);
                ui.heading(if confirming { "Bitte am Handy bestätigen" } else { "Handy koppeln" });
                if left == 0 {
                    ui.label("Der Code ist abgelaufen. Tippe am Rechner noch einmal auf ‚Handy koppeln‘.");
                } else {
                    if pair.texture.is_none() {
                        if let Ok(image) = qr_image(&pair.code.url) {
                            pair.texture = Some(ctx.load_texture("v3-pair-qr", image, egui::TextureOptions::NEAREST));
                        }
                    }
                    if let Some(texture) = &pair.texture {
                        ui.image((texture.id(), egui::vec2(280.0, 280.0)));
                    }
                    ui.label(RichText::new(&pair.code.code).monospace().size(22.0));
                    ui.add_space(12.0);
                    ui.label(if confirming {
                        "Öffne cocopi auf deinem gekoppelten Handy. Scanne diesen Code und prüfe die angezeigte Aktion. Dieser Code gilt 2 Minuten. Der Rechner wartet auf deine Bestätigung."
                    } else {
                        "Öffne cocopi auf deinem Handy und tippe auf ‚Mit Rechner koppeln‘. Scanne dann diesen Code. Er gilt 5 Minuten."
                    });
                    ui.label(format!("noch {}:{:02}", left / 60, left % 60));
                    ctx.request_repaint_after(Duration::from_secs(1));
                }
                // Immer erreichbar: auch bei Ablauf oder fehlender QR-Textur.
                let cancel_button = ui.button("Abbrechen");
                #[cfg(test)]
                ctx.data_mut(|data| data.insert_temp(egui::Id::new("v3-pair-cancel-rect"), cancel_button.rect));
                if cancel_button.clicked() {
                    cancel = true;
                }
            });
            if cancel {
                self.pairing = None;
                self.pairing_login = None;
            }
        } else if self.limit {
            egui::Modal::new(egui::Id::new("v3-limit")).show(ctx, |ui| {
                ui.set_max_width(520.0);
                ui.heading("Zwei Sessions sind offen");
                ui.add_space(16.0);
                ui.label(LIMIT_TEXT);
                ui.add_space(20.0);
                let candidate = self.sessions.iter().rev().find(|s| matches!(s.state, SessionState::Done | SessionState::Ready)).cloned();
                if let Some(s) = candidate {
                    if ui.button(format!("‚{}‘ ruhen lassen", s.title)).clicked() {
                        self.call(Request::Rest(s.id));
                        self.limit = false;
                    }
                } else {
                    ui.label("Warte, bis eine Session fertig ist. Arbeitende Sessions und offene Rückfragen werden nicht automatisch unterbrochen.");
                }
                if ui.button("Abbrechen").clicked() {
                    self.limit = false;
                }
            });
        }
    }
    /// N11-Haken: nur direkte IP-Adressen aus der eigenen bestätigten Netzruntime.
    /// Werden mit dem nächsten Aktiv-Signal über `aktiv_mit_adressen` gemeldet.
    fn eigene_adressen(&self) -> Vec<String> {
        self.seminar
            .as_ref()
            .map(|s| s.address())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|a| a.parse::<std::net::SocketAddr>().ok())
            .take(8)
            .map(|a| a.to_string())
            .collect()
    }
    fn verzeichnis_anwenden(&mut self) {
        if let (Some(seminar), Some(verzeichnis)) = (&mut self.seminar, &self.verzeichnis) {
            seminar.verzeichnis_uebernehmen(
                verzeichnis
                    .personen
                    .iter()
                    .map(|p| seminar::VerzeichnisEintrag {
                        person: p.person.clone(),
                        anzeige: p.anzeige.clone(),
                        rolle: p.rolle.clone(),
                        net_pubkey: p.net_pubkey.clone(),
                        adressen: p.adressen.clone(),
                    })
                    .collect(),
            );
        }
    }
    fn empfang_tick(&mut self) {
        if self.demo {
            return;
        }
        if !self.aktiv_pending {
            let connected = self.connected && self.flow.receipt.is_some();
            if let Some(signal) = self.aktiv_takt.next(self.aktiv_clock.elapsed(), connected) {
                self.aktiv_pending = true;
                let adressen = self.eigene_adressen();
                self.api.set_eigene_adressen(adressen);
                let (api, tx) = (self.api.clone(), self.tx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Reply::AktivCompleted(api.aktiv_signal(signal)));
                });
            }
        }
        if self.flow.receipt.is_some()
            && !self.verzeichnis_pending
            && self
                .verzeichnis_zuletzt
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
        {
            self.verzeichnis_pending = true;
            self.verzeichnis_zuletzt = Some(Instant::now());
            let (api, tx, epoch) = (self.api.clone(), self.tx.clone(), self.connection_epoch);
            std::thread::spawn(move || {
                let _ = tx.send(Reply::ConnectionEvent {
                    epoch,
                    reply: Box::new(Reply::VerzeichnisCompleted(api.seminar_verzeichnis())),
                });
            });
        }
    }
    fn seminar_tab(&mut self, ui: &mut Ui) {
        if let Some(seminar) = &mut self.seminar {
            seminar.ui(ui);
        } else {
            ui.heading("Seminar");
            ui.label("Noch nicht verbunden. Der bestätigte Geräteausweis und die Seminarverbindung werden benötigt. Es werden keine Nachrichten geteilt.");
        }
    }
    /// Ausschließlich das explizit freigegebene Hub-Ziel, nie die aktuelle Auswahl.
    #[cfg(test)]
    pub fn send_as(&mut self, session: &str, von: pi_net::seminar::Von, text: String) {
        if !self.connected || !self.sessions.iter().any(|s| s.id == session && !s.local) {
            self.error = Some("Auftrag nicht gesendet: Die freigegebene R2-Session ist nicht mehr verbunden. Es wurde kein anderes Ziel verwendet.".into());
            return;
        }
        self.call(Request::PromptAs {
            session: session.into(),
            text,
            von,
        });
    }
    pub fn scene(&mut self, scene: &str) {
        if !self.demo {
            return;
        }
        self.enabled = true;
        self.flow = Flow::default();
        self.flow_generation += 1;
        self.flow_pending = false;
        self.flow_failed = false;
        self.flow_retry = None;
        self.seminar_selected = false;
        self.seminar = None;
        self.guest = false;
        self.account = None;
        self.welcome_done = false;
        self.settings = None;
        self.error = None;
        self.hybrid_dialog = false;
        self.limit = false;
        self.pairing = None;
        if let Some(seminar) = seminar::scene(scene) {
            // Szenen bleiben synthetisch. Kein produktiver Empfangspin aus Demo-Daten.
            self.scene("v3-arbeiten-eingecheckt");
            if scene == "v3-mitsteuern-aktiv-leitung" {
                let account = Account {
                    person: "Aylin".into(),
                    slot: "account01".into(),
                    device_name: "Aylins Rechner".into(),
                    device_id: "demo-leitung".into(),
                };
                self.flow.receipt.as_mut().unwrap().account = account.clone();
                self.account = Some(account);
            }
            for prompt in &seminar.marked_prompts {
                self.chat_by_session
                    .entry("a".into())
                    .or_default()
                    .push(ChatMessage::new(prompt.von.label(), &prompt.text));
            }
            self.seminar = Some(seminar);
            self.seminar_selected = !matches!(
                scene,
                "v3-mitsteuern-aktiv-student" | "v3-mitsteuern-anfrage"
            );
            return;
        }
        if self.flow_scene(scene) {
            return;
        }
        if scene == "v3-start" {
            return;
        }
        if scene == "v3-start-fehler" {
            self.invitation = "https://r2.example.invalid/c#1.e.K7QM-2XDA-9FHT-W3NB".into();
            self.error = Some(InviteError::Expired.text());
            return;
        }
        self.account = Some(Account {
            person: "Mira".into(),
            slot: "account03".into(),
            device_name: "Miras MacBook".into(),
            device_id: "demo-desktop".into(),
        });
        self.welcome_done = true;
        self.connected = true;
        self.devices = vec![
            Device {
                id: "demo-desktop".into(),
                name: "Miras MacBook".into(),
                kind: "Rechner".into(),
                last_seen: "heute 14:05".into(),
            },
            Device {
                id: "demo-phone".into(),
                name: "Pixel 7".into(),
                kind: "Handy".into(),
                last_seen: "heute 14:02".into(),
            },
        ];
        self.sessions = vec![
            Session {
                id: "a".into(),
                title: "Seminar-Handout".into(),
                state: SessionState::Working,
                local: false,
                pinned: true,
            },
            Session {
                id: "b".into(),
                title: "Website-Entwurf".into(),
                state: SessionState::Done,
                local: false,
                pinned: false,
            },
        ];
        self.selected = Some("a".into());
        self.messages = vec![
            ChatMessage::new(
                "Mira",
                "Erstelle einen Entwurf für ein zweiseitiges Seminar-Handout. Zeige zuerst die Gliederung. Nichts veröffentlichen.",
            ),
            ChatMessage::new(
                "cocopi · R2",
                "Ich bereite die Gliederung vor und prüfe das freigegebene Material. Dateien und Ergebnisse bleiben zunächst auf R2.",
            ),
        ];
        match scene {
            "v3-modus" => self.settings = Some(Section::Mode),
            "v3-hybrid-dialog" => {
                self.settings = Some(Section::Mode);
                self.hybrid_dialog = true;
                self.hybrid_focus = true;
            }
            "v3-geraete" => self.settings = Some(Section::Devices),
            "v3-koppeln-qr" => {
                self.settings = None;
                self.pairing = Some(Pairing {
                    code: PairCode {
                        url: "https://r2.example.invalid/c#1.k.K7QM-2XDA-9FHT-W3NB".into(),
                        code: "K7QM-2XDA-9FHT-W3NB".into(),
                        valid_for: Duration::from_secs(253),
                    },
                    expires: Instant::now() + Duration::from_secs(253),
                    texture: None,
                });
            }
            "v3-grenze" => self.limit = true,
            _ => {}
        }
    }
}
fn chat_message(ui: &mut Ui, message: &ChatMessage, id: impl std::hash::Hash + std::fmt::Debug) {
    ui.add_space(12.0);
    let leader = message.speaker.ends_with(" · Seminarleitung");
    let response = egui::Frame::new().inner_margin(14.0).show(ui, |ui| {
        ui.label(RichText::new(&message.speaker).small().color(if leader {
            Color32::from_rgb(0x4f, 0xb8, 0xd8)
        } else {
            QUIET
        }));
        ui.add_space(8.0);
        md_egui::show_doc(
            ui,
            &md_egui::Theme::default(),
            &md_core::parse(&message.text),
            id,
        );
    });
    if leader {
        ui.painter().vline(
            response.response.rect.left() + 3.0,
            response.response.rect.y_range(),
            egui::Stroke::new(3.0, Color32::from_rgb(0x4f, 0xb8, 0xd8)),
        );
    }
}

/// Vier weiße Randmodule sind Teil der Textur, nicht nur UI-Abstand.
pub fn qr_image(text: &str) -> Result<egui::ColorImage, String> {
    let qr = QrCode::encode_text(text, QrCodeEcc::Medium).map_err(|e| e.to_string())?;
    let n = qr.size() + 8;
    let mut image = egui::ColorImage::filled([n as usize, n as usize], Color32::WHITE);
    for y in 0..qr.size() {
        for x in 0..qr.size() {
            if qr.get_module(x, y) {
                image[((x + 4) as usize, (y + 4) as usize)] = Color32::BLACK;
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_has_one_authoritative_mode_and_offline() {
        let mut d = Desktop::new(true);
        assert_eq!(d.status(), "☁ Nur Remote · nicht eingecheckt");
        d.scene("v3-main-remote");
        assert_eq!(
            d.status(),
            "☁ Nur Remote · Mira · account03 @ R2 · verbunden"
        );
        d.connected = false;
        d.switch(Mode::Hybrid);
        assert_eq!(d.status(), "⇄ Hybrid · Mira · account03 @ R2 · offline");
    }
    #[test]
    fn hybrid_requires_confirmation_and_preserves_sessions() {
        let mut d = Desktop::new(true);
        d.scene("v3-hybrid-dialog");
        assert_eq!(d.mode, Mode::RemoteOnly);
        let before = d.sessions.len();
        d.hybrid_dialog = false;
        assert_eq!(d.mode, Mode::RemoteOnly);
        d.switch(Mode::Hybrid);
        d.switch(Mode::RemoteOnly);
        assert_eq!(before, d.sessions.len());
        assert!(HYBRID_TEXT.contains("Zugriff auf die Dateien dieses Rechners"));
    }
    #[test]
    fn exact_a3_errors() {
        assert_eq!(
            InviteError::Expired.text(),
            "Diese Einladung ist abgelaufen. Bitte deine Seminarleitung um eine neue."
        );
        assert_eq!(
            InviteError::Used.text(),
            "Diese Einladung wurde schon benutzt. Jede Einladung gilt nur einmal. Wenn du das nicht warst, sag deiner Seminarleitung Bescheid."
        );
        assert_eq!(
            InviteError::Unknown.text(),
            "Dieser Code passt zu keiner Einladung. Prüfe die Zeichen oder öffne den Link direkt."
        );
        assert_eq!(
            InviteError::RateLimited.text(),
            "Zu viele Versuche. Warte eine Minute und versuche es dann noch einmal."
        );
    }
    fn frame(d: &mut Desktop, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0));
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ui| d.draw(ui),
        );
        out.textures_delta.clear();
        out
    }
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    fn click(d: &mut Desktop, ctx: &egui::Context, pos: egui::Pos2) {
        frame(
            d,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            d,
            ctx,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn status_click_opens_mode_and_only_settings_shortcut_is_added() {
        let ctx = egui::Context::default();
        crate::setup_ctx(&ctx);
        let mut d = Desktop::new(true);
        d.scene("v3-main-remote");
        frame(&mut d, &ctx, vec![]);
        frame(&mut d, &ctx, vec![]);
        click(&mut d, &ctx, egui::pos2(190.0, 859.0));
        assert_eq!(d.settings, Some(Section::Mode));
        frame(
            &mut d,
            &ctx,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert_eq!(d.settings, None);
        frame(
            &mut d,
            &ctx,
            vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)],
        );
        assert_eq!(d.settings, Some(Section::Account));
    }
    #[test]
    fn hybrid_escape_and_default_enter_cancel_without_switch() {
        for keypress in [egui::Key::Escape, egui::Key::Enter] {
            let ctx = egui::Context::default();
            crate::setup_ctx(&ctx);
            let mut d = Desktop::new(true);
            d.scene("v3-hybrid-dialog");
            frame(&mut d, &ctx, vec![]);
            frame(&mut d, &ctx, vec![]);
            frame(&mut d, &ctx, vec![key(keypress, egui::Modifiers::NONE)]);
            assert!(!d.hybrid_dialog, "{keypress:?} must cancel");
            assert_eq!(d.mode, Mode::RemoteOnly);
        }
    }
    #[test]
    fn eight_v3_scenes_render_without_legacy_panels() {
        for scene in [
            "v3-start",
            "v3-start-fehler",
            "v3-main-remote",
            "v3-modus",
            "v3-hybrid-dialog",
            "v3-geraete",
            "v3-koppeln-qr",
            "v3-grenze",
        ] {
            let ctx = egui::Context::default();
            crate::setup_ctx(&ctx);
            let mut d = Desktop::new(true);
            d.scene(scene);
            for _ in 0..3 {
                let out = frame(&mut d, &ctx, vec![]);
                assert!(!out.shapes.is_empty());
            }
            assert_eq!(d.pending, 0);
            assert_eq!(d.mode, Mode::RemoteOnly);
        }
    }
    #[test]
    fn seminar_scenes_are_embedded_and_leader_prompt_is_marked_in_main_chat() {
        for scene in [
            "v3-seminar-chat",
            "v3-seminar-dm",
            "v3-mitsteuern-anfrage",
            "v3-mitsteuern-aktiv-student",
            "v3-mitsteuern-aktiv-leitung",
        ] {
            let ctx = egui::Context::default();
            crate::setup_ctx(&ctx);
            let mut d = Desktop::new(true);
            d.scene(scene);
            assert!(d.seminar.is_some(), "{scene}");
            for _ in 0..3 {
                assert!(!frame(&mut d, &ctx, vec![]).shapes.is_empty());
            }
            if scene == "v3-mitsteuern-aktiv-student" {
                assert!(!d.seminar_selected);
                assert!(d.chat_by_session["a"]
                    .iter()
                    .any(|m| m.speaker == "Aylin · Seminarleitung"));
            }
            assert_eq!(d.pending, 0);
        }
    }
    #[test]
    fn leader_send_uses_explicit_session_not_selected_chat_and_checkout_clears_seminar() {
        struct Echo;
        impl DesktopApi for Echo {
            fn call(&self, request: Request) -> Reply {
                match request {
                    Request::PromptAs { session, text, von } => Reply::SentAs(session, text, von),
                    _ => panic!("wrong request"),
                }
            }
        }
        let mut d = Desktop::new(true);
        d.scene("v3-mitsteuern-aktiv-student");
        d.selected = Some("b".into());
        d.demo = false;
        d.set_api(Arc::new(Echo));
        let prompt = seminar::HubPrompt {
            session: pi_net::remote::SessionRef {
                label: "not used".into(),
                cwd: "/never/use/as/hub-id".into(),
            },
            text: "Gezielter Auftrag".into(),
            von: pi_net::seminar::Von {
                person: "Aylin".into(),
                rolle: pi_net::ausweis::Rolle::Seminarleitung,
            },
        };
        d.apply(Reply::SeminarPrompt {
            session_id: "a".into(),
            prompt,
        });
        d.apply(d.rx.recv_timeout(Duration::from_secs(2)).unwrap());
        assert_eq!(
            d.chat_by_session["a"].last().unwrap().speaker,
            "Aylin · Seminarleitung"
        );
        assert!(!d.chat_by_session.contains_key("b"));
        let before = d.pending;
        d.send_as(
            "missing",
            pi_net::seminar::Von {
                person: "Aylin".into(),
                rolle: pi_net::ausweis::Rolle::Seminarleitung,
            },
            "Nicht senden".into(),
        );
        assert_eq!(d.pending, before);
        assert!(d.error.is_some());
        d.flow.stage = Stage::Transfer(TransferKind::Checkout);
        d.apply(Reply::Flow(FlowReply::CheckedOut(checkin::ReturnSummary {
            files: 0,
            bytes: 0,
            conflicts: 0,
        })));
        assert!(d.seminar.is_none());
        assert!(d.chat_by_session.is_empty());
    }
    #[test]
    fn send_ack_preserves_newer_edit_and_neutral_ready_has_no_marker() {
        let mut d = Desktop::new(true);
        d.prompt = "Nächster Entwurf".into();
        d.apply(Reply::Sent("a".into(), "Alter Entwurf".into()));
        assert_eq!(d.prompt, "Nächster Entwurf");
        assert_eq!(SessionState::Ready.text(), "");
        d.apply(Reply::Sent("a".into(), "Nächster Entwurf".into()));
        assert!(d.prompt.is_empty());
    }
    #[test]
    fn worker_boundary_delivers_errors_without_clearing_unsent_prompt() {
        struct Mock;
        impl DesktopApi for Mock {
            fn call(&self, _: Request) -> Reply {
                Reply::Limit
            }
        }
        let mut d = Desktop::new(true);
        d.demo = false; // Konstruktion war rein; kein Profil-/Schlüsselzugriff.
        d.set_api(Arc::new(Mock));
        d.prompt = "Nicht verlieren".into();
        d.call(Request::Open(String::new()));
        assert_eq!(d.pending, 1);
        assert!(!d.limit);
        let reply = d.rx.recv_timeout(Duration::from_secs(2)).unwrap();
        d.apply(reply);
        assert_eq!(d.pending, 0);
        assert!(d.limit);
        assert_eq!(d.prompt, "Nicht verlieren");
    }
    #[test]
    fn stream_updates_do_not_duplicate_completed_message_and_keep_sessions_separate() {
        let mut d = Desktop::new(true);
        d.apply(Reply::PiEvent("a".into(), serde_json::json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hallo"}})));
        assert_eq!(d.streaming["a"], "Hallo");
        d.apply(Reply::PiEvent("a".into(), serde_json::json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Hallo Welt"}]}})));
        assert!(!d.streaming.contains_key("a"));
        assert_eq!(d.chat_by_session["a"].len(), 1);
        assert_eq!(d.chat_by_session["a"][0].speaker, "cocopi · R2");
        assert_eq!(d.chat_by_session["a"][0].text, "Hallo Welt");
        assert!(!d.chat_by_session.contains_key("b"));
    }
    #[test]
    fn heartbeat_is_not_a_command_completion() {
        let mut d = Desktop::new(true);
        d.pending = 1;
        d.connected = false;
        d.apply(Reply::Heartbeat);
        assert_eq!(d.pending, 1);
        assert!(d.connected);
    }
    #[test]
    fn qr_pixels_match_encoder_and_have_quiet_zone() {
        let text = "https://r2.example.invalid/c#1.k.K7QM-2XDA-9FHT-W3NB";
        let image = qr_image(text).unwrap();
        let qr = QrCode::encode_text(text, QrCodeEcc::Medium).unwrap();
        assert_eq!(image.size, [(qr.size() + 8) as usize; 2]);
        for y in 0..image.size[1] {
            for x in 0..image.size[0] {
                assert_eq!(
                    image[(x, y)] == Color32::BLACK,
                    qr.get_module(x as i32 - 4, y as i32 - 4)
                );
            }
        }
    }
    #[test]
    fn pairing_closes_settings_and_cancel_is_clickable_before_and_after_expiry() {
        for expired in [false, true] {
            let ctx = egui::Context::default();
            crate::setup_ctx(&ctx);
            let mut d = Desktop::new(true);
            d.scene("v3-geraete");
            d.apply(Reply::Pair(PairCode {
                url: "https://r2.example.invalid/c#1.k.K7QM-2XDA-9FHT-W3NB".into(),
                code: "K7QM-2XDA-9FHT-W3NB".into(),
                valid_for: if expired {
                    Duration::ZERO
                } else {
                    Duration::from_secs(300)
                },
            }));
            assert_eq!(d.settings, None);
            for _ in 0..5 {
                frame(&mut d, &ctx, vec![]);
            }
            frame(&mut d, &ctx, vec![]);
            let cancel = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("v3-pair-cancel-rect")))
                .expect("Abbrechen wurde angelegt")
                .center();
            click(&mut d, &ctx, cancel);
            assert!(d.pairing.is_none(), "Code abgelaufen: {expired}");
            assert_eq!(d.settings, None);
            assert_eq!(d.pending, 0);
        }
    }

    #[test]
    fn chat_speakers_are_not_part_of_markdown_body() {
        let mut d = Desktop::new(true);
        d.scene("v3-main-remote");
        assert_eq!(d.messages[0].speaker, "Mira");
        assert!(!d.messages[0].text.starts_with("Mira"));
        assert_eq!(d.messages[1].speaker, "cocopi · R2");
        d.apply(Reply::Sent("a".into(), "Ein Entwurf".into()));
        assert_eq!(d.chat_by_session["a"][0].speaker, "Du");
        assert_eq!(d.chat_by_session["a"][0].text, "Ein Entwurf");
    }

    #[test]
    fn only_working_uses_accent() {
        for s in [
            SessionState::Resting,
            SessionState::Ready,
            SessionState::NeedsYou,
            SessionState::Done,
            SessionState::Error,
        ] {
            assert_ne!(s.color(), WORKING);
        }
        assert_eq!(SessionState::Working.color(), WORKING);
    }
    #[test]
    fn i3_seminar_has_no_second_agent_input() {
        let ctx = egui::Context::default();
        crate::setup_ctx(&ctx);
        let mut d = Desktop::new(true);
        d.scene("v3-mitsteuern-aktiv-leitung");
        for _ in 0..3 {
            frame(&mut d, &ctx, vec![]);
        }
        let output = frame(&mut d, &ctx, vec![]);
        let texts = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!texts
            .iter()
            .any(|t| t.contains("Nachricht an die gewählte Session")));
        assert!(texts.contains(&"Als Seminarleitung senden"));
    }

    #[test]
    fn i3_status_uses_own_receipt_even_when_viewing_someone_else() {
        let mut d = Desktop::new(true);
        d.scene("v3-mitsteuern-aktiv-leitung");
        assert_eq!(
            d.status(),
            "☁ Nur Remote · Aylin · account01 @ R2 · eingecheckt seit 14:02"
        );
        d.account = Some(MemoryApi::receipt().account);
        assert!(
            d.status().contains("Aylin · account01"),
            "Receipt ist maßgeblich, nicht fremde Session"
        );
        d.connected = false;
        assert_eq!(
            d.status(),
            "☁ Nur Remote · Aylin · account01 @ R2 · offline"
        );
    }

    #[test]
    fn i3_queue_and_clear_unconfirmed_are_honest_rendered_states() {
        let ctx = egui::Context::default();
        crate::setup_ctx(&ctx);
        let mut d = Desktop::new(true);
        for (scene, expected) in [
            ("v3-warteschlange", "Warteschlange"),
            (
                "v3-checkout-leeren-unbestaetigt",
                "Deine Dateien sind trotzdem schon lokal.",
            ),
        ] {
            d.scene(scene);
            for _ in 0..3 {
                frame(&mut d, &ctx, vec![]);
            }
            let output = frame(&mut d, &ctx, vec![]);
            let texts = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(texts.contains(&expected), "{scene}: {texts:?}");
            assert!(!texts
                .iter()
                .any(|t| t.contains("Ein Arbeitsplatz wird für dich frei")
                    || t.ends_with("ist leer.")));
        }
    }

    #[test]
    fn i2_scenes_render_the_requested_cards_and_one_checkout_location() {
        for (scene, expected) in [
            ("v3-start", "Einchecken mit UdK-Konto"),
            ("v3-checkin", "Warte auf Anmeldung im Browser …"),
            (
                "v3-warteschlange",
                "Alle 10 Arbeitsplätze sind belegt. Du bist Nr. 2 in der Warteschlange.",
            ),
            ("v3-sync-auswahl", "Was möchtest du nach R2 mitnehmen?"),
            ("v3-arbeiten-eingecheckt", "Auschecken …"),
            ("v3-checkout-vorschau", "Auschecken und Dateien holen"),
            ("v3-checkout-fertig", "Ausgecheckt"),
            ("v3-seminar-platzhalter", "Seminar"),
            ("v3-sync-fortschritt", "Deine Auswahl geht nach R2 …"),
            ("v3-checkout-fortschritt", "Dateien kommen zurück …"),
            ("v3-abholfach", "Dateien vom letzten Mal abholen"),
        ] {
            let ctx = egui::Context::default();
            crate::setup_ctx(&ctx);
            let mut d = Desktop::new(true);
            d.scene(scene);
            for _ in 0..4 {
                frame(&mut d, &ctx, vec![]);
            }
            let output = frame(&mut d, &ctx, vec![]);
            let texts = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(texts.contains(&expected), "{scene}: {texts:?}");
            let checkouts = texts.iter().filter(|s| **s == "Auschecken …").count();
            assert_eq!(
                checkouts,
                usize::from(
                    scene == "v3-arbeiten-eingecheckt" || scene == "v3-seminar-platzhalter"
                )
            );
            assert_eq!(d.pending, 0);
            assert!(!d.flow_pending);
            if scene == "v3-arbeiten-eingecheckt" {
                assert_eq!(
                    d.status(),
                    "☁ Nur Remote · Mira · account03 @ R2 · eingecheckt seit 14:02"
                );
            }
        }
    }
    fn click_text(d: &mut Desktop, ctx: &egui::Context, text: &str) {
        for _ in 0..3 {
            frame(d, ctx, vec![]);
        }
        let output = frame(d, ctx, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text == text => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Knopf fehlt: {text}"));
        click(d, ctx, pos);
    }
    #[test]
    fn i2_ui_selection_requires_click_and_preview_cancel_does_not_checkout() {
        let ctx = egui::Context::default();
        crate::setup_ctx(&ctx);
        let mut d = Desktop::new(true);
        d.scene("v3-sync-auswahl");
        assert_eq!(d.flow.stage, Stage::Selection);
        click_text(&mut d, &ctx, "Ohne Dateien starten");
        assert_eq!(d.flow.stage, Stage::Working);
        assert!(d.flow_retry.is_none(), "kein Upload ohne Klick auf Senden");
        d.scene("v3-checkout-vorschau");
        click_text(&mut d, &ctx, "Zurück zur Arbeit");
        assert_eq!(d.flow.stage, Stage::Working);
        assert!(d.flow.receipt.is_some());
        assert!(d.flow_retry.is_none());
    }
    #[test]
    fn i3_large_checkbox_is_required_and_later_pickup_only_plans() {
        let ctx = egui::Context::default();
        crate::setup_ctx(&ctx);
        let mut d = Desktop::new(true);
        d.scene("v3-checkout-vorschau");
        assert!(d.flow.checkout().is_none());
        click_text(&mut d, &ctx, "Ergebnisse/Film.mov");
        let Some(Request::Checkout { large_files, .. }) = d.flow.checkout() else {
            panic!("Einzelbestätigung fehlt")
        };
        assert_eq!(large_files, vec!["Ergebnisse/Film.mov"]);
        d.scene("v3-abholfach");
        click_text(&mut d, &ctx, "Später abholen · weiter zur Auswahl");
        assert!(matches!(d.flow_retry, Some(Request::SyncPlan(_))));
        assert!(d.flow.receipt.as_ref().unwrap().pickup.is_some());
        d.scene("v3-checkout-konflikt");
        for _ in 0..3 {
            frame(&mut d, &ctx, vec![]);
        }
        let output = frame(&mut d, &ctx, vec![]);
        assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::epaint::Shape::Text(t) if t.galley.job.text.contains("(R2 28.10. 16-40).md"))));
    }

    #[test]
    fn i2_escape_cancels_browser_queue_upload_checkout_and_preview() {
        for scene in [
            "v3-checkin",
            "v3-warteschlange",
            "v3-sync-fortschritt",
            "v3-checkout-fortschritt",
            "v3-checkout-vorschau",
        ] {
            let ctx = egui::Context::default();
            crate::setup_ctx(&ctx);
            let mut d = Desktop::new(true);
            d.scene(scene);
            frame(
                &mut d,
                &ctx,
                vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            );
            if scene == "v3-checkout-vorschau" {
                assert_eq!(d.flow.stage, Stage::Working);
            } else {
                assert_eq!(d.flow.stage, Stage::Cancelling);
            }
            assert_ne!(d.flow.stage, Stage::Done);
        }
    }
    #[test]
    fn i2_checkout_invalidates_late_session_and_heartbeat_events() {
        let mut d = Desktop::new(true);
        d.scene("v3-checkout-fortschritt");
        let epoch = d.connection_epoch;
        d.apply(Reply::Flow(FlowReply::CheckedOut(
            checkin::MemoryApi::summary(),
        )));
        d.apply(Reply::ConnectionEvent {
            epoch,
            reply: Box::new(Reply::Sessions(vec![Session {
                id: "old".into(),
                title: "Alt".into(),
                state: SessionState::Working,
                local: false,
                pinned: false,
            }])),
        });
        d.apply(Reply::ConnectionEvent {
            epoch,
            reply: Box::new(Reply::Heartbeat),
        });
        assert!(d.sessions.is_empty());
        assert!(!d.connected);
        assert!(d.account.is_none());
    }
    #[test]
    fn i2_stale_worker_reply_after_cancel_cannot_reopen_checkin() {
        let mut d = Desktop::new(true);
        d.flow.stage = Stage::Browser;
        d.flow_generation = 4;
        d.flow_pending = true;
        d.apply(Reply::FlowCompleted {
            generation: 3,
            reply: Box::new(Reply::Flow(FlowReply::CheckedIn(
                checkin::MemoryApi::receipt(),
            ))),
        });
        assert_eq!(d.flow.stage, Stage::Browser);
        assert!(d.account.is_none());
        assert!(d.flow_pending, "altes Ergebnis beendet nicht neuen Request");
    }
    #[test]
    fn i2_worker_boundary_runs_memory_checkin_plan_and_checkout_without_filesystem() {
        let mut d = Desktop::new(true);
        d.demo = false; // reine Konstruktion, kein Profilzugriff
        d.set_api(Arc::new(checkin::MemoryApi::default()));
        let request = d.flow.begin(CheckinMethod::Guest("mock".into())).unwrap();
        d.flow_call(request);
        for _ in 0..2 {
            let reply = d.rx.recv_timeout(Duration::from_secs(2)).unwrap();
            d.apply(reply);
        }
        assert_eq!(d.flow.stage, Stage::Selection);
        assert!(d.connected);
        assert_eq!(d.pending, 0);
        let request = d.flow.preview().unwrap();
        d.flow_call(request);
        let reply = d.rx.recv_timeout(Duration::from_secs(2)).unwrap();
        d.apply(reply);
        assert_eq!(d.flow.stage, Stage::Preview);
        let request = d.flow.checkout().unwrap();
        d.flow_call(request);
        loop {
            let reply = d.rx.recv_timeout(Duration::from_secs(2)).unwrap();
            d.apply(reply);
            if d.flow.stage == Stage::Done {
                break;
            }
            let request = d.flow.poll().unwrap();
            d.flow_call(request);
        }
        assert!(d.account.is_none());
        assert!(d.sessions.is_empty());
        assert!(!d.flow_pending);
        assert_eq!(d.flow.summary.as_ref().unwrap().conflicts, 2);
    }
    #[test]
    fn revoked_returns_to_welcome() {
        let mut d = Desktop::new(true);
        d.scene("v3-main-remote");
        d.apply(Reply::Error(InviteError::Revoked));
        assert!(d.account.is_none());
        assert!(d.sessions.is_empty());
        assert_eq!(d.error.as_deref(), Some(REVOKED_TEXT));
    }
}
