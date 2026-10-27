//! Gemeinsames Datenmodell von cocopi: Teilnehmende (Menschen und Agenten), Rollen,
//! Räume (Seminar/Projekt), Chats (Gruppe/DM) und Nachrichten.
//!
//! GUI- und transportfrei; alle Typen sind serde-kompatibel und damit auch für einen
//! späteren Tauri-Adapter nutzbar. **Vertrag** zwischen den Teilen von cocopi
//! (Transport, Agent2Agent, Fernzugriff, Oberfläche) – nur additiv ändern.
use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
        pub struct $name(pub String);
        impl $name {
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
        }
        impl From<&str> for $name { fn from(s: &str) -> Self { Self(s.to_string()) } }
    };
}

id_type!(
    /// Geräteschlüssel eines Menschen: Ed25519-Public-Key, 64 Hex-Zeichen (iroh `EndpointId`).
    PeerId
);
id_type!(
    /// Chat-Kennung (zufällig, 32 Hex-Zeichen).
    ChatId
);
id_type!(
    /// Nachrichten-Kennung (zufällig, 32 Hex-Zeichen); global eindeutig, dient der Duplikaterkennung.
    MsgId
);
id_type!(
    /// Raum-Kennung (Seminar oder Projekt).
    SpaceId
);
id_type!(
    /// Kennung eines Nebenagenten (Persona), z. B. Dateiname ohne `.md`.
    PersonaId
);

/// Zufällige Kennung (128 Bit, Hex) aus dem Betriebssystem-Zufall.
pub fn random_id() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("Betriebssystem-Zufall nicht verfügbar");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Millisekunden seit Unix-Epoche.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Wer an einem Chat teilnimmt.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Participant {
    /// Mensch an einem Gerät (auch die lokale Person, siehe [`crate::store::ChatStore::me`]).
    Human { peer: PeerId },
    /// Laufende Entwicklungs-Session (pi-Instanz, lokal oder auf einem Server-Backend),
    /// gehört dem Gerät `owner`. Nur `owner` darf ihr Eingaben schicken.
    DevAgent { owner: PeerId, backend: String, instance: String },
    /// Nebenagent mit eigener Persönlichkeit, läuft auf dem Gerät `owner`.
    Persona { owner: PeerId, persona: PersonaId },
}

impl Participant {
    pub fn is_agent(&self) -> bool {
        !matches!(self, Participant::Human { .. })
    }
    pub fn is_human(&self) -> bool {
        matches!(self, Participant::Human { .. })
    }
    /// Gerät, das für diesen Teilnehmenden spricht (Mensch selbst oder Besitzer des Agenten).
    pub fn device(&self) -> &PeerId {
        match self {
            Participant::Human { peer } => peer,
            Participant::DevAgent { owner, .. } | Participant::Persona { owner, .. } => owner,
        }
    }
}

/// Rolle in einem Raum oder Chat.
///
/// - Seminar: genau eine Leitung (Ersteller, kryptografisch an dessen Geräteschlüssel
///   gebunden – **nicht** institutionell geprüft, siehe `docs/COCOPI.md`), Teilnehmende.
/// - Projekt: alle Menschen sind gleichberechtigte `Developer` (hierarchiefrei).
/// - Agenten haben immer `Agent` und handeln nur im Auftrag ihres Besitzers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Lead,
    Attendee,
    Developer,
    Agent,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Lead => "Seminarleitung",
            Role::Attendee => "Teilnehmend",
            Role::Developer => "Projektentwicklung",
            Role::Agent => "Agent",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceKind {
    Seminar,
    Project,
}

impl SpaceKind {
    pub fn label(self) -> &'static str {
        match self {
            SpaceKind::Seminar => "Seminar",
            SpaceKind::Project => "Projekt",
        }
    }
    /// Rolle für neu aufgenommene Menschen.
    pub fn default_human_role(self) -> Role {
        match self {
            SpaceKind::Seminar => Role::Attendee,
            SpaceKind::Project => Role::Developer,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub who: Participant,
    pub role: Role,
    /// Anzeigename (frei wählbar, **nicht** geprüft; maßgeblich ist der Schlüssel).
    pub name: String,
}

/// Seminar oder Projekt: Klammer um mehrere Chats mit gemeinsamer Mitgliederliste.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    pub kind: SpaceKind,
    /// Gerät, das den Raum angelegt hat (bei Seminaren = Leitung, Vertrauensanker).
    pub founder: PeerId,
    pub members: Vec<Member>,
    /// Tutorial-Schritt (nur Seminar; von der Leitung weitergeschaltet).
    pub step: usize,
    pub created: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatKind {
    /// Einzelchat zwischen genau zwei Teilnehmenden.
    Direct,
    Group,
}

/// Zusammensetzung eines Chats (für Filter und Kennzeichnung).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Composition {
    Humans,
    Agents,
    Mixed,
}

impl Composition {
    pub fn label(self) -> &'static str {
        match self {
            Composition::Humans => "Menschen",
            Composition::Agents => "Agenten",
            Composition::Mixed => "Gemischt",
        }
    }
}

/// Inhalt einer Nachricht. Alles ist **Daten**: nichts wird automatisch ausgeführt
/// oder an ein Modell geschickt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    Text { text: String },
    Code { lang: String, text: String },
    /// Prompt-Vorschlag; Übernahme in eine Eingabe nur nach ausdrücklicher Bestätigung.
    Prompt { text: String },
    /// Systemhinweis (Beitritt, Rollenwechsel, Fernzugriff …), lokal erzeugt.
    System { text: String },
}

impl Body {
    pub fn text(&self) -> &str {
        match self {
            Body::Text { text } | Body::Code { text, .. } | Body::Prompt { text } | Body::System { text } => text,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum Delivery {
    /// Nur lokal (z. B. Systemhinweis oder Chat ohne entfernte Mitglieder).
    Local,
    Pending,
    Sent,
    Received,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: MsgId,
    pub author: Participant,
    pub body: Body,
    pub ts: u64,
    pub delivery: Delivery,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    pub id: ChatId,
    pub title: String,
    pub kind: ChatKind,
    pub space: Option<SpaceId>,
    pub members: Vec<Member>,
    pub messages: Vec<Message>,
    pub archived: bool,
    pub muted: bool,
    pub pinned: bool,
    pub unread: usize,
    pub created: u64,
    /// Agent2Agent-Einstellungen, wenn mindestens zwei Agenten im Chat sind.
    #[serde(default)]
    pub a2a: Option<crate::a2a::A2aPolicy>,
}

impl Chat {
    pub fn composition(&self) -> Composition {
        let humans = self.members.iter().filter(|m| m.who.is_human()).count();
        let agents = self.members.len() - humans;
        // Die lokale Person zählt bei reinen Agentenchats nicht als „Mensch im Gespräch“,
        // wenn sie nur Moderatorin ist; maßgeblich ist, ob mehr als ein Mensch oder ein
        // Mensch *und* Agenten beteiligt sind.
        match (humans, agents) {
            (_, 0) => Composition::Humans,
            (0, _) => Composition::Agents,
            (1, a) if a >= 2 => Composition::Agents,
            _ => Composition::Mixed,
        }
    }
    pub fn member(&self, who: &Participant) -> Option<&Member> {
        self.members.iter().find(|m| &m.who == who)
    }
    pub fn role_of(&self, who: &Participant) -> Option<Role> {
        self.member(who).map(|m| m.role)
    }
    /// Entfernte Geräte, an die eine Nachricht dieses Chats zugestellt werden muss.
    pub fn remote_devices(&self, me: &PeerId) -> Vec<PeerId> {
        let mut v: Vec<PeerId> = self.members.iter().map(|m| m.who.device().clone()).filter(|d| d != me).collect();
        v.sort();
        v.dedup();
        v
    }
    pub fn last_ts(&self) -> u64 {
        self.messages.last().map(|m| m.ts).unwrap_or(self.created)
    }
}
