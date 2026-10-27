//! Lokale Chat- und Rollenverwaltung („ChatStore“) inklusive Sicherheitsregeln für
//! eingehende Umschläge. Reine Logik ohne I/O außer [`ChatStore::load`]/[`ChatStore::save`].
//!
//! Jede Änderung, die an andere Geräte gehen muss, liefert die Ziel-Geräte und die
//! Nutzlast zurück ([`Outbox`]); das Senden erledigt der Transport.
use serde::{Deserialize, Serialize};

use crate::model::*;
use crate::wire::Payload;

/// Höchstzahlen (Speicher- und Missbrauchsschutz).
pub const MAX_CHATS: usize = 200;
pub const MAX_MEMBERS: usize = 64;
pub const MAX_MESSAGES_PER_CHAT: usize = 2000;
pub const MAX_PENDING_INVITES: usize = 20;
pub const MAX_NAME: usize = 80;

/// Zu sendende Umschläge: (Zielgerät, Nutzlast).
pub type Outbox = Vec<(PeerId, Payload)>;

/// Eingeladen, aber noch nicht angenommen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingInvite {
    pub from: PeerId,
    pub chat: Chat,
    pub space: Option<Space>,
    pub note: String,
    pub received: u64,
}

/// Bekannter Kontakt (Mensch/Gerät). Nur bestätigte Kontakte dürfen zustellen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contact {
    pub peer: PeerId,
    pub name: String,
    /// Kontaktadresse (iroh `EndpointAddr` als JSON); enthält IP-Adressen.
    pub addr: String,
    /// Schlüssel persönlich/über vertrauenswürdigen Kanal geprüft (Sicherheitswörter).
    pub verified: bool,
    pub blocked: bool,
    pub added: u64,
}

/// Ergebnis von [`ChatStore::apply`] für die Oberfläche.
#[derive(Clone, Debug, PartialEq)]
pub enum Applied {
    /// Nichts sichtbar (z. B. Duplikat, Ack).
    Nothing,
    NewMessage { chat: ChatId, msg: MsgId },
    InviteReceived { chat: ChatId },
    MembersChanged { chat: ChatId },
    Left { chat: ChatId, who: PeerId },
    StepChanged { space: SpaceId, step: usize },
    Delivered { chat: ChatId, msg: MsgId },
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct ChatStore {
    /// Eigene Geräte-ID (aus der Identität).
    pub me: PeerId,
    /// Eigener Anzeigename.
    pub my_name: String,
    pub contacts: Vec<Contact>,
    pub spaces: Vec<Space>,
    pub chats: Vec<Chat>,
    pub invites: Vec<PendingInvite>,
}

impl ChatStore {
    pub fn new(me: PeerId, my_name: &str) -> Self {
        Self { me, my_name: my_name.to_string(), ..Default::default() }
    }
    pub fn me_participant(&self) -> Participant {
        Participant::Human { peer: self.me.clone() }
    }

    // ── Kontakte ──
    pub fn add_contact(&mut self, c: Contact) -> Result<(), String> {
        let _ = c;
        Err("noch nicht implementiert".into())
    }
    pub fn contact(&self, peer: &PeerId) -> Option<&Contact> {
        self.contacts.iter().find(|c| &c.peer == peer)
    }
    /// Darf dieses Gerät überhaupt zustellen? (bestätigt und nicht blockiert)
    pub fn accepts_from(&self, peer: &PeerId) -> bool {
        self.contact(peer).is_some_and(|c| !c.blocked)
    }

    // ── Räume ──
    /// Raum anlegen; Ersteller wird `Lead` (Seminar) bzw. `Developer` (Projekt).
    pub fn create_space(&mut self, name: &str, kind: SpaceKind) -> Result<SpaceId, String> {
        let _ = (name, kind);
        Err("noch nicht implementiert".into())
    }
    pub fn space(&self, id: &SpaceId) -> Option<&Space> {
        self.spaces.iter().find(|s| &s.id == id)
    }
    /// Rolle eines Menschen im Raum.
    pub fn space_role(&self, space: &SpaceId, peer: &PeerId) -> Option<Role> {
        let _ = (space, peer);
        None
    }
    /// Ist `peer` Leitung des Seminars `space`? (Vertrauensanker: `founder`)
    pub fn is_lead(&self, space: &SpaceId, peer: &PeerId) -> bool {
        self.space(space).is_some_and(|s| s.kind == SpaceKind::Seminar && &s.founder == peer)
    }
    /// Seminar: Tutorial-Schritt setzen (nur Leitung) → Outbox an alle Mitglieder.
    pub fn set_step(&mut self, space: &SpaceId, step: usize) -> Result<Outbox, String> {
        let _ = (space, step);
        Err("noch nicht implementiert".into())
    }

    // ── Chats ──
    /// Gruppenchat anlegen. `members` ohne mich; ich werde automatisch ergänzt.
    /// Rollen ergeben sich aus dem Raum bzw. `Developer`/`Agent`. Liefert Einladungen.
    pub fn create_group(&mut self, title: &str, space: Option<SpaceId>, members: Vec<Member>) -> Result<(ChatId, Outbox), String> {
        let _ = (title, space, members);
        Err("noch nicht implementiert".into())
    }
    /// Einzelchat mit genau einem Gegenüber (Mensch oder Agent). Bestehenden DM wiederverwenden.
    pub fn open_direct(&mut self, other: Member) -> Result<(ChatId, Outbox), String> {
        let _ = other;
        Err("noch nicht implementiert".into())
    }
    pub fn chat(&self, id: &ChatId) -> Option<&Chat> {
        self.chats.iter().find(|c| &c.id == id)
    }
    pub fn chat_mut(&mut self, id: &ChatId) -> Option<&mut Chat> {
        self.chats.iter_mut().find(|c| &c.id == id)
    }
    /// Nachricht als `author` posten (Mensch: ich; Agent: einer meiner Agenten).
    /// Liefert die Nachricht-ID und die Zustellungen an entfernte Mitglieder.
    pub fn post(&mut self, chat: &ChatId, author: Participant, body: Body) -> Result<(MsgId, Outbox), String> {
        let _ = (chat, author, body);
        Err("noch nicht implementiert".into())
    }
    /// Lokaler Systemhinweis (wird nicht verschickt).
    pub fn note(&mut self, chat: &ChatId, text: &str) {
        let _ = (chat, text);
    }
    /// Mitglied hinzufügen/entfernen/Rolle ändern (nur wenn ich verwalten darf).
    pub fn set_members(&mut self, chat: &ChatId, members: Vec<Member>) -> Result<Outbox, String> {
        let _ = (chat, members);
        Err("noch nicht implementiert".into())
    }
    /// Darf `peer` die Mitglieder dieses Chats verwalten?
    /// Gruppe im Seminar: nur Leitung. Projekt/ohne Raum: jedes menschliche Mitglied
    /// (hierarchiefrei), aber nur eigene Agenten hinzufügen/entfernen.
    pub fn can_manage(&self, chat: &ChatId, peer: &PeerId) -> bool {
        let _ = (chat, peer);
        false
    }
    pub fn leave(&mut self, chat: &ChatId) -> Outbox {
        let _ = chat;
        Vec::new()
    }
    pub fn set_archived(&mut self, chat: &ChatId, on: bool) {
        if let Some(c) = self.chat_mut(chat) {
            c.archived = on;
        }
    }
    pub fn set_muted(&mut self, chat: &ChatId, on: bool) {
        if let Some(c) = self.chat_mut(chat) {
            c.muted = on;
        }
    }
    pub fn set_pinned(&mut self, chat: &ChatId, on: bool) {
        if let Some(c) = self.chat_mut(chat) {
            c.pinned = on;
        }
    }
    pub fn mark_read(&mut self, chat: &ChatId) {
        if let Some(c) = self.chat_mut(chat) {
            c.unread = 0;
        }
    }
    /// Lokal löschen (nur auf diesem Gerät).
    pub fn delete_local(&mut self, chat: &ChatId) {
        self.chats.retain(|c| &c.id != chat);
    }
    /// Sortierte Liste für die Chatverwaltung (angeheftet zuerst, dann jüngste Aktivität).
    pub fn sorted(&self, filter: ChatFilter) -> Vec<&Chat> {
        let _ = filter;
        Vec::new()
    }

    // ── Einladungen ──
    pub fn accept_invite(&mut self, chat: &ChatId) -> Result<Outbox, String> {
        let _ = chat;
        Err("noch nicht implementiert".into())
    }
    pub fn decline_invite(&mut self, chat: &ChatId) -> Outbox {
        let _ = chat;
        Vec::new()
    }

    // ── Eingang ──
    /// Eingehenden Umschlag prüfen und anwenden. `from` = TLS-Identität der Gegenstelle.
    /// Fernzugriffs-Nutzlasten (`Remote*`) gehören zu [`crate::remote::RemoteBook`] und
    /// werden hier mit `Err` abgewiesen.
    pub fn apply(&mut self, from: &PeerId, p: Payload) -> Result<(Applied, Outbox), String> {
        let _ = (from, p);
        Err("noch nicht implementiert".into())
    }
    /// Zustellstatus setzen (vom Transport gemeldet).
    pub fn set_delivery(&mut self, chat: &ChatId, msg: &MsgId, d: Delivery) {
        let _ = (chat, msg, d);
    }

    // ── Persistenz ──
    /// Laden aus JSON-Datei; fehlt die Datei, `Ok(None)`.
    pub fn load(path: &std::path::Path) -> Result<Option<Self>, String> {
        let _ = path;
        Ok(None)
    }
    /// Atomar speichern (`.new` + rename), Modus 0600.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let _ = path;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ChatFilter {
    #[default]
    All,
    Direct,
    Humans,
    Agents,
    Mixed,
    Archived,
}
