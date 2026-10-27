//! cocopi-Laufzeit in der App: verbindet [`pi_net::store::ChatStore`] (Chats/Rollen),
//! den P2P-Transport [`pi_net::node::Node`], den Fernzugriff [`pi_net::remote::RemoteBook`],
//! Nebenagenten ([`pi_net::persona`]) und den Agent2Agent-Dirigenten ([`pi_net::a2a`]).
//!
//! Die Oberfläche (`coco_view.rs`) liest `App::coco` und meldet Absichten als [`CocoAct`];
//! **nur** [`App::coco_apply`] ändert Zustand, sendet oder startet Prozesse.
//!
//! Teilnehmer-Konvention: `Participant::DevAgent { backend: MAIN_BACKEND, instance: "" }`
//! ist die Haupt-Session von cocopi selbst (das normale Chatfenster); andere `backend`/
//! `instance`-Paare verweisen auf Instanzen aus `App::net` (lokal oder Server).
//!
//! Sicherheit (siehe `docs/COCOPI_SECURITY.md`): Empfangene Inhalte werden nie automatisch
//! ausgeführt. Ausnahmen sind ausdrücklich eingeschaltet: Agent2Agent in einem Chat (nur
//! eigene Agenten, Rundenbudget, Not-Aus) und `Control`-Fernzugriff nach Freigabe.
use std::collections::{HashMap, HashSet};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use pi_backend::instances::{Ev, Instances, Spec, Subscription, Tagged};
use pi_net::a2a::{Conductor, Next};
use pi_net::model::*;
use pi_net::persona::Persona;
use pi_net::remote::{GrantState, MirrorItem, RemoteBook, RemoteEffect, RemoteFrame, RemoteScope, SessionRef, Side};
use pi_net::store::{Applied, ChatStore, Contact};
use pi_net::wire::{Envelope, Payload};
use serde_json::{json, Value};

use crate::app::{App, Item, ToastKind};

/// Pseudo-Backend für die Haupt-Session.
pub const MAIN_BACKEND: &str = "@main";
/// Höchstdauer eines Agentenzugs, danach gilt er als gescheitert.
const TURN_TIMEOUT: Duration = Duration::from_secs(600);
/// Nicht zustellbare Umschläge, solange das Netz aus ist (danach verworfen).
const MAX_UNSENT: usize = 500;
/// Spiegel: höchstens so viele Zeichen je Eintrag.
const MIRROR_TEXT: usize = 2000;

/// Was der Hauptbereich zeigt, wenn nicht die normale Agenten-Session.
#[derive(Clone, Debug, PartialEq)]
pub enum CocoMain {
    /// Chat (Gruppe/DM) im Hauptbereich; die normale Eingabe schreibt in diesen Chat.
    Chat(ChatId),
    /// Leitung sieht die freigegebene Session eines Teilnehmenden (Fernzugriff, `req`).
    Remote(String),
}

/// Absichten aus der Oberfläche. Werden nach dem Zeichnen angewandt.
#[derive(Clone, Debug)]
pub enum CocoAct {
    // Netz/Identität
    StartNode,
    StopNode,
    SetMyName(String),
    AddContact { addr: String, name: String },
    VerifyContact(PeerId, bool),
    BlockContact(PeerId, bool),
    RemoveContact(PeerId),
    // Räume/Rollen
    CreateSpace { name: String, kind: SpaceKind },
    SetStep { space: SpaceId, step: usize },
    // Chats
    Open(Option<CocoMain>),
    CreateGroup { title: String, space: Option<SpaceId>, members: Vec<Member> },
    OpenDirect(Member),
    Post { chat: ChatId, body: Body },
    SetMembers { chat: ChatId, members: Vec<Member> },
    Leave(ChatId),
    Archive(ChatId, bool),
    Mute(ChatId, bool),
    Pin(ChatId, bool),
    DeleteLocal(ChatId),
    AcceptInvite(ChatId),
    DeclineInvite(ChatId),
    /// Prompt-Karte nach Bestätigung in die normale Eingabe übernehmen (nicht senden).
    AdoptPrompt(String),
    // Agent2Agent
    SetA2a { chat: ChatId, policy: pi_net::a2a::A2aPolicy },
    A2aResume(ChatId),
    A2aStop(ChatId),
    A2aApprove(ChatId),
    A2aReject(ChatId),
    // Personas
    SavePersona(Persona),
    /// Eigene Persona-Datei löschen (mitgelieferte Original erscheint wieder).
    DeletePersona(PersonaId),
    /// Persona-Ordner neu einlesen (nach Änderungen im Editor).
    ReloadPersonas,
    // Fernzugriff
    RemoteRequest { peer: PeerId, space: SpaceId, scope: RemoteScope, minutes: u32, reason: String },
    /// Host: Anfrage annehmen und dabei die geteilte Session wählen (`Participant::DevAgent`).
    RemoteAccept { req: String, scope: RemoteScope, minutes: u32, session: Participant },
    RemoteEnd(String),
    /// Viewer: Vorschlag/Eingabe an die freigegebene Session.
    RemoteInput { req: String, text: String },
    /// Host: Vorschlag der Leitung in die eigene Eingabe übernehmen (nicht senden).
    RemoteAdopt { req: String, idx: usize },
    RemoteDismiss { req: String, idx: usize },
}

/// Agentenzug, der auf Freigabe wartet (`approve_each`).
#[derive(Clone, Debug)]
pub struct Approval {
    pub chat: ChatId,
    pub agent: Participant,
    pub prompt: String,
}

/// Laufender Agentenzug.
struct Turn {
    chat: ChatId,
    started: Instant,
    /// Nur Haupt-Session: Verlaufsindex beim Senden und ob „arbeitet“ schon gesehen wurde.
    main_idx: usize,
    seen_busy: bool,
    /// Instanzen: letzter vollständiger Assistententext dieses Zugs.
    reply: String,
}

/// Abonnement einer Instanz (Persona oder Instanz aus `App::net`), das cocopi selbst hält.
struct AgentLink {
    api: Arc<dyn Instances>,
    inst: String,
    live: bool,
    _sub: Subscription,
}

/// Zustand der cocopi-Erweiterung.
pub struct Coco {
    pub store: ChatStore,
    pub node: Option<pi_net::node::Node>,
    /// Eigene Kontaktadresse (sobald der Transport läuft).
    pub address: String,
    pub book: RemoteBook,
    pub personas: Vec<Persona>,
    /// Fehlerhafte Persona-Dateien (Dateiname, Fehler).
    pub persona_errors: Vec<(String, String)>,
    pub main: Option<CocoMain>,
    /// Kurzstatus für die Leiste (keine Geheimnisse).
    pub status: String,
    pub demo: bool,
    pub ui: crate::coco_view::CocoUi,
    /// Agentenzüge, die auf Freigabe warten.
    pub approvals: Vec<Approval>,
    /// Host: welche eigene Session je Freigabe geteilt wird.
    pub shared: HashMap<String, Participant>,
    /// Gerade arbeitende eigene Agenten (für Anzeige „schreibt …“).
    pub speaking: Vec<(ChatId, Participant)>,
    identity: Option<pi_net::identity::Identity>,
    turns: HashMap<Participant, Turn>,
    links: HashMap<Participant, AgentLink>,
    personas_api: Option<Arc<dyn Instances>>,
    /// Zuletzt vom Dirigenten bewertete Nachricht je Chat.
    handled: HashMap<ChatId, MsgId>,
    unsent: Vec<(PeerId, Payload)>,
    tx: mpsc::Sender<(Participant, Tagged)>,
    rx: mpsc::Receiver<(Participant, Tagged)>,
    ctx: Option<egui::Context>,
    dirty: bool,
    saved_at: Option<Instant>,
    tick: Option<Instant>,
    /// Host: Spiegelstand je Freigabe (Folgenummer, Prüfsumme des letzten Pakets).
    mirror_seq: HashMap<String, (u64, u64)>,
    loaded: bool,
}

impl Coco {
    pub fn new(demo: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        Coco {
            store: ChatStore::new(PeerId::default(), ""),
            node: None,
            address: String::new(),
            book: RemoteBook::default(),
            personas: pi_net::persona::builtins(),
            persona_errors: Vec::new(),
            main: None,
            status: String::new(),
            demo,
            ui: Default::default(),
            approvals: Vec::new(),
            shared: HashMap::new(),
            speaking: Vec::new(),
            identity: None,
            turns: HashMap::new(),
            links: HashMap::new(),
            personas_api: None,
            handled: HashMap::new(),
            unsent: Vec::new(),
            tx,
            rx,
            ctx: None,
            dirty: false,
            saved_at: None,
            tick: None,
            mirror_seq: HashMap::new(),
            loaded: false,
        }
    }

    /// Ordner für cocopi-Dateien (`<config>/copi/cocopi`).
    pub fn dir() -> Option<std::path::PathBuf> {
        pi_backend::copi_config_dir().map(|d| d.join("cocopi"))
    }
    pub fn persona_dir() -> Option<std::path::PathBuf> {
        pi_backend::copi_config_dir().map(|d| d.join("personas"))
    }

    /// Ist der Agent gerade an einem Zug?
    pub fn is_speaking(&self, who: &Participant) -> bool {
        self.turns.contains_key(who)
    }

    /// Identität und gespeicherte Chats laden (nur lokal, kein Netz). Einmalig.
    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if self.demo {
            return;
        }
        let Some(dir) = Self::dir() else {
            self.status = "Kein Konfigurationsordner – cocopi bleibt aus.".into();
            return;
        };
        match pi_net::identity::Identity::load_or_create(&dir.join("identity.key")) {
            Ok(id) => {
                let me = id.peer();
                self.identity = Some(id);
                match ChatStore::load(&dir.join("chats.json")) {
                    Ok(Some(s)) if s.me == me => self.store = s,
                    Ok(Some(_)) => {
                        self.status = "Gespeicherte Chats gehören zu einem anderen Schlüssel – nicht geladen.".into();
                        self.store = ChatStore::new(me, "");
                    }
                    Ok(None) => self.store = ChatStore::new(me, ""),
                    Err(e) => {
                        self.status = format!("Chats nicht lesbar: {e}");
                        self.store = ChatStore::new(me, "");
                    }
                }
                if self.store.my_name.is_empty() {
                    self.store.my_name = std::env::var("USER").unwrap_or_else(|_| "Ich".into());
                }
            }
            Err(e) => self.status = format!("Geräteschlüssel: {e}"),
        }
        if let Some(pd) = Self::persona_dir() {
            let (p, errs) = pi_net::persona::load_all(&pd);
            if !p.is_empty() {
                self.personas = p;
            }
            self.persona_errors = errs;
        }
    }

    fn save_now(&mut self) {
        self.dirty = false;
        self.saved_at = Some(Instant::now());
        if self.demo || self.identity.is_none() {
            return;
        }
        if let Some(dir) = Self::dir() {
            if let Err(e) = self.store.save(&dir.join("chats.json")) {
                self.status = format!("Speichern fehlgeschlagen: {e}");
            }
        }
    }

    /// Umschläge zustellen (bzw. bis zum Netzstart vormerken).
    fn send_all(&mut self, out: Vec<(PeerId, Payload)>) {
        for (to, p) in out {
            self.send_one(to, p);
        }
    }

    fn send_one(&mut self, to: PeerId, p: Payload) {
        if to == self.store.me {
            return;
        }
        let tag = match &p {
            Payload::Post { msg, .. } => Some(msg.id.clone()),
            _ => None,
        };
        match &self.node {
            Some(n) => {
                if let Err(e) = n.send(pi_net::node::Outgoing { to: to.clone(), env: Envelope::new(p), tag: tag.clone() }) {
                    self.fail_delivery(&tag, &e);
                }
            }
            None => {
                // Fernzugriff und Spiegel nicht vormerken (zeitkritisch).
                if matches!(p, Payload::RemoteFrame { .. } | Payload::RemoteInput { .. }) {
                    return;
                }
                if self.unsent.len() >= MAX_UNSENT {
                    self.unsent.remove(0);
                }
                self.unsent.push((to, p));
            }
        }
    }

    fn fail_delivery(&mut self, tag: &Option<MsgId>, err: &str) {
        let Some(id) = tag else { return };
        if let Some(c) = self.store.chats.iter().find(|c| c.messages.iter().any(|m| m.id == *id)).map(|c| c.id.clone()) {
            self.store.set_delivery(&c, id, Delivery::Failed(err.to_string()));
            self.dirty = true;
        }
    }

    fn set_allowlist(&self) {
        if let Some(n) = &self.node {
            n.set_allowed(self.store.contacts.iter().filter(|c| !c.blocked && !c.addr.is_empty()).map(|c| (c.peer.clone(), c.addr.clone())).collect());
        }
    }

    fn update_status(&mut self) {
        let n = self.store.contacts.iter().filter(|c| !c.blocked).count();
        self.status = match &self.node {
            Some(_) => format!("Netz an · {n} Kontakte"),
            None => "Netz aus".into(),
        };
    }
}

/// Zeichenkette auf `n` Zeichen kürzen (UTF-8-sicher).
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n).collect();
        t.push('…');
        t
    }
}

fn home_short(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && p.starts_with(&h) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

/// Text einer Antwort aus einem pi-Ereignis (`message_end` des Assistenten).
fn assistant_text(v: &Value) -> Option<String> {
    if v["type"] != "message_end" || v["message"]["role"] != "assistant" {
        return None;
    }
    let t = crate::commands::text_of(&v["message"]["content"]);
    (!t.trim().is_empty()).then_some(t)
}

fn checksum(items: &[MirrorItem], state: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    state.hash(&mut h);
    for i in items {
        format!("{i:?}").hash(&mut h);
    }
    h.finish()
}

impl App {
    fn coco_err(&mut self, e: impl Into<String>) {
        self.toast(e, ToastKind::Error);
    }

    fn coco_reload_personas(&mut self, dir: &std::path::Path) {
        let (list, errs) = pi_net::persona::load_all(dir);
        self.coco.personas = list;
        self.coco.persona_errors = errs;
    }

    /// Laufende Persona-Instanz beenden (außer während eines Zugs); der nächste Zug startet neu.
    fn coco_drop_persona_inst(&mut self, me: &PeerId, id: &PersonaId) {
        let who = Participant::Persona { owner: me.clone(), persona: id.clone() };
        if !self.coco.turns.contains_key(&who) {
            if let Some(l) = self.coco.links.remove(&who) {
                let _ = l.api.remove(&l.inst);
            }
        }
    }

    /// Einmal pro Frame aus `App::pump`: Transportereignisse, A2A-Züge, Spiegel, Abläufe.
    pub(crate) fn coco_pump(&mut self) {
        if self.coco.ctx.is_none() {
            self.coco.ctx = Some(self.ctx.clone());
        }
        self.coco.load();
        // Transport
        let mut evs = Vec::new();
        if let Some(n) = &self.coco.node {
            while let Ok(e) = n.events.try_recv() {
                evs.push(e);
                if evs.len() >= 200 {
                    self.ctx.request_repaint();
                    break;
                }
            }
        }
        for e in evs {
            self.coco_node_event(e);
        }
        // Agenten-Ereignisse (Personas und eigene Instanzen)
        let mut lines = Vec::new();
        while let Ok(x) = self.coco.rx.try_recv() {
            lines.push(x);
            if lines.len() >= 500 {
                self.ctx.request_repaint();
                break;
            }
        }
        for (who, t) in lines {
            self.coco_agent_event(who, t);
        }
        self.coco_main_turn();
        // Freigabevorschau für die Oberfläche (erste offene Freigabe).
        self.coco.ui.approval_preview = self.coco.approvals.first().map(|a| {
            let name = self.coco_name(&a.agent);
            (a.chat.clone(), format!("{name} möchte antworten auf:\n{}", clip(&a.prompt, 600)))
        });
        // Seltener: A2A, Ablauf, Spiegel, Speichern
        let due = self.coco.tick.is_none_or(|t| t.elapsed() >= Duration::from_millis(500));
        if due {
            self.coco.tick = Some(Instant::now());
            self.coco_conduct();
            self.coco_timeouts();
            let ended = self.coco.book.expire(now_ms());
            for (peer, p) in ended {
                if let Payload::RemoteEnd { req, .. } = &p {
                    self.coco.shared.remove(req);
                }
                self.coco.send_one(peer, p);
                self.toast("Fernzugriff abgelaufen.", ToastKind::Info);
            }
            self.coco_mirror();
            if !self.coco.book.grants.is_empty() || !self.coco.turns.is_empty() {
                self.ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
        if self.coco.dirty && self.coco.saved_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(2)) {
            self.coco.save_now();
        }
        if self.coco.dirty {
            self.ctx.request_repaint_after(Duration::from_secs(2));
        }
    }

    fn coco_node_event(&mut self, e: pi_net::node::NodeEvent) {
        use pi_net::node::NodeEvent as N;
        match e {
            N::Address(a) => self.coco.address = a,
            N::Received { from, env } => self.coco_received(from, env.payload),
            N::Delivered { tag, .. } => {
                if let Some(id) = tag {
                    if let Some(c) = self.coco.store.chats.iter().find(|c| c.messages.iter().any(|m| m.id == id)).map(|c| c.id.clone()) {
                        let pending = self.coco.store.chat(&c).and_then(|c| c.messages.iter().find(|m| m.id == id)).is_some_and(|m| matches!(m.delivery, Delivery::Pending | Delivery::Failed(_)));
                        if pending {
                            self.coco.store.set_delivery(&c, &id, Delivery::Sent);
                            self.coco.dirty = true;
                        }
                    }
                }
            }
            N::Failed { tag, error, .. } => self.coco.fail_delivery(&tag, &error),
            N::Rejected { from, why } => {
                self.toast(format!("Unbekanntes Gerät {} abgewiesen ({why}).", pi_net::identity::short(&from)), ToastKind::Warn);
            }
            N::Reachable { .. } => {}
            N::Error(e) => self.coco.status = format!("Netz: {e}"),
            N::Stopped => {
                self.coco.node = None;
                self.coco.address.clear();
                self.coco.update_status();
            }
        }
    }

    fn coco_received(&mut self, from: PeerId, p: Payload) {
        let remote = matches!(p, Payload::RemoteRequest { .. } | Payload::RemoteGrant { .. } | Payload::RemoteEnd { .. } | Payload::RemoteFrame { .. } | Payload::RemoteInput { .. });
        if remote {
            if !self.coco.store.accepts_from(&from) {
                return;
            }
            let store = &self.coco.store;
            let lead_of = |peer: &PeerId, space: &SpaceId| store.is_lead(space, peer) && store.space(space).is_some_and(|s| s.kind == SpaceKind::Seminar);
            let r = self.coco.book.apply(&from, &p, now_ms(), &lead_of);
            match r {
                Ok(Some(eff)) => self.coco_remote_effect(&from, eff),
                Ok(None) => {}
                Err(e) => self.coco.status = format!("Fernzugriff abgewiesen: {e}"),
            }
            return;
        }
        match self.coco.store.apply(&from, p) {
            Ok((applied, out)) => {
                self.coco.send_all(out);
                self.coco.dirty = true;
                match applied {
                    Applied::NewMessage { chat, msg } => self.coco_after_message(&chat, &msg),
                    Applied::InviteReceived { .. } => self.toast(format!("Einladung von {}", self.coco_name(&Participant::Human { peer: from })), ToastKind::Info),
                    Applied::StepChanged { step, .. } => self.toast(format!("Seminar: Schritt {}", step + 1), ToastKind::Info),
                    _ => {}
                }
            }
            Err(e) => self.coco.status = format!("Abgewiesen ({}): {e}", pi_net::identity::short(&from)),
        }
    }

    fn coco_remote_effect(&mut self, from: &PeerId, eff: RemoteEffect) {
        let who = self.coco_name(&Participant::Human { peer: from.clone() });
        match eff {
            RemoteEffect::Asked { .. } => self.toast(format!("{who} möchte deine Session sehen."), ToastKind::Warn),
            RemoteEffect::Granted { req } => {
                self.toast(format!("{who} hat den Fernzugriff erlaubt."), ToastKind::Info);
                self.coco.main = Some(CocoMain::Remote(req));
            }
            RemoteEffect::Ended { req, reason } => {
                self.coco.shared.remove(&req);
                self.toast(format!("Fernzugriff beendet: {reason}"), ToastKind::Info);
            }
            RemoteEffect::Mirrored { .. } => {}
            RemoteEffect::Suggested { .. } => self.toast(format!("Vorschlag von {who}"), ToastKind::Info),
            RemoteEffect::Inject { req, text } => {
                let Some(target) = self.coco.shared.get(&req).cloned() else { return };
                let text = pi_net::a2a::sanitize(&text, 4000);
                if let Err(e) = self.coco_prompt_agent(&target, &text) {
                    self.coco_err(format!("Eingabe der Leitung nicht zugestellt: {e}"));
                } else {
                    self.toast(format!("{who} hat eine Eingabe an deinen Agenten geschickt."), ToastKind::Warn);
                }
            }
        }
    }

    /// Nach jeder neuen Nachricht: Budget/Resume des Dirigenten pflegen.
    fn coco_after_message(&mut self, chat: &ChatId, msg: &MsgId) {
        let Some(c) = self.coco.store.chat_mut(chat) else { return };
        let Some(m) = c.messages.iter().find(|m| m.id == *msg) else { return };
        let agent = m.author.is_agent();
        let system = matches!(m.body, Body::System { .. });
        if let Some(p) = c.a2a.as_mut() {
            if system {
            } else if agent {
                Conductor::record_turn(p);
            } else {
                Conductor::human_resume(p);
            }
        }
        if Some(chat) == self.coco_main_chat().as_ref() {
            self.coco.store.mark_read(chat);
        }
    }

    fn coco_main_chat(&self) -> Option<ChatId> {
        match &self.coco.main {
            Some(CocoMain::Chat(c)) => Some(c.clone()),
            _ => None,
        }
    }

    // ───────────── Agent2Agent ─────────────

    fn coco_conduct(&mut self) {
        let me = self.coco.store.me.clone();
        let mut busy: Vec<Participant> = self.coco.turns.keys().cloned().collect();
        if self.busy {
            busy.push(Participant::DevAgent { owner: me.clone(), backend: MAIN_BACKEND.into(), instance: String::new() });
        }
        let ids: Vec<ChatId> = self.coco.store.chats.iter().filter(|c| !c.archived && c.a2a.as_ref().is_some_and(|p| p.enabled && !p.paused)).map(|c| c.id.clone()).collect();
        for id in ids {
            if self.coco.turns.values().any(|t| t.chat == id) || self.coco.approvals.iter().any(|a| a.chat == id) {
                continue;
            }
            let Some(chat) = self.coco.store.chat(&id) else { continue };
            let Some(last) = chat.messages.last().map(|m| m.id.clone()) else { continue };
            if self.coco.handled.get(&id) == Some(&last) {
                continue;
            }
            // Lokale Systemnotizen (z. B. „Agent2Agent eingeschaltet“) sollen keinen Zug blockieren.
            let next = if chat.messages.last().is_some_and(|m| matches!(m.body, Body::System { .. })) {
                let mut view = chat.clone();
                while view.messages.last().is_some_and(|m| matches!(m.body, Body::System { .. })) {
                    view.messages.pop();
                }
                Conductor::next(&view, &me, &busy)
            } else {
                Conductor::next(chat, &me, &busy)
            };
            match next {
                Next::Idle => {
                    self.coco.handled.insert(id, last);
                }
                Next::Speak { agent, prompt } => {
                    self.coco.handled.insert(id.clone(), last);
                    busy.push(agent.clone());
                    self.coco_start_turn(&id, agent, prompt);
                }
                Next::NeedsApproval { agent, prompt } => {
                    self.coco.handled.insert(id.clone(), last);
                    self.coco.approvals.push(Approval { chat: id, agent, prompt });
                }
                Next::BudgetExhausted => {
                    self.coco.handled.insert(id.clone(), last);
                    if let Some(p) = self.coco.store.chat_mut(&id).and_then(|c| c.a2a.as_mut()) {
                        p.paused = true;
                    }
                    self.coco.store.note(&id, "Agent2Agent pausiert: Rundenbudget erschöpft. „Fortsetzen“ startet neu.");
                    self.coco.dirty = true;
                }
            }
        }
    }

    fn coco_start_turn(&mut self, chat: &ChatId, agent: Participant, prompt: String) {
        if agent.device() != &self.coco.store.me {
            return;
        }
        match self.coco_prompt_agent(&agent, &prompt) {
            Ok(()) => {
                let main_idx = self.items.len();
                self.coco.turns.insert(agent.clone(), Turn { chat: chat.clone(), started: Instant::now(), main_idx, seen_busy: false, reply: String::new() });
                self.coco.speaking.push((chat.clone(), agent));
            }
            Err(e) => {
                let name = self.coco_name(&agent);
                self.coco.store.note(chat, &format!("{name} konnte nicht antworten: {e}"));
                if let Some(p) = self.coco.store.chat_mut(chat).and_then(|c| c.a2a.as_mut()) {
                    p.paused = true;
                }
                self.coco.dirty = true;
            }
        }
    }

    /// Prompt an einen eigenen Agenten schicken (Haupt-Session, Instanz oder Persona).
    fn coco_prompt_agent(&mut self, agent: &Participant, text: &str) -> Result<(), String> {
        if self.coco.demo {
            return Err("Demo-Modus: keine Agenten".into());
        }
        if agent.device() != &self.coco.store.me {
            return Err("fremder Agent".into());
        }
        let cmd = json!({"type": "prompt", "message": text});
        match agent {
            Participant::DevAgent { backend, .. } if backend == MAIN_BACKEND => {
                if self.busy {
                    return Err("Haupt-Session arbeitet gerade".into());
                }
                self.send(cmd);
                Ok(())
            }
            Participant::DevAgent { backend, instance, .. } => {
                let api = self.net.backend(backend).and_then(|b| b.api.clone()).ok_or("Backend nicht verbunden")?;
                self.coco_link(agent, api.clone(), instance)?;
                api.harness(instance)?.send(&cmd)
            }
            Participant::Persona { persona, .. } => {
                let p = self.coco.personas.iter().find(|p| p.id == *persona).cloned().ok_or("Persona unbekannt")?;
                let (api, inst) = self.coco_persona_inst(agent, &p)?;
                api.harness(&inst)?.send(&cmd)
            }
            Participant::Human { .. } => Err("kein Agent".into()),
        }
    }

    /// Eigene Persona-Instanz (`pi --mode rpc` mit Persona-Argumenten) sicherstellen.
    fn coco_persona_inst(&mut self, who: &Participant, p: &Persona) -> Result<(Arc<dyn Instances>, String), String> {
        if let Some(l) = self.coco.links.get(who) {
            return Ok((l.api.clone(), l.inst.clone()));
        }
        let api = self.coco.personas_api.get_or_insert_with(|| Arc::new(pi_backend::instances::LocalInstances::new("coco-persona")) as Arc<dyn Instances>).clone();
        let cwd = pi_backend::copi_data_dir().map(|d| d.join("cocopi").join("persona-cwd")).ok_or("kein Datenordner")?;
        std::fs::create_dir_all(&cwd).map_err(|e| e.to_string())?;
        let spec = Spec { name: Some(p.name.clone()), harness: Some("pi".into()), cwd: cwd.to_string_lossy().into(), session: None, args: p.pi_args() };
        let info = api.create(&spec)?;
        self.coco_link(who, api.clone(), &info.id)?;
        Ok((api, info.id))
    }

    fn coco_link(&mut self, who: &Participant, api: Arc<dyn Instances>, inst: &str) -> Result<(), String> {
        if self.coco.links.get(who).is_some_and(|l| l.inst == inst) {
            return Ok(());
        }
        let tx = std::sync::Mutex::new(self.coco.tx.clone());
        let ctx = self.ctx.clone();
        let w = who.clone();
        let sub = api.subscribe(inst, Arc::new(move |t| {
            let _ = tx.lock().map(|tx| tx.send((w.clone(), t)));
            ctx.request_repaint();
        }))?;
        // Verlauf vor `Live` gehört nicht zum Zug; bei Personas ist er leer.
        self.coco.links.insert(who.clone(), AgentLink { api, inst: inst.to_string(), live: false, _sub: sub });
        Ok(())
    }

    fn coco_agent_event(&mut self, who: Participant, t: Tagged) {
        let live = self.coco.links.get(&who).is_some_and(|l| l.live && l.inst == t.inst);
        match t.ev {
            Ev::Live => {
                if let Some(l) = self.coco.links.get_mut(&who) {
                    l.live = true;
                }
            }
            Ev::Line(line) if live => {
                let Ok(v) = serde_json::from_str::<Value>(&line) else { return };
                if let Some(text) = assistant_text(&v) {
                    if let Some(turn) = self.coco.turns.get_mut(&who) {
                        turn.reply = text;
                    }
                } else if v["type"] == "agent_settled" {
                    let aborted = v["aborted"] == true;
                    self.coco_finish_turn(&who, aborted);
                }
            }
            Ev::Exit | Ev::Gone => {
                self.coco.links.remove(&who);
                if self.coco.turns.contains_key(&who) {
                    if let Some(t) = self.coco.turns.get_mut(&who) {
                        t.reply.clear();
                    }
                    self.coco_finish_turn(&who, true);
                }
            }
            _ => {}
        }
    }

    /// Haupt-Session: Zugende über `busy` erkennen und Antwort aus dem Verlauf lesen.
    fn coco_main_turn(&mut self) {
        let me = self.coco.store.me.clone();
        let main = Participant::DevAgent { owner: me, backend: MAIN_BACKEND.into(), instance: String::new() };
        let busy = self.busy;
        let Some(t) = self.coco.turns.get_mut(&main) else { return };
        if busy {
            t.seen_busy = true;
            return;
        }
        if !t.seen_busy {
            return;
        }
        let from = t.main_idx.min(self.items.len());
        let reply = self.items[from..].iter().rev().find_map(|i| match i {
            Item::Assistant { text, .. } if !text.trim().is_empty() => Some(text.clone()),
            _ => None,
        });
        if let Some(t) = self.coco.turns.get_mut(&main) {
            t.reply = reply.unwrap_or_default();
        }
        self.coco_finish_turn(&main, false);
    }

    fn coco_finish_turn(&mut self, who: &Participant, aborted: bool) {
        let Some(turn) = self.coco.turns.remove(who) else { return };
        self.coco.speaking.retain(|(_, p)| p != who);
        let name = self.coco_name(who);
        let reply = turn.reply.trim().to_string();
        if aborted || reply.is_empty() {
            let why = if aborted { "abgebrochen" } else { "keine Antwort" };
            self.coco.store.note(&turn.chat, &format!("{name}: {why}"));
            if let Some(p) = self.coco.store.chat_mut(&turn.chat).and_then(|c| c.a2a.as_mut()) {
                p.paused = true;
            }
            self.coco.dirty = true;
            return;
        }
        let text = clip(&reply, pi_net::wire::MAX_TEXT / 2);
        match self.coco.store.post(&turn.chat, who.clone(), Body::Text { text }) {
            Ok((id, out)) => {
                self.coco.send_all(out);
                self.coco.dirty = true;
                self.coco_after_message(&turn.chat, &id);
            }
            Err(e) => {
                self.coco.store.note(&turn.chat, &format!("{name}: Antwort nicht gepostet ({e})"));
                self.coco.dirty = true;
            }
        }
    }

    fn coco_timeouts(&mut self) {
        let late: Vec<Participant> = self.coco.turns.iter().filter(|(_, t)| t.started.elapsed() > TURN_TIMEOUT).map(|(p, _)| p.clone()).collect();
        for p in late {
            self.coco_abort_agent(&p);
            if let Some(t) = self.coco.turns.get_mut(&p) {
                t.reply.clear();
            }
            self.coco_finish_turn(&p, true);
        }
    }

    fn coco_abort_agent(&mut self, who: &Participant) {
        match who {
            Participant::DevAgent { backend, .. } if backend == MAIN_BACKEND => self.send(json!({"type": "abort"})),
            _ => {
                if let Some(l) = self.coco.links.get(who) {
                    if let Ok(h) = l.api.harness(&l.inst) {
                        let _ = h.abort();
                    }
                }
            }
        }
    }

    // ───────────── Fernzugriff: Spiegeln (Host) ─────────────

    fn coco_mirror_items(&self, who: &Participant) -> Option<(Vec<MirrorItem>, String)> {
        let red = |s: &str| clip(&pi_net::remote::redact(s), MIRROR_TEXT);
        match who {
            Participant::DevAgent { backend, .. } if backend == MAIN_BACKEND => {
                let items = self.items.iter().filter_map(|i| match i {
                    Item::User(t) => Some(MirrorItem::User { text: red(t) }),
                    Item::Assistant { text, done, .. } => Some(MirrorItem::Assistant { text: red(text), done: *done }),
                    Item::Tool { name, args, err, out, .. } => Some(MirrorItem::Tool { name: name.clone(), detail: red(&clip(args, 160)), state: if *err { "Fehler".into() } else if out.is_empty() { "läuft".into() } else { "fertig".into() } }),
                    Item::Shell { title, state, .. } => Some(MirrorItem::Tool { name: "Shell".into(), detail: red(title), state: state.clone() }),
                    Item::Note { text, .. } => Some(MirrorItem::Note { text: red(text) }),
                    _ => None,
                });
                let all: Vec<MirrorItem> = items.collect();
                let state = if self.busy { "arbeitet" } else { "bereit" };
                Some((all, state.into()))
            }
            Participant::DevAgent { backend, instance, .. } => {
                let i = self.net.inst(&(backend.clone(), instance.clone()))?;
                use crate::net::NetItem as N;
                let all: Vec<MirrorItem> = i.chat.items.iter().filter_map(|it| match it {
                    N::User(t) => Some(MirrorItem::User { text: red(t) }),
                    N::Assistant { text, done, .. } => Some(MirrorItem::Assistant { text: red(text), done: *done }),
                    N::Tool { name, detail, done, err, .. } => Some(MirrorItem::Tool { name: name.clone(), detail: red(detail), state: if *err { "Fehler".into() } else if *done { "fertig".into() } else { "läuft".into() } }),
                    N::Note { text, .. } => Some(MirrorItem::Note { text: red(text) }),
                    N::Thinking(_) => None,
                }).collect();
                Some((all, format!("{:?}", i.info.state).to_lowercase()))
            }
            _ => None,
        }
    }

    fn coco_mirror(&mut self) {
        let now = now_ms();
        let hosts: Vec<(String, PeerId)> = self.coco.book.active_hosts(now).iter().map(|g| (g.req.clone(), g.peer.clone())).collect();
        self.coco.mirror_seq.retain(|req, _| hosts.iter().any(|(r, _)| r == req));
        for (req, peer) in hosts {
            let Some(who) = self.coco.shared.get(&req).cloned() else { continue };
            let Some((mut items, state)) = self.coco_mirror_items(&who) else { continue };
            let keep = pi_net::remote::MAX_FRAME_ITEMS;
            if items.len() > keep {
                items.drain(..items.len() - keep);
            }
            let sum = checksum(&items, &state);
            let (seq, last) = self.coco.mirror_seq.get(&req).copied().unwrap_or((0, 0));
            if seq > 0 && sum == last {
                continue;
            }
            // Vollständiger Stand je Paket (`reset`): robust gegen verlorene Pakete.
            loop {
                let frame = RemoteFrame { seq: seq + 1, reset: true, items: items.clone(), state: state.clone() };
                let p = Payload::RemoteFrame { req: req.clone(), frame };
                if Envelope::new(p.clone()).encode().is_ok() || items.is_empty() {
                    self.coco.send_one(peer.clone(), p);
                    break;
                }
                items.remove(0);
            }
            self.coco.mirror_seq.insert(req, (seq + 1, sum));
        }
    }

    /// Einzige Stelle, die cocopi-Zustand ändert.
    pub(crate) fn coco_apply(&mut self, act: CocoAct) {
        let me = self.coco.store.me.clone();
        match act {
            CocoAct::StartNode => {
                if self.coco.demo {
                    return self.toast("Demo-Modus: kein Netz.", ToastKind::Info);
                }
                if self.coco.node.is_some() {
                    return;
                }
                let Some(id) = &self.coco.identity else {
                    return self.coco_err("Kein Geräteschlüssel – Netz kann nicht starten.");
                };
                let node = pi_net::node::Node::start(pi_net::identity::Identity { secret: id.secret.clone() });
                self.coco.node = Some(node);
                self.coco.set_allowlist();
                let queued = std::mem::take(&mut self.coco.unsent);
                self.coco.send_all(queued);
                self.coco.update_status();
            }
            CocoAct::StopNode => {
                if let Some(n) = self.coco.node.take() {
                    n.stop();
                }
                self.coco.address.clear();
                self.coco.update_status();
            }
            CocoAct::SetMyName(n) => {
                self.coco.store.my_name = clip(pi_net::a2a::sanitize(&n, 200).trim(), pi_net::store::MAX_NAME);
                self.coco.dirty = true;
            }
            CocoAct::AddContact { addr, name } => match pi_net::node::parse_address(&addr) {
                Ok(peer) => {
                    let c = Contact { peer, name, addr: addr.trim().to_string(), verified: false, blocked: false, added: now_ms() };
                    match self.coco.store.add_contact(c) {
                        Ok(()) => {
                            self.coco.set_allowlist();
                            self.coco.update_status();
                            self.coco.dirty = true;
                            self.toast("Kontakt hinzugefügt – bitte Sicherheitswörter vergleichen.", ToastKind::Info);
                        }
                        Err(e) => self.coco_err(e),
                    }
                }
                Err(e) => self.coco_err(e),
            },
            CocoAct::VerifyContact(peer, ok) => {
                if let Some(c) = self.coco.store.contacts.iter_mut().find(|c| c.peer == peer) {
                    c.verified = ok;
                    self.coco.dirty = true;
                }
            }
            CocoAct::BlockContact(peer, on) => {
                if let Some(c) = self.coco.store.contacts.iter_mut().find(|c| c.peer == peer) {
                    c.blocked = on;
                    self.coco.dirty = true;
                }
                self.coco.set_allowlist();
                self.coco.update_status();
            }
            CocoAct::RemoveContact(peer) => {
                self.coco.store.contacts.retain(|c| c.peer != peer);
                self.coco.set_allowlist();
                self.coco.update_status();
                self.coco.dirty = true;
            }
            CocoAct::CreateSpace { name, kind } => match self.coco.store.create_space(&name, kind) {
                Ok(_) => self.coco.dirty = true,
                Err(e) => self.coco_err(e),
            },
            CocoAct::SetStep { space, step } => match self.coco.store.set_step(&space, step) {
                Ok(out) => {
                    self.coco.send_all(out);
                    self.coco.dirty = true;
                }
                Err(e) => self.coco_err(e),
            },
            CocoAct::Open(m) => {
                if let Some(CocoMain::Chat(c)) = &m {
                    self.coco.store.mark_read(c);
                    self.coco.dirty = true;
                }
                self.coco.main = m;
                self.focus_prompt = true;
            }
            CocoAct::CreateGroup { title, space, members } => match self.coco.store.create_group(&title, space, members) {
                Ok((id, out)) => {
                    self.coco.send_all(out);
                    self.coco.dirty = true;
                    self.coco.main = Some(CocoMain::Chat(id));
                    self.focus_prompt = true;
                }
                Err(e) => self.coco_err(e),
            },
            CocoAct::OpenDirect(m) => match self.coco.store.open_direct(m) {
                Ok((id, out)) => {
                    self.coco.send_all(out);
                    self.coco.dirty = true;
                    self.coco.main = Some(CocoMain::Chat(id));
                    self.focus_prompt = true;
                }
                Err(e) => self.coco_err(e),
            },
            CocoAct::Post { chat, body } => self.coco_post(&chat, body),
            CocoAct::SetMembers { chat, members } => match self.coco.store.set_members(&chat, members) {
                Ok(out) => {
                    self.coco.send_all(out);
                    self.coco.dirty = true;
                }
                Err(e) => self.coco_err(e),
            },
            CocoAct::Leave(chat) => {
                let out = self.coco.store.leave(&chat);
                self.coco.send_all(out);
                self.coco_close_if_main(&chat);
                self.coco.dirty = true;
            }
            CocoAct::Archive(chat, on) => {
                self.coco.store.set_archived(&chat, on);
                self.coco.dirty = true;
            }
            CocoAct::Mute(chat, on) => {
                self.coco.store.set_muted(&chat, on);
                self.coco.dirty = true;
            }
            CocoAct::Pin(chat, on) => {
                self.coco.store.set_pinned(&chat, on);
                self.coco.dirty = true;
            }
            CocoAct::DeleteLocal(chat) => {
                self.coco_close_if_main(&chat);
                self.coco.approvals.retain(|a| a.chat != chat);
                self.coco.store.delete_local(&chat);
                self.coco.dirty = true;
            }
            CocoAct::AcceptInvite(chat) => match self.coco.store.accept_invite(&chat) {
                Ok(out) => {
                    self.coco.send_all(out);
                    self.coco.dirty = true;
                    self.coco.main = Some(CocoMain::Chat(chat));
                }
                Err(e) => self.coco_err(e),
            },
            CocoAct::DeclineInvite(chat) => {
                let out = self.coco.store.decline_invite(&chat);
                self.coco.send_all(out);
                self.coco.dirty = true;
            }
            CocoAct::AdoptPrompt(text) => {
                self.coco.main = None;
                self.set_prompt(text);
                self.toast("In die Eingabe übernommen – prüfen und selbst senden.", ToastKind::Info);
            }
            CocoAct::SetA2a { chat, mut policy } => {
                policy.max_turns = policy.max_turns.clamp(1, pi_net::a2a::MAX_TURNS_LIMIT);
                let was_on = self.coco.store.chat(&chat).and_then(|c| c.a2a.as_ref()).is_some_and(|p| p.enabled);
                if let Some(c) = self.coco.store.chat_mut(&chat) {
                    c.a2a = Some(policy.clone());
                }
                if policy.enabled != was_on {
                    let t = if policy.enabled { "Agent2Agent eingeschaltet." } else { "Agent2Agent ausgeschaltet." };
                    self.coco.store.note(&chat, t);
                }
                if !policy.enabled {
                    self.coco_stop_chat(&chat);
                }
                self.coco.handled.remove(&chat);
                self.coco.dirty = true;
            }
            CocoAct::A2aResume(chat) => {
                if let Some(p) = self.coco.store.chat_mut(&chat).and_then(|c| c.a2a.as_mut()) {
                    Conductor::human_resume(p);
                }
                self.coco.handled.remove(&chat);
                self.coco.dirty = true;
            }
            CocoAct::A2aStop(chat) => {
                if let Some(p) = self.coco.store.chat_mut(&chat).and_then(|c| c.a2a.as_mut()) {
                    p.paused = true;
                }
                self.coco_stop_chat(&chat);
                self.coco.store.note(&chat, "Agent2Agent angehalten (Not-Aus).");
                self.coco.dirty = true;
            }
            CocoAct::A2aApprove(chat) => {
                if let Some(i) = self.coco.approvals.iter().position(|a| a.chat == chat) {
                    let a = self.coco.approvals.remove(i);
                    self.coco_start_turn(&a.chat, a.agent, a.prompt);
                }
            }
            CocoAct::A2aReject(chat) => {
                self.coco.approvals.retain(|a| a.chat != chat);
                if let Some(p) = self.coco.store.chat_mut(&chat).and_then(|c| c.a2a.as_mut()) {
                    p.paused = true;
                }
                self.coco.store.note(&chat, "Agentenzug verworfen – Agent2Agent pausiert.");
                self.coco.dirty = true;
            }
            CocoAct::SavePersona(p) => {
                let Some(dir) = Coco::persona_dir() else { return self.coco_err("Kein Konfigurationsordner.") };
                if self.coco.demo {
                    return self.toast("Demo-Modus: nicht gespeichert.", ToastKind::Info);
                }
                match pi_net::persona::save(&dir, &p) {
                    Ok(_) => {
                        self.coco_reload_personas(&dir);
                        // Laufende Instanz neu starten lassen (neuer Systemprompt).
                        self.coco_drop_persona_inst(&me, &p.id);
                        self.toast(format!("Persona „{}“ gespeichert.", p.name), ToastKind::Info);
                    }
                    Err(e) => self.coco_err(e),
                }
            }
            CocoAct::DeletePersona(id) => {
                let Some(dir) = Coco::persona_dir() else { return self.coco_err("Kein Konfigurationsordner.") };
                if self.coco.demo {
                    self.coco.personas.retain(|p| p.id != id || p.builtin);
                    return self.toast("Demo-Modus: nur in der Anzeige entfernt.", ToastKind::Info);
                }
                match pi_net::persona::delete(&dir, id.as_str()) {
                    Ok(()) => {
                        self.coco_reload_personas(&dir);
                        self.coco_drop_persona_inst(&me, &id);
                        let back = self.coco.personas.iter().any(|p| p.id == id && p.builtin);
                        self.toast(if back { "Persona gelöscht – mitgeliefertes Original ist wieder aktiv.".to_string() } else { "Persona gelöscht.".to_string() }, ToastKind::Info);
                    }
                    Err(e) => self.coco_err(e),
                }
            }
            CocoAct::ReloadPersonas => {
                let Some(dir) = Coco::persona_dir() else { return self.coco_err("Kein Konfigurationsordner.") };
                if self.coco.demo {
                    return self.toast("Demo-Modus: nichts neu geladen.", ToastKind::Info);
                }
                self.coco_reload_personas(&dir);
                // Nicht sprechende Instanzen beenden, damit neue Systemtexte greifen.
                let ids: Vec<PersonaId> = self.coco.personas.iter().map(|p| p.id.clone()).collect();
                for id in ids {
                    self.coco_drop_persona_inst(&me, &id);
                }
                let n = self.coco.personas.len();
                let e = self.coco.persona_errors.len();
                self.toast(if e == 0 { format!("{n} Nebenagenten geladen.") } else { format!("{n} Nebenagenten geladen, {e} Datei(en) fehlerhaft.") }, if e == 0 { ToastKind::Info } else { ToastKind::Warn });
            }
            CocoAct::RemoteRequest { peer, space, scope, minutes, reason } => {
                if !self.coco.store.contact(&peer).is_some_and(|c| c.verified && !c.blocked) {
                    return self.coco_err("Fernzugriff nur bei geprüften Kontakten (Sicherheitswörter).");
                }
                let lead = self.coco.store.is_lead(&space, &me) && self.coco.store.space(&space).is_some_and(|s| s.kind == SpaceKind::Seminar);
                match self.coco.book.request(peer.clone(), space, scope, minutes, &reason, lead) {
                    Ok(p) => {
                        self.coco.send_one(peer, p);
                        self.toast("Anfrage gesendet – die Person entscheidet.", ToastKind::Info);
                    }
                    Err(e) => self.coco_err(e),
                }
            }
            CocoAct::RemoteAccept { req, scope, minutes, session } => {
                if session.device() != &me || !session.is_agent() {
                    return self.coco_err("Nur eigene Sessions können geteilt werden.");
                }
                let cwd = match &session {
                    Participant::DevAgent { backend, .. } if backend == MAIN_BACKEND => home_short(&self.cwd),
                    Participant::DevAgent { backend, instance, .. } => self.net.inst(&(backend.clone(), instance.clone())).map(|i| home_short(&i.info.cwd)).unwrap_or_default(),
                    _ => String::new(),
                };
                let sref = SessionRef { label: self.coco_name(&session), cwd };
                match self.coco.book.accept(&req, scope, minutes, sref, now_ms()) {
                    Ok(p) => {
                        let peer = self.coco.book.get(&req).map(|g| g.peer.clone());
                        self.coco.shared.insert(req.clone(), session);
                        self.coco.mirror_seq.remove(&req);
                        if let Some(peer) = peer {
                            self.coco.send_one(peer, p);
                        }
                    }
                    Err(e) => self.coco_err(e),
                }
            }
            CocoAct::RemoteEnd(req) => {
                let peer = self.coco.book.get(&req).map(|g| g.peer.clone());
                if let (Some(p), Some(peer)) = (self.coco.book.end(&req, "beendet"), peer) {
                    self.coco.send_one(peer, p);
                }
                self.coco.shared.remove(&req);
                if self.coco.main == Some(CocoMain::Remote(req)) {
                    self.coco.main = None;
                }
            }
            CocoAct::RemoteInput { req, text } => self.coco_remote_input(&req, &text),
            CocoAct::RemoteAdopt { req, idx } => {
                let t = self.coco.book.grants.iter_mut().find(|g| g.req == req && g.side == Side::Host).and_then(|g| (idx < g.suggestions.len()).then(|| g.suggestions.remove(idx)));
                if let Some(t) = t {
                    self.coco.main = None;
                    self.set_prompt(t);
                    self.toast("Vorschlag in die Eingabe übernommen – prüfen und selbst senden.", ToastKind::Info);
                }
            }
            CocoAct::RemoteDismiss { req, idx } => {
                if let Some(g) = self.coco.book.grants.iter_mut().find(|g| g.req == req && g.side == Side::Host) {
                    if idx < g.suggestions.len() {
                        g.suggestions.remove(idx);
                    }
                }
            }
        }
    }

    fn coco_close_if_main(&mut self, chat: &ChatId) {
        if self.coco.main.as_ref() == Some(&CocoMain::Chat(chat.clone())) {
            self.coco.main = None;
        }
    }

    fn coco_stop_chat(&mut self, chat: &ChatId) {
        self.coco.approvals.retain(|a| a.chat != *chat);
        let running: Vec<Participant> = self.coco.turns.iter().filter(|(_, t)| t.chat == *chat).map(|(p, _)| p.clone()).collect();
        for p in running {
            self.coco_abort_agent(&p);
            self.coco.turns.remove(&p);
            self.coco.speaking.retain(|(_, s)| *s != p);
        }
    }

    fn coco_post(&mut self, chat: &ChatId, body: Body) {
        let me = self.coco.store.me_participant();
        match self.coco.store.post(chat, me, body) {
            Ok((id, out)) => {
                self.coco.send_all(out);
                self.coco.dirty = true;
                self.coco_after_message(chat, &id);
            }
            Err(e) => self.coco_err(e),
        }
    }

    fn coco_remote_input(&mut self, req: &str, text: &str) {
        let Some(g) = self.coco.book.get(req) else { return };
        let ok = g.side == Side::Viewer && matches!(g.state, GrantState::Active { expires_ms } if expires_ms > now_ms()) && g.scope != RemoteScope::View;
        if !ok {
            return self.coco_err("Keine Eingabe möglich (nur ansehen oder beendet).");
        }
        let (peer, scope) = (g.peer.clone(), g.scope);
        let text = pi_net::a2a::sanitize(text, 4000);
        self.coco.send_one(peer, Payload::RemoteInput { req: req.to_string(), text });
        let t = if scope == RemoteScope::Suggest { "Vorschlag gesendet." } else { "Eingabe gesendet." };
        self.toast(t, ToastKind::Info);
    }

    /// Normale Eingabe abfangen, wenn der Hauptbereich einen Chat oder eine Fernsitzung
    /// zeigt. `true` = verbraucht (Eingabe wird geleert).
    pub(crate) fn coco_submit(&mut self, text: &str) -> bool {
        match self.coco.main.clone() {
            Some(CocoMain::Chat(chat)) => {
                let t = text.trim();
                let body = if let Some(rest) = t.strip_prefix("/prompt ") {
                    Body::Prompt { text: rest.trim().to_string() }
                } else if let Some(rest) = t.strip_prefix("```") {
                    let (lang, code) = rest.split_once('\n').unwrap_or(("", rest));
                    Body::Code { lang: lang.trim().to_string(), text: code.trim_end().trim_end_matches("```").trim_end().to_string() }
                } else {
                    Body::Text { text: t.to_string() }
                };
                self.coco_post(&chat, body);
                true
            }
            Some(CocoMain::Remote(req)) => {
                self.coco_remote_input(&req, text);
                true
            }
            None => false,
        }
    }

    /// Anzeigename eines Teilnehmenden (Kontaktname, Persona, Instanzname, „Ich“).
    pub(crate) fn coco_name(&self, p: &Participant) -> String {
        match p {
            Participant::Human { peer } if *peer == self.coco.store.me => {
                if self.coco.store.my_name.is_empty() { "Ich".into() } else { self.coco.store.my_name.clone() }
            }
            Participant::Human { peer } => self.coco.store.contact(peer).map(|c| c.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| pi_net::identity::short(peer)),
            Participant::DevAgent { backend, .. } if backend == MAIN_BACKEND => "Haupt-Session".into(),
            Participant::DevAgent { backend, instance, .. } => self.net.inst(&(backend.clone(), instance.clone())).map(|i| i.info.name.clone()).unwrap_or_else(|| instance.clone()),
            Participant::Persona { persona, .. } => self.coco.personas.iter().find(|x| x.id == *persona).map(|x| x.name.clone()).unwrap_or_else(|| persona.to_string()),
        }
    }

    /// Eigene Agenten, die in Chats aufgenommen werden können (Haupt-Session, Instanzen, Personas).
    pub(crate) fn coco_my_agents(&self) -> Vec<Member> {
        let me = self.coco.store.me.clone();
        let mut v = vec![Member { who: Participant::DevAgent { owner: me.clone(), backend: MAIN_BACKEND.into(), instance: String::new() }, role: Role::Agent, name: "Haupt-Session".into() }];
        for b in &self.net.backends {
            for i in &b.insts {
                v.push(Member { who: Participant::DevAgent { owner: me.clone(), backend: b.cfg.id.clone(), instance: i.info.id.clone() }, role: Role::Agent, name: i.info.name.clone() });
            }
        }
        for p in &self.coco.personas {
            v.push(Member { who: Participant::Persona { owner: me.clone(), persona: p.id.clone() }, role: Role::Agent, name: p.name.clone() });
        }
        v
    }
}

#[allow(dead_code)]
fn _unused(_: HashSet<()>) {}
