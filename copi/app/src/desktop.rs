//! Seminar-Desktop: eine Oberfläche, ein Modus-Ort, keine lokalen Agentenstarts.
//! Alle Hub-Aktionen laufen über DesktopApi; Fixtures sind ausschließlich Demo/Tests.
use egui::{Color32, RichText, Ui};
use pi_backend::mode::{self, Mode};
use qrcodegen::{QrCode, QrCodeEcc};
use std::{sync::{mpsc, Arc}, time::{Duration, Instant}};

pub const HYBRID_TEXT: &str = "Dann kann cocopi hier einen Agenten starten. Er hat Zugriff auf die Dateien dieses Rechners. Laufende Sessions bleiben, wie sie sind. Du kannst jederzeit zurückschalten.";
pub const LIMIT_TEXT: &str = "Du hast schon 2 offene Sessions. Lass eine ruhen, dann kannst du diese öffnen. Es geht nichts verloren.";
pub const REVOKED_TEXT: &str = "Dieses Gerät wurde abgemeldet. Melde dich mit einer neuen Einladung oder durch Koppeln wieder an.";
const WORKING: Color32 = Color32::from_rgb(0xe4, 0x54, 0xc4);
const QUIET: Color32 = Color32::from_rgb(0xad, 0xb4, 0xbe);

#[derive(Clone, Debug, PartialEq)]
pub enum InviteError { Expired, Used, Unknown, Offline(String), RateLimited, Revoked, Other(String) }
impl InviteError {
    pub fn text(&self) -> String {
        match self {
            Self::Expired => "Diese Einladung ist abgelaufen. Bitte deine Seminarleitung um eine neue.".into(),
            Self::Used => "Diese Einladung wurde schon benutzt. Jede Einladung gilt nur einmal. Wenn du das nicht warst, sag deiner Seminarleitung Bescheid.".into(),
            Self::Unknown => "Dieser Code passt zu keiner Einladung. Prüfe die Zeichen oder öffne den Link direkt.".into(),
            Self::Offline(until) => format!("R2 ist gerade nicht erreichbar. Prüfe deine Internetverbindung. Die Einladung bleibt gültig bis {until}."),
            Self::RateLimited => "Zu viele Versuche. Warte eine Minute und versuche es dann noch einmal.".into(),
            Self::Revoked => REVOKED_TEXT.into(),
            Self::Other(s) => s.clone(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Account { pub person: String, pub slot: String, pub device_name: String, pub device_id: String }
#[derive(Clone, Debug)]
pub struct Device { pub id: String, pub name: String, pub kind: String, pub last_seen: String }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionState { Resting, Ready, Working, NeedsYou, Done, Error }
impl SessionState {
    pub fn text(self) -> &'static str { match self { Self::Resting | Self::Ready => "ruht", Self::Working => "arbeitet", Self::NeedsYou => "wartet auf dich", Self::Done => "fertig", Self::Error => "Fehler" } }
    fn color(self) -> Color32 { if self == Self::Working { WORKING } else { QUIET } }
}
#[derive(Clone, Debug)]
pub struct Session { pub id: String, pub title: String, pub state: SessionState, pub local: bool, pub pinned: bool }
#[derive(Clone, Debug)]
pub struct PairCode { pub url: String, pub code: String, pub valid_for: Duration }
#[derive(Clone, Debug)]
pub enum Request { Redeem(String), Devices, Pair, PairDecision(String, bool), Revoke(String), Rename(String), Sessions, Open(String), Rest(String), Prompt(String, String), Stop(String) }
#[derive(Clone, Debug)]
pub enum Reply { SignedIn(Account), Devices(Vec<Device>), Pair(PairCode), PairWaiting { id: String, name: String, words: String }, PairDone, Revoked(String), Sessions(Vec<Session>), Messages(Vec<String>), Ok, Limit, Error(InviteError) }
/// Impl darf blockieren: Aufrufe werden immer in einen Worker ausgelagert.
pub trait DesktopApi: Send + Sync { fn call(&self, request: Request) -> Reply; }
struct Unavailable;
impl DesktopApi for Unavailable {
    fn call(&self, _: Request) -> Reply { Reply::Error(InviteError::Other("Die Verbindung ist noch nicht eingerichtet. Bitte deine Seminarleitung um den freigegebenen Hub-Zugang.".into())) }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Section { Account, Devices, Mode, Appearance, Speech, Advanced }
const SECTIONS: [(Section, &str); 6] = [(Section::Account, "Konto"), (Section::Devices, "Geräte"), (Section::Mode, "Modus"), (Section::Appearance, "Darstellung"), (Section::Speech, "Vorlesen"), (Section::Advanced, "Erweitert")];
struct Pairing { code: PairCode, expires: Instant, texture: Option<egui::TextureHandle> }
pub struct Desktop {
    pub enabled: bool,
    demo: bool, api: Arc<dyn DesktopApi>, tx: mpsc::Sender<Reply>, rx: mpsc::Receiver<Reply>, pending: usize,
    mode: Mode, pub account: Option<Account>, connected: bool, invitation: String, welcome_done: bool,
    error: Option<String>, settings: Option<Section>, hybrid_dialog: bool, hybrid_focus: bool, devices: Vec<Device>, revoke: Option<Device>,
    pairing: Option<Pairing>, pair_request: Option<(String, String, String)>, limit: bool, sessions: Vec<Session>, selected: Option<String>, messages: Vec<String>,
    prompt: String, files: bool, rename: bool, device_name: String,
}
impl Desktop {
    pub fn new(demo: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        Self { enabled: !cfg!(test) || !demo, demo, api: Arc::new(Unavailable), tx, rx, pending: 0, mode: if demo { Mode::RemoteOnly } else { mode::current() }, account: None, connected: false, invitation: String::new(), welcome_done: false, error: None, settings: None, hybrid_dialog: false, hybrid_focus: false, devices: vec![], revoke: None, pairing: None, pair_request: None, limit: false, sessions: vec![], selected: None, messages: vec![], prompt: String::new(), files: false, rename: false, device_name: String::new() }
    }
    pub fn set_api(&mut self, api: Arc<dyn DesktopApi>) { self.api = api; }
    fn call(&mut self, request: Request) {
        if self.demo { self.error = Some("Vorschau: Es wird keine Verbindung aufgebaut und kein Agent gestartet.".into()); return; }
        self.pending += 1;
        let (api, tx) = (self.api.clone(), self.tx.clone());
        std::thread::spawn(move || { let _ = tx.send(api.call(request)); });
    }
    fn apply(&mut self, reply: Reply) {
        match reply {
            Reply::SignedIn(account) => { self.device_name = account.device_name.clone(); self.account = Some(account); self.connected = true; self.error = None; self.call(Request::Devices); self.call(Request::Sessions); }
            Reply::Devices(devices) => self.devices = devices,
            Reply::Pair(code) => self.pairing = Some(Pairing { expires: Instant::now() + code.valid_for, code, texture: None }),
            Reply::PairWaiting { id, name, words } => { self.pair_request = Some((id, name, words)); self.pairing = None; }
            Reply::PairDone => { self.pair_request = None; self.pairing = None; self.call(Request::Devices); }
            Reply::Revoked(id) => {
                self.devices.retain(|d| d.id != id);
                if self.account.as_ref().is_some_and(|a| a.device_id == id) {
                    self.apply(Reply::Error(InviteError::Revoked));
                }
            }
            Reply::Sessions(sessions) => { self.sessions = sessions; self.connected = true; }
            Reply::Messages(messages) => { self.messages = messages; self.connected = true; }
            Reply::Limit => self.limit = true,
            Reply::Error(e) => { if matches!(e, InviteError::Offline(_) | InviteError::Revoked) { self.connected = false; } if e == InviteError::Revoked { self.account = None; self.welcome_done = false; self.sessions.clear(); self.messages.clear(); } self.error = Some(e.text()); }
            Reply::Ok => self.error = None,
        }
    }
    pub fn status(&self) -> String {
        let prefix = if self.mode == Mode::Hybrid { "⇄ Hybrid" } else { "☁ Nur Remote" };
        match &self.account { Some(a) => format!("{prefix} · {} · {} @ R2 · {}", a.person, a.slot, if self.connected { "verbunden" } else { "offline" }), None => format!("{prefix} · nicht angemeldet") }
    }
    fn switch(&mut self, mode: Mode) {
        let result = if self.demo { Ok(()) } else { mode::set(mode) };
        match result { Ok(()) => { self.mode = mode; self.hybrid_dialog = false; }, Err(e) => self.error = Some(format!("Der Modus bleibt unverändert. {e}")) }
    }
    pub fn draw(&mut self, ui: &mut Ui) {
        while let Ok(reply) = self.rx.try_recv() { self.pending = self.pending.saturating_sub(1); self.apply(reply); }
        if self.pending > 0 { ui.ctx().request_repaint_after(Duration::from_millis(100)); }
        // Keine Signalfarbe für Menüs, Cursor oder Auswahl; Magenta gehört nur „arbeitet“.
        ui.ctx().all_styles_mut(|s| { s.visuals.selection.bg_fill = Color32::from_rgb(0x3c,0x49,0x5a); s.visuals.text_cursor.stroke.color = QUIET; });
        let ctx = ui.ctx().clone();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) { self.settings = Some(Section::Account); }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.hybrid_dialog { self.hybrid_dialog = false; } else if self.revoke.is_some() { self.revoke = None; } else if self.pairing.is_some() { self.pairing = None; } else { self.settings = None; }
        }
        egui::Panel::top("v3-menu").show(ui, |ui| { ui.horizontal(|ui| {
            ui.strong("cocopi");
            ui.menu_button("Menü", |ui| { if ui.button(if cfg!(target_os="macos") { "Einstellungen …    ⌘," } else { "Einstellungen …    Ctrl+," }).clicked() { self.settings = Some(Section::Account); ui.close(); } });
            if self.demo { ui.label(RichText::new("VORSCHAU · erfundene Daten · keine Verbindung").small().color(QUIET)); }
        }); });
        egui::Panel::bottom("v3-fixed-status").exact_size(34.0).show(ui, |ui| { if ui.add(egui::Button::new(self.status()).frame(false)).on_hover_text("Einstellungen › Modus").clicked() { self.settings = Some(Section::Mode); } });
        if self.welcome_done && self.account.is_some() { self.main(ui); } else { self.welcome(ui); }
        self.settings_ui(&ctx);
        self.dialogs(&ctx);
    }
    fn welcome(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(125.0);
            egui::Frame::group(ui.style()).inner_margin(32.0).show(ui, |ui| { ui.set_max_width(540.0);
                ui.heading("Willkommen bei cocopi"); ui.add_space(20.0);
                if let Some(a) = &self.account {
                    ui.label(format!("Angemeldet als {} · {} @ R2. Dieser Rechner heißt jetzt ‚{}‘.", a.person, a.slot, a.device_name));
                    ui.add_space(16.0);
                    ui.horizontal(|ui| { if ui.button("Name ändern").clicked() { self.rename = true; } if ui.button("Los geht’s").clicked() { self.welcome_done = true; } });
                    if self.rename { ui.text_edit_singleline(&mut self.device_name); if ui.button("Namen speichern").clicked() { self.call(Request::Rename(self.device_name.clone())); self.rename = false; } }
                } else {
                    ui.label("Du hast eine Einladung bekommen – als Link oder als QR-Code. Öffne den Link, oder füge ihn hier ein."); ui.add_space(18.0);
                    ui.add(egui::TextEdit::singleline(&mut self.invitation).hint_text("Einladungslink oder Code").desired_width(470.0)); ui.add_space(12.0);
                    if ui.add_enabled(self.pending == 0 && !self.invitation.trim().is_empty(), egui::Button::new("Anmelden")).clicked() { self.error = None; self.call(Request::Redeem(self.invitation.trim().into())); }
                    if self.pending > 0 { ui.horizontal(|ui| { ui.spinner(); ui.label("Einladung wird geprüft …"); }); }
                    ui.add_space(14.0); ui.label(RichText::new("Keine Einladung? Frag deine Seminarleitung.").color(QUIET));
                }
                if let Some(e) = &self.error { ui.add_space(20.0); ui.label(e); }
            });
        });
    }
    fn main(&mut self, ui: &mut Ui) {
        egui::Panel::left("v3-sessions").exact_size(270.0).show(ui, |ui| {
            ui.heading("Sessions"); ui.add_space(12.0);
            if ui.button("Neue Session").clicked() { self.call(Request::Open(String::new())); }
            let mut action = None;
            // Stabile Sortierung: angeheftet zuerst, sonst Hub-Reihenfolge (zuletzt benutzt).
            let mut sessions = self.sessions.iter().collect::<Vec<_>>(); sessions.sort_by_key(|s| !s.pinned);
            for s in sessions { ui.add_space(12.0); ui.horizontal(|ui| {
                if ui.selectable_label(self.selected.as_ref() == Some(&s.id), &s.title).clicked() { self.selected = Some(s.id.clone()); action = Some(Request::Open(s.id.clone())); }
                ui.menu_button("⋯", |ui| { if ui.button("Ruhen lassen").clicked() { action = Some(Request::Rest(s.id.clone())); ui.close(); } });
            }); ui.label(RichText::new(format!("{} · {}", s.state.text(), if s.local { "Dieser Rechner" } else { "R2" })).color(s.state.color())); }
            if let Some(a) = action { self.call(a); }
        });
        egui::Panel::bottom("v3-input").show(ui, |ui| {
            if let Some(e) = &self.error { ui.label(e); if e.contains("Hier startet kein Agent") && ui.button("Einstellungen öffnen").clicked() { self.settings = Some(Section::Mode); } }
            let working = self.sessions.iter().any(|s| Some(&s.id) == self.selected.as_ref() && s.state == SessionState::Working);
            let text = if working { "Abbrechen" } else { "Senden" };
            ui.add_sized([ui.available_width(), 86.0], egui::TextEdit::multiline(&mut self.prompt).hint_text("Nachricht an die gewählte Session auf R2 …"));
            ui.horizontal(|ui| {
                ui.label("Modell: udk2/…");
                if ui.add_enabled(self.connected && self.selected.is_some(), egui::Button::new(text)).clicked() {
                    let id = self.selected.clone().unwrap();
                    if working { self.call(Request::Stop(id)); } else if !self.prompt.trim().is_empty() { self.call(Request::Prompt(id, self.prompt.clone())); }
                }
                ui.label(RichText::new("Verbrauch: unbekannt").small().color(QUIET));
            });
        });
        ui.horizontal(|ui| { ui.heading("Seminarprojekt"); ui.selectable_value(&mut self.files, false, "Chat"); ui.selectable_value(&mut self.files, true, "Dateien"); }); ui.separator();
        if self.files {
            ui.label("Dateiübertragung wird in I2 freigeschaltet. Es werden keine lokalen Dateien automatisch gelesen oder übertragen.");
            for text in ["Nach R2 übertragen", "Änderungen prüfen", "Ergebnisse auf den Mac holen"] { ui.add_enabled(false, egui::Button::new(text)); }
        } else { egui::ScrollArea::vertical().show(ui, |ui| { for text in &self.messages { ui.add_space(18.0); ui.label(text); } if self.messages.is_empty() { ui.label("Wähle eine Session. Ohne Verbindung wird kein lokaler Ersatzagent gestartet."); } }); }
    }
    fn settings_ui(&mut self, ctx: &egui::Context) {
        let Some(section) = self.settings else { return }; let mut open = true;
        egui::Window::new("Einstellungen").id(egui::Id::new("v3-settings")).open(&mut open).default_width(740.0).anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0]).show(ctx, |ui| {
            ui.horizontal(|ui| { for (s, label) in SECTIONS { if ui.selectable_label(s == section, label).clicked() { self.settings = Some(s); if s == Section::Devices && self.account.is_some() { self.call(Request::Devices); } } } }); ui.separator(); ui.add_space(18.0);
            match section {
                Section::Mode => { ui.heading("Wo arbeitet der Agent?"); ui.add_space(18.0);
                    if ui.radio(self.mode == Mode::RemoteOnly, "Nur auf R2 (empfohlen)").clicked() { self.switch(Mode::RemoteOnly); }
                    ui.label("Auf diesem Rechner wird kein Agent gestartet. Deine Arbeit läuft auf dem Seminarserver."); ui.add_space(18.0);
                    if ui.radio(self.mode == Mode::Hybrid, "Auch auf diesem Rechner (Hybrid) …").clicked() && self.mode != Mode::Hybrid { self.hybrid_dialog = true; self.hybrid_focus = true; }
                    ui.label("Für Fortgeschrittene. Dafür muss pi auf diesem Rechner installiert sein.");
                    ui.add_space(18.0); ui.label("Ein Wechsel startet und beendet keine Session. Offline wird niemals auf diesen Rechner ausgewichen.");
                }
                Section::Devices => { ui.heading("Deine Geräte"); let mut revoke = None;
                    for d in &self.devices { ui.add_space(14.0); ui.horizontal(|ui| { let own = self.account.as_ref().is_some_and(|a| a.device_id == d.id); ui.label(format!("{}{} · {} · zuletzt aktiv {}", d.name, if own { " (dieser Rechner)" } else { "" }, d.kind, d.last_seen)); if ui.button("Abmelden …").clicked() { revoke = Some(d.clone()); } }); }
                    if let Some(d) = revoke { self.revoke = Some(d); }
                    ui.add_space(20.0); if ui.add_enabled(self.account.is_some(), egui::Button::new("Handy koppeln")).clicked() { self.call(Request::Pair); }
                    ui.label(RichText::new("I1: Geräteschlüssel lokal als Datei (0600); Systemschlüsselbund folgt.").small().color(QUIET));
                }
                Section::Account => { ui.heading("Konto"); if let Some(a) = &self.account { ui.label(format!("{} · {} @ R2", a.person, a.slot)); if ui.button("Abmelden …").clicked() { self.revoke = Some(Device { id: a.device_id.clone(), name: a.device_name.clone(), kind: "Rechner".into(), last_seen: "jetzt".into() }); } } else { ui.label("Nicht angemeldet. Löse deine persönliche Einladung auf der Startkarte ein."); } }
                Section::Appearance => { ui.heading("Darstellung"); ui.label("Die Oberfläche verwendet die mitgelieferten Schriften. Farben werden immer durch Zustandstext ergänzt."); }
                Section::Speech => { ui.heading("Vorlesen"); ui.label("Vorlesen ist in dieser I1-Oberfläche noch nicht angebunden. Es wird kein Hilfsmodell gestartet."); }
                Section::Advanced => { ui.heading("Verbindung"); ui.label("Der freigegebene Hub wird durch die Einladung bestimmt. Kein SSH-Bootstrap, Deployment oder automatischer Hub-Neustart."); }
            }
            if let Some(e) = &self.error { ui.add_space(16.0); ui.label(e); }
            ui.add_space(18.0);
        }); if !open { self.settings = None; }
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.hybrid_dialog {
            egui::Modal::new(egui::Id::new("v3-hybrid")).show(ctx, |ui| { ui.set_max_width(500.0); ui.heading("Auch auf diesem Rechner arbeiten?"); ui.add_space(16.0); ui.label(HYBRID_TEXT); ui.add_space(20.0);
                ui.horizontal(|ui| { let cancel = ui.button("Abbrechen"); if self.hybrid_focus { cancel.request_focus(); self.hybrid_focus = false; } if cancel.clicked() { self.hybrid_dialog = false; } if ui.button("Hybrid einschalten").clicked() { self.switch(Mode::Hybrid); } });
            });
        } else if let Some(d) = self.revoke.clone() {
            egui::Modal::new(egui::Id::new("v3-revoke")).show(ctx, |ui| { ui.set_max_width(500.0); let slot = self.account.as_ref().map(|a| a.slot.as_str()).unwrap_or("dein Konto"); ui.label(format!("‚{}‘ abmelden? Das Gerät verliert sofort den Zugang zu {slot}. Um es wieder zu nutzen, musst du es neu koppeln.", d.name)); ui.add_space(18.0); ui.horizontal(|ui| { if ui.button("Abbrechen").clicked() { self.revoke = None; } if ui.button(RichText::new("Abmelden").color(Color32::from_rgb(240,130,110))).clicked() { self.call(Request::Revoke(d.id)); self.revoke = None; } }); });
        } else if let Some((id, name, words)) = self.pair_request.clone() {
            egui::Modal::new(egui::Id::new("v3-pair-confirm")).show(ctx, |ui| {
                ui.set_max_width(500.0);
                ui.label(format!("Neues Gerät möchte sich verbinden: ‚{name}‘. Ist das dein Handy?"));
                ui.label(format!("Prüfwort: {words}"));
                ui.horizontal(|ui| {
                    if ui.button("Ablehnen").clicked() { self.call(Request::PairDecision(id.clone(), false)); }
                    if ui.button("Erlauben").clicked() { self.call(Request::PairDecision(id, true)); }
                });
            });
        } else if let Some(pair) = &mut self.pairing {
            let left = pair.expires.saturating_duration_since(Instant::now()).as_secs(); let mut cancel = false;
            egui::Modal::new(egui::Id::new("v3-pair")).show(ctx, |ui| { ui.set_max_width(500.0); ui.heading("Handy koppeln");
                if left == 0 { ui.label("Der Code ist abgelaufen. Tippe am Rechner noch einmal auf ‚Handy koppeln‘."); } else {
                    if pair.texture.is_none() { if let Ok(image) = qr_image(&pair.code.url) { pair.texture = Some(ctx.load_texture("v3-pair-qr", image, egui::TextureOptions::NEAREST)); } }
                    if let Some(texture) = &pair.texture { ui.image((texture.id(), egui::vec2(280.0,280.0))); }
                    ui.label(RichText::new(&pair.code.code).monospace().size(22.0)); ui.add_space(12.0);
                    ui.label("Öffne cocopi auf deinem Handy und tippe auf ‚Mit Rechner koppeln‘. Scanne dann diesen Code. Er gilt 5 Minuten."); ui.label(format!("noch {}:{:02}", left / 60, left % 60)); ctx.request_repaint_after(Duration::from_secs(1));
                } if ui.button("Abbrechen").clicked() { cancel = true; }
            }); if cancel { self.pairing = None; }
        } else if self.limit {
            egui::Modal::new(egui::Id::new("v3-limit")).show(ctx, |ui| { ui.set_max_width(520.0); ui.heading("Zwei Sessions sind offen"); ui.add_space(16.0); ui.label(LIMIT_TEXT); ui.add_space(20.0);
                let candidate = self.sessions.iter().rev().find(|s| matches!(s.state, SessionState::Done | SessionState::Ready)).cloned();
                if let Some(s) = candidate { if ui.button(format!("‚{}‘ ruhen lassen", s.title)).clicked() { self.call(Request::Rest(s.id)); self.limit = false; } } else { ui.label("Warte, bis eine Session fertig ist. Arbeitende Sessions und offene Rückfragen werden nicht automatisch unterbrochen."); }
                if ui.button("Abbrechen").clicked() { self.limit = false; }
            });
        }
    }
    pub fn scene(&mut self, scene: &str) {
        if !self.demo { return; }
        self.enabled = true;
        self.account = None; self.welcome_done = false; self.settings = None; self.error = None; self.hybrid_dialog = false; self.limit = false; self.pairing = None;
        if scene == "v3-start" { return; }
        if scene == "v3-start-fehler" { self.invitation = "https://r2.example.invalid/c#1.e.K7QM-2XDA-9FHT-W3NB".into(); self.error = Some(InviteError::Expired.text()); return; }
        self.account = Some(Account { person: "Mira".into(), slot: "account03".into(), device_name: "Miras MacBook".into(), device_id: "demo-desktop".into() }); self.welcome_done = true; self.connected = true;
        self.devices = vec![Device { id: "demo-desktop".into(), name: "Miras MacBook".into(), kind: "Rechner".into(), last_seen: "heute 14:05".into() }, Device { id: "demo-phone".into(), name: "Pixel 7".into(), kind: "Handy".into(), last_seen: "heute 14:02".into() }];
        self.sessions = vec![Session { id: "a".into(), title: "Seminar-Handout".into(), state: SessionState::Working, local: false, pinned: true }, Session { id: "b".into(), title: "Website-Entwurf".into(), state: SessionState::Done, local: false, pinned: false }]; self.selected = Some("a".into());
        self.messages = vec!["Mira\nErstelle einen Entwurf für ein zweiseitiges Seminar-Handout. Zeige zuerst die Gliederung. Nichts veröffentlichen.".into(), "cocopi · R2\nIch bereite die Gliederung vor und prüfe das freigegebene Material. Dateien und Ergebnisse bleiben zunächst auf R2.".into()];
        match scene {
            "v3-modus" => self.settings = Some(Section::Mode),
            "v3-hybrid-dialog" => { self.settings = Some(Section::Mode); self.hybrid_dialog = true; self.hybrid_focus = true; }
            "v3-geraete" => self.settings = Some(Section::Devices),
            "v3-koppeln-qr" => { self.settings = Some(Section::Devices); self.pairing = Some(Pairing { code: PairCode { url: "https://r2.example.invalid/c#1.k.K7QM-2XDA-9FHT-W3NB".into(), code: "K7QM-2XDA-9FHT-W3NB".into(), valid_for: Duration::from_secs(253) }, expires: Instant::now() + Duration::from_secs(253), texture: None }); }
            "v3-grenze" => self.limit = true,
            _ => {}
        }
    }
}
/// Vier weiße Randmodule sind Teil der Textur, nicht nur UI-Abstand.
pub fn qr_image(text: &str) -> Result<egui::ColorImage, String> {
    let qr = QrCode::encode_text(text, QrCodeEcc::Medium).map_err(|e| e.to_string())?;
    let n = qr.size() + 8; let mut image = egui::ColorImage::filled([n as usize, n as usize], Color32::WHITE);
    for y in 0..qr.size() { for x in 0..qr.size() { if qr.get_module(x,y) { image[((x+4) as usize,(y+4) as usize)] = Color32::BLACK; } } }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn status_has_one_authoritative_mode_and_offline() { let mut d = Desktop::new(true); assert_eq!(d.status(), "☁ Nur Remote · nicht angemeldet"); d.scene("v3-main-remote"); assert_eq!(d.status(), "☁ Nur Remote · Mira · account03 @ R2 · verbunden"); d.connected = false; d.switch(Mode::Hybrid); assert_eq!(d.status(), "⇄ Hybrid · Mira · account03 @ R2 · offline"); }
    #[test] fn hybrid_requires_confirmation_and_preserves_sessions() { let mut d = Desktop::new(true); d.scene("v3-hybrid-dialog"); assert_eq!(d.mode, Mode::RemoteOnly); let before = d.sessions.len(); d.hybrid_dialog = false; assert_eq!(d.mode, Mode::RemoteOnly); d.switch(Mode::Hybrid); d.switch(Mode::RemoteOnly); assert_eq!(before, d.sessions.len()); assert!(HYBRID_TEXT.contains("Zugriff auf die Dateien dieses Rechners")); }
    #[test] fn exact_a3_errors() { assert_eq!(InviteError::Expired.text(), "Diese Einladung ist abgelaufen. Bitte deine Seminarleitung um eine neue."); assert_eq!(InviteError::Used.text(), "Diese Einladung wurde schon benutzt. Jede Einladung gilt nur einmal. Wenn du das nicht warst, sag deiner Seminarleitung Bescheid."); assert_eq!(InviteError::Unknown.text(), "Dieser Code passt zu keiner Einladung. Prüfe die Zeichen oder öffne den Link direkt."); assert_eq!(InviteError::RateLimited.text(), "Zu viele Versuche. Warte eine Minute und versuche es dann noch einmal."); assert!(InviteError::Offline("morgen 12:00".into()).text().ends_with("bis morgen 12:00.")); }
    fn frame(d: &mut Desktop, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 880.0));
        let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ui| d.draw(ui));
        out.textures_delta.clear();
        out
    }
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }
    fn click(d: &mut Desktop, ctx: &egui::Context, pos: egui::Pos2) {
        frame(d, ctx, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
        frame(d, ctx, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
    }
    #[test] fn status_click_opens_mode_and_only_settings_shortcut_is_added() {
        let ctx = egui::Context::default(); crate::setup_ctx(&ctx);
        let mut d = Desktop::new(true); d.scene("v3-main-remote");
        frame(&mut d, &ctx, vec![]); frame(&mut d, &ctx, vec![]);
        click(&mut d, &ctx, egui::pos2(190.0, 859.0));
        assert_eq!(d.settings, Some(Section::Mode));
        frame(&mut d, &ctx, vec![key(egui::Key::Escape, egui::Modifiers::NONE)]);
        assert_eq!(d.settings, None);
        frame(&mut d, &ctx, vec![key(egui::Key::Comma, egui::Modifiers::COMMAND)]);
        assert_eq!(d.settings, Some(Section::Account));
    }
    #[test] fn hybrid_escape_and_default_enter_cancel_without_switch() {
        for keypress in [egui::Key::Escape, egui::Key::Enter] {
            let ctx = egui::Context::default(); crate::setup_ctx(&ctx);
            let mut d = Desktop::new(true); d.scene("v3-hybrid-dialog");
            frame(&mut d, &ctx, vec![]); frame(&mut d, &ctx, vec![]);
            frame(&mut d, &ctx, vec![key(keypress, egui::Modifiers::NONE)]);
            assert!(!d.hybrid_dialog, "{keypress:?} must cancel");
            assert_eq!(d.mode, Mode::RemoteOnly);
        }
    }
    #[test] fn eight_v3_scenes_render_without_legacy_panels() {
        for scene in ["v3-start", "v3-start-fehler", "v3-main-remote", "v3-modus", "v3-hybrid-dialog", "v3-geraete", "v3-koppeln-qr", "v3-grenze"] {
            let ctx = egui::Context::default(); crate::setup_ctx(&ctx);
            let mut d = Desktop::new(true); d.scene(scene);
            for _ in 0..3 { let out = frame(&mut d, &ctx, vec![]); assert!(!out.shapes.is_empty()); }
            assert_eq!(d.pending, 0); assert_eq!(d.mode, Mode::RemoteOnly);
        }
    }
    #[test] fn qr_pixels_match_encoder_and_have_quiet_zone() { let text = "https://r2.example.invalid/c#1.k.K7QM-2XDA-9FHT-W3NB"; let image = qr_image(text).unwrap(); let qr = QrCode::encode_text(text, QrCodeEcc::Medium).unwrap(); assert_eq!(image.size, [(qr.size()+8) as usize;2]); for y in 0..image.size[1] { for x in 0..image.size[0] { assert_eq!(image[(x,y)] == Color32::BLACK, qr.get_module(x as i32-4,y as i32-4)); } } }
    #[test] fn only_working_uses_accent() { for s in [SessionState::Resting, SessionState::Ready, SessionState::NeedsYou, SessionState::Done, SessionState::Error] { assert_ne!(s.color(), WORKING); } assert_eq!(SessionState::Working.color(), WORKING); }
    #[test] fn revoked_returns_to_welcome() { let mut d = Desktop::new(true); d.scene("v3-main-remote"); d.apply(Reply::Error(InviteError::Revoked)); assert!(d.account.is_none()); assert!(d.sessions.is_empty()); assert_eq!(d.error.as_deref(), Some(REVOKED_TEXT)); }
}
