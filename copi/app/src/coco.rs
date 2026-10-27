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
use pi_net::model::*;
use pi_net::persona::Persona;
use pi_net::remote::{RemoteBook, RemoteScope};
use pi_net::store::ChatStore;

use crate::app::App;

/// Pseudo-Backend für die Haupt-Session.
pub const MAIN_BACKEND: &str = "@main";

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
}

impl Coco {
    pub fn new(demo: bool) -> Self {
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
        }
    }
}

impl App {
    /// Einmal pro Frame aus `App::pump`: Transportereignisse, A2A-Züge, Spiegel, Abläufe.
    pub(crate) fn coco_pump(&mut self) {}

    /// Einzige Stelle, die cocopi-Zustand ändert.
    pub(crate) fn coco_apply(&mut self, act: CocoAct) {
        let _ = act;
    }

    /// Normale Eingabe abfangen, wenn der Hauptbereich einen Chat oder eine Fernsitzung
    /// zeigt. `true` = verbraucht (Eingabe wird geleert).
    pub(crate) fn coco_submit(&mut self, text: &str) -> bool {
        let _ = text;
        false
    }

    /// Anzeigename eines Teilnehmenden (Kontaktname, Persona, Instanzname, „Ich“).
    pub(crate) fn coco_name(&self, p: &Participant) -> String {
        match p {
            Participant::Human { peer } if *peer == self.coco.store.me => "Ich".into(),
            Participant::Human { peer } => pi_net::identity::short(peer),
            Participant::DevAgent { instance, .. } => instance.clone(),
            Participant::Persona { persona, .. } => persona.to_string(),
        }
    }
}
