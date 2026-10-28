//! Lokale Chat- und Rollenverwaltung mit Prüfung aller eingehenden Umschläge.
//! `from` muss aus der authentifizierten Transportverbindung stammen, nie aus JSON.
//! Outboxen werden vom Aufrufer versendet; sie sind kein dauerhaftes Offline-Postfach.
use serde::{Deserialize, Serialize};

use crate::model::*;
use crate::wire::Payload;

pub const MAX_CHATS: usize = 200;
pub const MAX_MEMBERS: usize = 64;
pub const MAX_MESSAGES_PER_CHAT: usize = 2000;
pub const MAX_PENDING_INVITES: usize = 20;
pub const MAX_NAME: usize = 80;

pub type Outbox = Vec<(PeerId, Payload)>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingInvite {
    pub from: PeerId,
    pub chat: Chat,
    pub space: Option<Space>,
    pub note: String,
    pub received: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contact {
    pub peer: PeerId,
    pub name: String,
    pub addr: String,
    pub verified: bool,
    pub blocked: bool,
    pub added: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Applied {
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
    pub me: PeerId,
    pub my_name: String,
    pub contacts: Vec<Contact>,
    pub spaces: Vec<Space>,
    pub chats: Vec<Chat>,
    pub invites: Vec<PendingInvite>,
}

fn clean_name(s: &str) -> String {
    crate::a2a::sanitize(s, MAX_NAME).trim().to_string()
}

fn human(peer: &PeerId) -> Participant {
    Participant::Human { peer: peer.clone() }
}

fn has_device(c: &Chat, peer: &PeerId) -> bool {
    c.members.iter().any(|m| m.who.device() == peer)
}

fn distribute(devices: Vec<PeerId>, p: Payload) -> Outbox {
    devices.into_iter().map(|d| (d, p.clone())).collect()
}

fn clean_body(body: Body) -> Result<Body, String> {
    if body.text().len() > crate::wire::MAX_TEXT {
        return Err("Nachricht zu groß (höchstens 32 KiB Text)".into());
    }
    let text = crate::a2a::sanitize(body.text(), crate::wire::MAX_TEXT);
    Ok(match body {
        Body::Text { .. } => Body::Text { text },
        Body::Prompt { .. } => Body::Prompt { text },
        Body::Code { lang, .. } => Body::Code {
            lang: clean_name(&lang),
            text,
        },
        Body::System { .. } => return Err("Systemhinweise dürfen nur lokal entstehen".into()),
    })
}

fn append(c: &mut Chat, msg: Message) {
    c.messages.push(msg);
    if c.messages.len() > MAX_MESSAGES_PER_CHAT {
        c.messages.drain(..c.messages.len() - MAX_MESSAGES_PER_CHAT);
    }
}

/// Prüft auch Rollen aus Netz-/Dateidaten, statt ihnen zu vertrauen.
fn validate_members(members: &[Member], space: Option<&Space>) -> Result<(), String> {
    if members.is_empty() || members.len() > MAX_MEMBERS {
        return Err(format!("Ein Chat braucht 1 bis {MAX_MEMBERS} Mitglieder"));
    }
    for (i, m) in members.iter().enumerate() {
        if members[..i].iter().any(|old| old.who == m.who) {
            return Err("Doppeltes Mitglied".into());
        }
        if m.name.len() > MAX_NAME || clean_name(&m.name) != m.name {
            return Err("Ungültiger Mitgliedsname".into());
        }
        let expected = if m.who.is_agent() {
            Role::Agent
        } else if space
            .is_some_and(|s| s.kind == SpaceKind::Seminar && &s.founder == m.who.device())
        {
            Role::Lead
        } else {
            space
                .map(|s| s.kind.default_human_role())
                .unwrap_or(Role::Developer)
        };
        if m.role != expected {
            return Err("Unzulässige Rolle: Leitung bleibt an den Seminargründer gebunden".into());
        }
    }
    if let Some(s) = space.filter(|s| s.kind == SpaceKind::Seminar) {
        if !members
            .iter()
            .any(|m| m.who == human(&s.founder) && m.role == Role::Lead)
        {
            return Err("Die Seminarleitung muss Mitglied bleiben".into());
        }
    }
    Ok(())
}

impl ChatStore {
    pub fn new(me: PeerId, my_name: &str) -> Self {
        Self {
            me,
            my_name: clean_name(my_name),
            ..Default::default()
        }
    }
    pub fn me_participant(&self) -> Participant {
        human(&self.me)
    }

    pub fn add_contact(&mut self, mut c: Contact) -> Result<(), String> {
        if c.peer == self.me {
            return Err("Der eigene Schlüssel ist kein Kontakt".into());
        }
        c.name = clean_name(&c.name);
        if let Some(old) = self.contacts.iter_mut().find(|old| old.peer == c.peer) {
            // Adresspflege darf weder Prüfung noch eine bewusste Sperre aufheben.
            old.name = c.name;
            old.addr = c.addr;
        } else {
            self.contacts.push(c);
        }
        Ok(())
    }
    pub fn contact(&self, peer: &PeerId) -> Option<&Contact> {
        self.contacts.iter().find(|c| &c.peer == peer)
    }
    /// Ein lokal hinzugefügter, nicht gesperrter Kontakt darf zustellen.
    /// `verified` ist zusätzlich für Seminare erforderlich.
    pub fn accepts_from(&self, peer: &PeerId) -> bool {
        self.contact(peer).is_some_and(|c| !c.blocked)
    }

    pub fn create_space(&mut self, name: &str, kind: SpaceKind) -> Result<SpaceId, String> {
        if self.spaces.len() >= MAX_CHATS {
            return Err("Zu viele Räume".into());
        }
        let name = clean_name(name);
        if name.is_empty() {
            return Err("Bitte einen Raumnamen angeben".into());
        }
        let id = SpaceId(random_id());
        self.spaces.push(Space {
            id: id.clone(),
            name,
            kind,
            founder: self.me.clone(),
            members: vec![Member {
                who: self.me_participant(),
                name: self.my_name.clone(),
                role: if kind == SpaceKind::Seminar {
                    Role::Lead
                } else {
                    Role::Developer
                },
            }],
            step: 0,
            created: now_ms(),
        });
        Ok(id)
    }
    pub fn space(&self, id: &SpaceId) -> Option<&Space> {
        self.spaces.iter().find(|s| &s.id == id)
    }
    pub fn space_role(&self, space: &SpaceId, peer: &PeerId) -> Option<Role> {
        self.space(space)?
            .members
            .iter()
            .find(|m| m.who == human(peer))
            .map(|m| m.role)
    }
    pub fn is_lead(&self, space: &SpaceId, peer: &PeerId) -> bool {
        self.space(space)
            .is_some_and(|s| s.kind == SpaceKind::Seminar && &s.founder == peer)
    }
    pub fn set_step(&mut self, space: &SpaceId, step: usize) -> Result<Outbox, String> {
        if !self.is_lead(space, &self.me) {
            return Err("Nur die Seminarleitung darf den Schritt setzen".into());
        }
        let s = self
            .spaces
            .iter_mut()
            .find(|s| &s.id == space)
            .ok_or("Raum unbekannt")?;
        s.step = step;
        let mut devices: Vec<_> = s
            .members
            .iter()
            .filter(|m| m.who.is_human())
            .map(|m| m.who.device().clone())
            .filter(|d| d != &self.me)
            .collect();
        devices.sort();
        devices.dedup();
        Ok(distribute(
            devices,
            Payload::Step {
                space: space.clone(),
                step,
            },
        ))
    }

    pub fn create_group(
        &mut self,
        title: &str,
        space: Option<SpaceId>,
        mut members: Vec<Member>,
    ) -> Result<(ChatId, Outbox), String> {
        if self.chats.len() >= MAX_CHATS {
            return Err("Zu viele Chats".into());
        }
        let room = match &space {
            Some(id) => Some(self.space(id).ok_or("Raum unbekannt")?),
            None => None,
        };
        if !members.iter().any(|m| m.who == self.me_participant()) {
            members.push(Member {
                who: self.me_participant(),
                name: self.my_name.clone(),
                role: Role::Developer,
            });
        }
        for m in &mut members {
            m.name = clean_name(&m.name);
            m.role = if m.who.is_agent() {
                Role::Agent
            } else {
                room.and_then(|s| s.members.iter().find(|r| r.who == m.who).map(|r| r.role))
                    .unwrap_or_else(|| {
                        room.map(|s| s.kind.default_human_role())
                            .unwrap_or(Role::Developer)
                    })
            };
        }
        validate_members(&members, room)?;
        let c = Chat {
            id: ChatId(random_id()),
            title: clean_name(title),
            kind: ChatKind::Group,
            space,
            members,
            messages: vec![],
            archived: false,
            muted: false,
            pinned: false,
            unread: 0,
            created: now_ms(),
            a2a: None,
        };
        self.check_invite_targets(&c, &c.remote_devices(&self.me))?;
        let out = self.invite_outbox(&c, c.remote_devices(&self.me));
        for (_, p) in &out {
            crate::wire::Envelope::new(p.clone()).encode()?;
        }
        self.remember_room_members(&c);
        let id = c.id.clone();
        self.chats.push(c);
        Ok((id, out))
    }
    pub fn open_direct(&mut self, mut other: Member) -> Result<(ChatId, Outbox), String> {
        if other.who == self.me_participant() {
            return Err("Ein Einzelchat braucht ein anderes Gegenüber".into());
        }
        let me = self.me_participant();
        if let Some(c) = self.chats.iter_mut().find(|c| {
            c.kind == ChatKind::Direct
                && c.members.len() == 2
                && c.member(&me).is_some()
                && c.member(&other.who).is_some()
        }) {
            c.archived = false;
            return Ok((c.id.clone(), vec![]));
        }
        other.name = clean_name(&other.name);
        let title = other.name.clone();
        let (id, out) = self.create_group(&title, None, vec![other])?;
        if let Some(c) = self.chat_mut(&id) {
            c.kind = ChatKind::Direct;
        }
        // Erst nach Festlegen der Chatart einladen.
        let out = out
            .into_iter()
            .map(|(d, p)| {
                (
                    d,
                    match p {
                        Payload::Invite {
                            mut chat,
                            space,
                            note,
                        } => {
                            chat.kind = ChatKind::Direct;
                            Payload::Invite { chat, space, note }
                        }
                        p => p,
                    },
                )
            })
            .collect();
        Ok((id, out))
    }
    pub fn chat(&self, id: &ChatId) -> Option<&Chat> {
        self.chats.iter().find(|c| &c.id == id)
    }
    pub fn chat_mut(&mut self, id: &ChatId) -> Option<&mut Chat> {
        self.chats.iter_mut().find(|c| &c.id == id)
    }

    pub fn post(
        &mut self,
        chat: &ChatId,
        author: Participant,
        body: Body,
    ) -> Result<(MsgId, Outbox), String> {
        let c = self.chat(chat).ok_or("Chat unbekannt")?;
        if c.archived || author.device() != &self.me || c.member(&author).is_none() {
            return Err("Nur eigene Mitglieder dürfen in einem aktiven Chat schreiben".into());
        }
        let body = clean_body(body)?;
        let devices = c.remote_devices(&self.me);
        let msg = Message {
            id: MsgId(random_id()),
            author,
            body,
            ts: now_ms(),
            delivery: if devices.is_empty() {
                Delivery::Local
            } else {
                Delivery::Pending
            },
        };
        let p = Payload::Post {
            chat: chat.clone(),
            msg: msg.clone(),
        };
        crate::wire::Envelope::new(p.clone()).encode()?;
        let id = msg.id.clone();
        append(self.chat_mut(chat).ok_or("Chat unbekannt")?, msg);
        Ok((id, distribute(devices, p)))
    }
    pub fn note(&mut self, chat: &ChatId, text: &str) {
        let author = self.me_participant();
        if let Some(c) = self.chat_mut(chat) {
            append(
                c,
                Message {
                    id: MsgId(random_id()),
                    author,
                    body: Body::System {
                        text: crate::a2a::sanitize(text, crate::wire::MAX_TEXT),
                    },
                    ts: now_ms(),
                    delivery: Delivery::Local,
                },
            );
        }
    }
    pub fn set_members(
        &mut self,
        chat: &ChatId,
        mut members: Vec<Member>,
    ) -> Result<Outbox, String> {
        for m in &mut members {
            m.name = clean_name(&m.name);
        }
        self.check_member_change(chat, &self.me, &members)?;
        let old = self.chat(chat).ok_or("Chat unbekannt")?;
        let old_devices = old.remote_devices(&self.me);
        let mut next = old.clone();
        next.members = members.clone();
        let new_devices = next.remote_devices(&self.me);
        let added = new_devices
            .iter()
            .filter(|d| !old_devices.contains(d))
            .cloned()
            .collect::<Vec<_>>();
        self.check_invite_targets(&next, &added)?;
        let mut all = old_devices;
        all.extend(new_devices);
        all.sort();
        all.dedup();
        // Neue Geräte brauchen zuerst die bestätigbare Einladung, nicht einen Verlauf.
        let mut out = self.invite_outbox(&next, added);
        out.extend(distribute(
            all,
            Payload::Members {
                chat: chat.clone(),
                members,
            },
        ));
        for (_, p) in &out {
            crate::wire::Envelope::new(p.clone()).encode()?;
        }
        self.chat_mut(chat).ok_or("Chat unbekannt")?.members = next.members.clone();
        self.remember_room_members(&next);
        if next.member(&self.me_participant()).is_none() {
            self.set_archived(chat, true);
            self.note(chat, "Du bist nicht mehr Mitglied dieses Chats.");
        }
        Ok(out)
    }
    pub fn can_manage(&self, chat: &ChatId, peer: &PeerId) -> bool {
        let Some(c) = self.chat(chat) else {
            return false;
        };
        if c.kind != ChatKind::Group || c.archived || c.member(&human(peer)).is_none() {
            return false;
        }
        match c.space.as_ref().and_then(|id| self.space(id)) {
            Some(s) if s.kind == SpaceKind::Seminar => &s.founder == peer,
            _ => true,
        }
    }
    fn check_member_change(
        &self,
        chat: &ChatId,
        requester: &PeerId,
        members: &[Member],
    ) -> Result<(), String> {
        if !self.can_manage(chat, requester) {
            return Err(
                "Mitgliederänderung nicht erlaubt (Seminar: nur Leitung; Einzelchat: nie)".into(),
            );
        }
        let c = self.chat(chat).ok_or("Chat unbekannt")?;
        let room = c.space.as_ref().and_then(|id| self.space(id));
        validate_members(members, room)?;
        if !room.is_some_and(|s| s.kind == SpaceKind::Seminar) {
            for m in &c.members {
                if m.who.is_agent()
                    && m.who.device() != requester
                    && !members.iter().any(|n| n.who == m.who)
                {
                    return Err("Fremde Agenten dürfen nicht entfernt werden".into());
                }
            }
            for m in members {
                if m.who.is_agent() && m.who.device() != requester && c.member(&m.who).is_none() {
                    return Err("Nur eigene Agenten dürfen hinzugefügt werden".into());
                }
            }
        }
        Ok(())
    }
    pub fn leave(&mut self, chat: &ChatId) -> Outbox {
        let Some(c) = self.chat(chat) else {
            return vec![];
        };
        if c.archived || !has_device(c, &self.me) {
            return vec![];
        }
        let out = distribute(
            c.remote_devices(&self.me),
            Payload::Leave { chat: chat.clone() },
        );
        let me = self.me.clone();
        if let Some(c) = self.chat_mut(chat) {
            c.members.retain(|m| m.who.device() != &me);
            c.archived = true;
        }
        self.note(chat, "Du hast den Chat verlassen.");
        out
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
    pub fn delete_local(&mut self, chat: &ChatId) {
        self.chats.retain(|c| &c.id != chat);
    }
    pub fn sorted(&self, filter: ChatFilter) -> Vec<&Chat> {
        let mut chats: Vec<_> = self
            .chats
            .iter()
            .filter(|c| {
                if c.archived != (filter == ChatFilter::Archived) {
                    return false;
                }
                match filter {
                    ChatFilter::Direct => c.kind == ChatKind::Direct,
                    ChatFilter::Humans => c.composition() == Composition::Humans,
                    ChatFilter::Agents => c.composition() == Composition::Agents,
                    ChatFilter::Mixed => c.composition() == Composition::Mixed,
                    _ => true,
                }
            })
            .collect();
        chats.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.last_ts().cmp(&a.last_ts()))
                .then_with(|| a.id.cmp(&b.id))
        });
        chats
    }

    fn check_invite_targets(&self, chat: &Chat, devices: &[PeerId]) -> Result<(), String> {
        if chat
            .space
            .as_ref()
            .is_some_and(|id| self.space(id).is_some_and(|s| s.kind == SpaceKind::Seminar))
        {
            if devices
                .iter()
                .any(|d| !self.contact(d).is_some_and(|c| c.verified && !c.blocked))
            {
                return Err("Seminar-Einladungen sind nur an geprüfte, nicht gesperrte Kontakte erlaubt; bitte Sicherheitswörter vergleichen".into());
            }
        }
        Ok(())
    }
    fn invite_outbox(&self, chat: &Chat, devices: Vec<PeerId>) -> Outbox {
        let mut chat = chat.clone();
        chat.messages.clear();
        chat.archived = false;
        chat.muted = false;
        chat.pinned = false;
        chat.unread = 0;
        // Zustimmung zu lokaler Automatisierung darf niemals mitverteilt werden.
        chat.a2a = None;
        let space = chat.space.as_ref().and_then(|id| self.space(id)).cloned();
        distribute(
            devices,
            Payload::Invite {
                chat,
                space,
                note: String::new(),
            },
        )
    }
    fn remember_room_members(&mut self, chat: &Chat) {
        if let Some(s) = chat
            .space
            .as_ref()
            .and_then(|id| self.spaces.iter_mut().find(|s| &s.id == id))
        {
            for m in &chat.members {
                if !s.members.iter().any(|r| r.who == m.who) && s.members.len() < MAX_MEMBERS {
                    s.members.push(m.clone());
                }
            }
        }
    }
    pub fn accept_invite(&mut self, chat: &ChatId) -> Result<Outbox, String> {
        let invite = self
            .invites
            .iter()
            .find(|i| &i.chat.id == chat)
            .cloned()
            .ok_or("Einladung unbekannt")?;
        if self.chats.len() >= MAX_CHATS {
            return Err("Zu viele Chats".into());
        }
        self.check_invite(&invite.from, &invite.chat, invite.space.as_ref())?;
        if self.chat(chat).is_some() {
            return Err("Chat bereits vorhanden".into());
        }
        if let Some(s) = &invite.space {
            if self.space(&s.id).is_none() {
                if self.spaces.len() >= MAX_CHATS {
                    return Err("Zu viele Räume".into());
                }
                self.spaces.push(s.clone());
            }
        }
        self.remember_room_members(&invite.chat);
        self.chats.push(invite.chat);
        self.invites.retain(|i| &i.chat.id != chat);
        Ok(vec![(
            invite.from,
            Payload::InviteAnswer {
                chat: chat.clone(),
                accept: true,
            },
        )])
    }
    pub fn decline_invite(&mut self, chat: &ChatId) -> Outbox {
        let from = self
            .invites
            .iter()
            .find(|i| &i.chat.id == chat)
            .map(|i| i.from.clone());
        self.invites.retain(|i| &i.chat.id != chat);
        from.map(|from| {
            vec![(
                from,
                Payload::InviteAnswer {
                    chat: chat.clone(),
                    accept: false,
                },
            )]
        })
        .unwrap_or_default()
    }
    fn check_invite(
        &self,
        from: &PeerId,
        chat: &Chat,
        space: Option<&Space>,
    ) -> Result<(), String> {
        if !self.accepts_from(from) {
            return Err("Absender ist kein freigegebener Kontakt".into());
        }
        if !has_device(chat, from) || !has_device(chat, &self.me) {
            return Err("Einladender und Eingeladener müssen Mitglieder sein".into());
        }
        if chat.title.len() > MAX_NAME
            || clean_name(&chat.title) != chat.title
            || !chat.messages.is_empty()
        {
            return Err(
                "Einladung muss einen gültigen Titel und darf keinen Verlauf enthalten".into(),
            );
        }
        if chat.kind == ChatKind::Direct && chat.members.len() != 2 {
            return Err("Einzelchat braucht genau zwei Mitglieder".into());
        }
        if chat.space.as_ref() != space.map(|s| &s.id) {
            return Err("Raum passt nicht zur Einladung".into());
        }
        if let Some(s) = space {
            if s.name.len() > MAX_NAME || clean_name(&s.name) != s.name {
                return Err("Ungültiger Raumname".into());
            }
            validate_members(&s.members, Some(s))?;
            if let Some(known) = self.space(&s.id) {
                if known.founder != s.founder || known.kind != s.kind {
                    return Err("Raumgründer darf nicht ersetzt werden".into());
                }
            } else if s.kind == SpaceKind::Seminar && &s.founder != from {
                return Err("Unbekanntes Seminar muss von seiner Leitung eingeladen werden".into());
            }
            if s.kind == SpaceKind::Seminar && !self.contact(from).is_some_and(|c| c.verified) {
                return Err("Seminar-Einladung erfordert einen geprüften Kontakt".into());
            }
        }
        validate_members(&chat.members, space)
    }

    /// Prüft vor jeder Mutation. Fernzugriff gehört in `RemoteBook`.
    pub fn apply(&mut self, from: &PeerId, p: Payload) -> Result<(Applied, Outbox), String> {
        if !self.accepts_from(from) {
            return Err("Absender ist kein freigegebener Kontakt oder ist gesperrt".into());
        }
        // Auch direkte Aufrufer (ohne Decoder) dürfen die Umschlaggrenze nicht umgehen.
        crate::wire::Envelope::new(p.clone()).encode()?;
        let applied = match p {
            Payload::Invite {
                mut chat,
                space,
                note,
            } => {
                self.check_invite(from, &chat, space.as_ref())?;
                if note.len() > crate::wire::MAX_TEXT {
                    return Err("Einladungstext zu groß".into());
                }
                if self.chat(&chat.id).is_some() {
                    return Ok((Applied::Nothing, vec![]));
                }
                if !self.invites.iter().any(|i| i.chat.id == chat.id)
                    && self.invites.len() >= MAX_PENDING_INVITES
                {
                    return Err("Zu viele ausstehende Einladungen".into());
                }
                chat.archived = false;
                chat.muted = false;
                chat.pinned = false;
                chat.unread = 0;
                chat.a2a = None;
                let id = chat.id.clone();
                self.invites.retain(|i| i.chat.id != id);
                self.invites.push(PendingInvite {
                    from: from.clone(),
                    chat,
                    space,
                    note: crate::a2a::sanitize(&note, crate::wire::MAX_TEXT),
                    received: now_ms(),
                });
                Applied::InviteReceived { chat: id }
            }
            Payload::Post { chat, mut msg } => {
                let c = self.chat(&chat).ok_or("Chat unbekannt")?;
                if c.archived
                    || !has_device(c, &self.me)
                    || !has_device(c, from)
                    || msg.author.device() != from
                    || c.member(&msg.author).is_none()
                {
                    return Err("Absender oder Autor ist kein berechtigtes Chatmitglied".into());
                }
                msg.body = clean_body(msg.body)?;
                if c.messages.iter().any(|m| m.id == msg.id) {
                    return Ok((Applied::Nothing, vec![]));
                }
                let now = now_ms();
                if msg.ts > now.saturating_add(60_000) {
                    msg.ts = now;
                }
                msg.delivery = Delivery::Received;
                let id = msg.id.clone();
                let c = self.chat_mut(&chat).ok_or("Chat unbekannt")?;
                if !c.muted {
                    c.unread = c.unread.saturating_add(1);
                }
                append(c, msg);
                return Ok((
                    Applied::NewMessage {
                        chat: chat.clone(),
                        msg: id.clone(),
                    },
                    vec![(from.clone(), Payload::Ack { chat, msg: id })],
                ));
            }
            Payload::Ack { chat, msg } => {
                let c = self.chat(&chat).ok_or("Chat unbekannt")?;
                if !has_device(c, from) {
                    return Err("Bestätigung von Nichtmitglied".into());
                }
                let own = c.messages.iter().any(|m| {
                    m.id == msg && m.author.device() == &self.me && m.delivery == Delivery::Pending
                });
                if !own {
                    return Ok((Applied::Nothing, vec![]));
                }
                // Sent = mindestens ein Gerät hat bestätigt, NICHT gelesen/alle erreicht.
                // Received bezeichnet ausschließlich eine eingegangene fremde Nachricht.
                self.set_delivery(&chat, &msg, Delivery::Sent);
                Applied::Delivered { chat, msg }
            }
            Payload::Members { chat, members } => {
                // Mitgliederlisten nach einer Einladung können vor deren Annahme eintreffen.
                // Sie ändern weder die ausstehende Einladung noch deren Zustimmung.
                self.check_member_change(&chat, from, &members)?;
                self.chat_mut(&chat).ok_or("Chat unbekannt")?.members = members;
                let c = self.chat(&chat).ok_or("Chat unbekannt")?.clone();
                self.remember_room_members(&c);
                if c.member(&self.me_participant()).is_none() {
                    self.set_archived(&chat, true);
                    self.note(&chat, "Du wurdest aus dem Chat entfernt.");
                }
                Applied::MembersChanged { chat }
            }
            Payload::Leave { chat }
            | Payload::InviteAnswer {
                chat,
                accept: false,
            } => {
                let c = self.chat(&chat).ok_or("Chat unbekannt")?;
                if !has_device(c, from) {
                    return Err("Austritt von Nichtmitglied".into());
                }
                self.chat_mut(&chat)
                    .ok_or("Chat unbekannt")?
                    .members
                    .retain(|m| m.who.device() != from);
                self.note(
                    &chat,
                    "Ein Mitglied hat den Chat verlassen oder die Einladung abgelehnt.",
                );
                Applied::Left {
                    chat,
                    who: from.clone(),
                }
            }
            Payload::InviteAnswer { chat, accept: true } => {
                if !self.chat(&chat).is_some_and(|c| has_device(c, from)) {
                    return Err("Einladungsantwort von Nichtmitglied".into());
                }
                Applied::Nothing
            }
            Payload::Step { space, step } => {
                if !self.is_lead(&space, from) {
                    return Err("Nur die Seminarleitung darf den Schritt setzen".into());
                }
                self.spaces
                    .iter_mut()
                    .find(|s| s.id == space)
                    .ok_or("Raum unbekannt")?
                    .step = step;
                Applied::StepChanged { space, step }
            }
            Payload::Steuern { .. } | Payload::Geraeteausweis { .. } | Payload::SeminarPacket { .. } | Payload::SeminarPresence => return Err("gehört zur Ausweis-/Freigabeprüfung".into()),
            Payload::RemoteRequest { .. }
            | Payload::RemoteGrant { .. }
            | Payload::RemoteEnd { .. }
            | Payload::RemoteFrame { .. }
            | Payload::RemoteInput { .. } => return Err("gehört zum Fernzugriff".into()),
        };
        Ok((applied, vec![]))
    }
    pub fn set_delivery(&mut self, chat: &ChatId, msg: &MsgId, d: Delivery) {
        let me = self.me.clone();
        if let Some(m) = self.chat_mut(chat).and_then(|c| {
            c.messages
                .iter_mut()
                .find(|m| &m.id == msg && m.author.device() == &me)
        }) {
            // Verspätete Transportmeldungen dürfen eine Bestätigung nicht zurücksetzen.
            if m.delivery != Delivery::Local
                && m.delivery != Delivery::Received
                && !(m.delivery == Delivery::Sent
                    && matches!(d, Delivery::Pending | Delivery::Failed(_)))
            {
                m.delivery = d;
            }
        }
    }

    pub fn load(path: &std::path::Path) -> Result<Option<Self>, String> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("Chats konnten nicht gelesen werden: {e}")),
        };
        let store: Self =
            serde_json::from_slice(&bytes).map_err(|e| format!("Ungültige Chatdatei: {e}"))?;
        if store.chats.len() > MAX_CHATS
            || store.spaces.len() > MAX_CHATS
            || store.invites.len() > MAX_PENDING_INVITES
        {
            return Err("Chatdatei überschreitet die Speichergrenzen".into());
        }
        if store.spaces.iter().any(|s| s.members.len() > MAX_MEMBERS)
            || store.invites.iter().any(|i| {
                i.chat.members.len() > MAX_MEMBERS
                    || !i.chat.messages.is_empty()
                    || i.space
                        .as_ref()
                        .is_some_and(|s| s.members.len() > MAX_MEMBERS)
                    || i.note.len() > crate::wire::MAX_TEXT
            })
        {
            return Err("Chatdatei überschreitet die Speichergrenzen".into());
        }
        for c in &store.chats {
            if c.members.len() > MAX_MEMBERS
                || c.messages.len() > MAX_MESSAGES_PER_CHAT
                || c.messages
                    .iter()
                    .any(|m| m.body.text().len() > crate::wire::MAX_TEXT)
            {
                return Err("Chatdatei überschreitet die Speichergrenzen".into());
            }
        }
        Ok(Some(store))
    }
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| format!("Chats konnten nicht gespeichert werden: {e}"))?;
        atomic_write(path, &bytes)
    }
}

/// Keine bestehenden `.new`-Dateien/Symlinks öffnen. Veraltete Reste müssen bewusst
/// entfernt werden. Ein fehlgeschlagener Austausch lässt die alte Datei unberührt.
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Ordner konnte nicht angelegt werden: {e}"))?;
    let mut name = path.as_os_str().to_os_string();
    name.push(".new");
    let tmp = std::path::PathBuf::from(name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|e| format!("Temporäre Chatdatei konnte nicht angelegt werden: {e}"))?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)?;
        std::fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("Chats konnten nicht atomar gespeichert werden: {e}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn member(peer: &str) -> Member {
        Member {
            who: human(&PeerId::from(peer)),
            role: Role::Developer,
            name: peer.into(),
        }
    }
    fn agent(peer: &str) -> Member {
        Member {
            who: Participant::Persona {
                owner: PeerId::from(peer),
                persona: PersonaId::from("helfer"),
            },
            role: Role::Agent,
            name: format!("Agent {peer}"),
        }
    }
    fn contact(peer: &str, verified: bool) -> Contact {
        Contact {
            peer: PeerId::from(peer),
            name: peer.into(),
            addr: "{}".into(),
            verified,
            blocked: false,
            added: now_ms(),
        }
    }
    fn pair() -> (ChatStore, ChatStore) {
        let mut a = ChatStore::new(PeerId::from("a"), "Anna");
        let mut b = ChatStore::new(PeerId::from("b"), "Berta");
        a.add_contact(contact("b", true)).unwrap();
        b.add_contact(contact("a", true)).unwrap();
        (a, b)
    }
    fn deliver(from: &ChatStore, to: &mut ChatStore, out: Outbox) -> Outbox {
        let mut replies = vec![];
        for (target, payload) in out {
            if target == to.me {
                replies.extend(to.apply(&from.me, payload).unwrap().1);
            }
        }
        replies
    }
    fn joined(kind: Option<SpaceKind>) -> (ChatStore, ChatStore, ChatId) {
        let (mut a, mut b) = pair();
        let space = kind.map(|k| a.create_space("Raum", k).unwrap());
        let (id, out) = a
            .create_group("Gruppe", space, vec![member("b"), agent("a"), agent("b")])
            .unwrap();
        deliver(&a, &mut b, out);
        assert!(b.chat(&id).is_none());
        let out = b.accept_invite(&id).unwrap();
        deliver(&b, &mut a, out);
        (a, b, id)
    }
    fn text(s: &str) -> Body {
        Body::Text { text: s.into() }
    }
    fn post(id: &ChatId, author: Participant) -> Payload {
        Payload::Post {
            chat: id.clone(),
            msg: Message {
                id: MsgId(random_id()),
                author,
                body: text("Hallo"),
                ts: now_ms(),
                delivery: Delivery::Local,
            },
        }
    }
    fn rejected_without_change(store: &mut ChatStore, from: &str, p: Payload) {
        let before = serde_json::to_vec(store).unwrap();
        assert!(store.apply(&PeerId::from(from), p).is_err());
        assert_eq!(before, serde_json::to_vec(store).unwrap());
    }

    #[test]
    fn contacts_update_without_resetting_trust_or_block() {
        let (mut a, _) = pair();
        a.contacts[0].blocked = true;
        let mut c = contact("b", false);
        c.name = format!("\u{202e}{}", "ü".repeat(100));
        c.addr = "neu".into();
        a.add_contact(c).unwrap();
        assert_eq!(a.contacts.len(), 1);
        assert!(a.contacts[0].verified);
        assert!(a.contacts[0].blocked);
        assert!(a.contacts[0].name.len() <= MAX_NAME);
        assert!(!a.contacts[0].name.contains('\u{202e}'));
        assert_eq!(a.contacts[0].addr, "neu");
        assert!(a.add_contact(contact("a", true)).is_err());
        assert!(!a.accepts_from(&PeerId::from("b")));
    }
    #[test]
    fn groups_project_and_seminar_exchange_over_two_stores() {
        for kind in [None, Some(SpaceKind::Project), Some(SpaceKind::Seminar)] {
            let (mut a, mut b, id) = joined(kind);
            let c = a.chat(&id).unwrap();
            assert_eq!(c.members.len(), 4);
            assert_eq!(c.role_of(&agent("b").who), Some(Role::Agent));
            let a_role = if kind == Some(SpaceKind::Seminar) {
                Role::Lead
            } else {
                Role::Developer
            };
            let b_role = if kind == Some(SpaceKind::Seminar) {
                Role::Attendee
            } else {
                Role::Developer
            };
            assert_eq!(c.role_of(&a.me_participant()), Some(a_role));
            assert_eq!(c.role_of(&b.me_participant()), Some(b_role));
            if let Some(space) = c.space.clone() {
                assert_eq!(a.space_role(&space, &a.me), Some(a_role));
                assert_eq!(b.space_role(&space, &b.me), Some(b_role));
                if kind == Some(SpaceKind::Seminar) {
                    let out = a.set_step(&space, 7).unwrap();
                    assert_eq!(out.len(), 1);
                    deliver(&a, &mut b, out);
                    assert_eq!(b.space(&space).unwrap().step, 7);
                    assert!(b.set_step(&space, 8).is_err());
                } else {
                    assert!(a.set_step(&space, 1).is_err());
                }
            }
            for (author, body) in [
                (a.me_participant(), text("hallo\u{202e}\u{0001}")),
                (
                    agent("a").who,
                    Body::Code {
                        lang: "rust".into(),
                        text: "42".into(),
                    },
                ),
            ] {
                let (msg, out) = a.post(&id, author, body).unwrap();
                assert_eq!(out.len(), 1);
                assert_eq!(
                    a.chat(&id).unwrap().messages.last().unwrap().delivery,
                    Delivery::Pending
                );
                let out = deliver(&a, &mut b, out);
                assert_eq!(out.len(), 1);
                deliver(&b, &mut a, out);
                assert_eq!(
                    a.chat(&id).unwrap().messages.last().unwrap().delivery,
                    Delivery::Sent
                );
                assert_eq!(
                    b.chat(&id).unwrap().messages.last().unwrap().delivery,
                    Delivery::Received
                );
                a.set_delivery(&id, &msg, Delivery::Pending);
                a.set_delivery(&id, &msg, Delivery::Failed("spät".into()));
                assert_eq!(
                    a.chat(&id).unwrap().messages.last().unwrap().delivery,
                    Delivery::Sent
                );
            }
            let (_, out) = b
                .post(
                    &id,
                    b.me_participant(),
                    Body::Prompt {
                        text: "Vorschlag".into(),
                    },
                )
                .unwrap();
            let out = deliver(&b, &mut a, out);
            deliver(&a, &mut b, out);
            assert_eq!(b.chat(&id).unwrap().unread, 2);
            assert_eq!(a.chat(&id).unwrap().unread, 1);
        }
    }
    #[test]
    fn direct_remote_reuse_and_own_agent_local() {
        let (mut a, mut b) = pair();
        let (id, out) = a.open_direct(member("b")).unwrap();
        deliver(&a, &mut b, out);
        assert_eq!(b.invites[0].chat.kind, ChatKind::Direct);
        let out = b.accept_invite(&id).unwrap();
        deliver(&b, &mut a, out);
        assert_eq!(a.chat(&id).unwrap().members.len(), 2);
        a.set_archived(&id, true);
        let (same, out) = a.open_direct(member("b")).unwrap();
        assert_eq!(id, same);
        assert!(out.is_empty());
        let (_, out) = a.post(&id, a.me_participant(), text("DM")).unwrap();
        let reply = deliver(&a, &mut b, out);
        deliver(&b, &mut a, reply);
        assert!(!a.can_manage(&id, &a.me));
        assert!(a.set_members(&id, vec![member("a")]).is_err());
        let (local, out) = a.open_direct(agent("a")).unwrap();
        assert!(out.is_empty());
        assert_eq!(a.chat(&local).unwrap().members.len(), 2);
        let (_, out) = a.post(&local, agent("a").who, text("lokal")).unwrap();
        assert!(out.is_empty());
        assert_eq!(
            a.chat(&local).unwrap().messages[0].delivery,
            Delivery::Local
        );
        assert!(a.open_direct(member("a")).is_err());
    }
    #[test]
    fn unknown_blocked_sender_nonmember_and_unknown_chat_rejected() {
        let (_, mut b, id) = joined(None);
        rejected_without_change(&mut b, "x", post(&id, member("x").who));
        b.add_contact(contact("x", true)).unwrap();
        rejected_without_change(&mut b, "x", post(&id, member("x").who));
        rejected_without_change(
            &mut b,
            "a",
            post(&ChatId::from("unbekannt"), member("a").who),
        );
        b.contacts
            .iter_mut()
            .find(|c| c.peer == PeerId::from("a"))
            .unwrap()
            .blocked = true;
        rejected_without_change(&mut b, "a", post(&id, member("a").who));
    }
    #[test]
    fn forged_human_agent_and_nonmember_author_rejected() {
        let (mut a, mut b, id) = joined(None);
        rejected_without_change(&mut b, "a", post(&id, member("b").who));
        rejected_without_change(&mut b, "a", post(&id, agent("b").who));
        let missing = Participant::Persona {
            owner: PeerId::from("a"),
            persona: PersonaId::from("nicht-im-chat"),
        };
        rejected_without_change(&mut b, "a", post(&id, missing.clone()));
        assert!(a.post(&id, member("b").who, text("falsch")).is_err());
        assert!(a.post(&id, agent("b").who, text("falsch")).is_err());
        assert!(a.post(&id, missing, text("falsch")).is_err());
    }
    #[test]
    fn duplicate_future_oversize_and_system_spoofing() {
        let (mut a, mut b, id) = joined(None);
        let (_, out) = a.post(&id, a.me_participant(), text("einmal")).unwrap();
        let p = out[0].1.clone();
        deliver(&a, &mut b, out);
        let (applied, out) = b.apply(&a.me, p).unwrap();
        assert_eq!(applied, Applied::Nothing);
        assert!(out.is_empty());
        assert_eq!(b.chat(&id).unwrap().messages.len(), 1);
        assert_eq!(b.chat(&id).unwrap().unread, 1);
        let mut p = post(&id, a.me_participant());
        if let Payload::Post { msg, .. } = &mut p {
            msg.ts = u64::MAX;
        }
        let before = now_ms();
        b.apply(&a.me, p).unwrap();
        let ts = b.chat(&id).unwrap().messages.last().unwrap().ts;
        assert!(ts >= before && ts <= now_ms());
        let too_big = "a".repeat(crate::wire::MAX_TEXT + 1);
        assert!(a.post(&id, a.me_participant(), text(&too_big)).is_err());
        let mut p = post(&id, a.me_participant());
        if let Payload::Post { msg, .. } = &mut p {
            msg.body = text(&too_big);
        }
        rejected_without_change(&mut b, "a", p);
        let mut p = post(&id, a.me_participant());
        if let Payload::Post { msg, .. } = &mut p {
            msg.body = Body::System {
                text: "Leitung ernannt".into(),
            };
        }
        rejected_without_change(&mut b, "a", p);
        assert!(
            a.post(
                &id,
                a.me_participant(),
                Body::System {
                    text: "fake".into()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn seminar_attendee_cannot_manage_or_set_step_and_lead_is_immutable() {
        let (mut a, mut b, id) = joined(Some(SpaceKind::Seminar));
        let members = a.chat(&id).unwrap().members.clone();
        let space = a.chat(&id).unwrap().space.clone().unwrap();
        assert!(!b.can_manage(&id, &b.me));
        rejected_without_change(
            &mut a,
            "b",
            Payload::Members {
                chat: id.clone(),
                members: members.clone(),
            },
        );
        rejected_without_change(&mut a, "b", Payload::Step { space, step: 4 });
        let mut forged = members.clone();
        forged
            .iter_mut()
            .find(|m| m.who == member("b").who)
            .unwrap()
            .role = Role::Lead;
        rejected_without_change(
            &mut b,
            "a",
            Payload::Members {
                chat: id.clone(),
                members: forged,
            },
        );
        let no_founder = members
            .into_iter()
            .filter(|m| m.who != member("a").who)
            .collect();
        rejected_without_change(
            &mut b,
            "a",
            Payload::Members {
                chat: id,
                members: no_founder,
            },
        );
    }
    #[test]
    fn project_agent_ownership_and_no_lead_claims() {
        let (mut a, mut b, id) = joined(Some(SpaceKind::Project));
        let members = a.chat(&id).unwrap().members.clone();
        let no_foreign = members
            .iter()
            .filter(|m| m.who != agent("a").who)
            .cloned()
            .collect();
        rejected_without_change(
            &mut a,
            "b",
            Payload::Members {
                chat: id.clone(),
                members: no_foreign,
            },
        );
        let mut foreign = members.clone();
        foreign.push(Member {
            who: Participant::Persona {
                owner: PeerId::from("a"),
                persona: PersonaId::from("neu"),
            },
            name: "Neu".into(),
            role: Role::Agent,
        });
        rejected_without_change(
            &mut a,
            "b",
            Payload::Members {
                chat: id.clone(),
                members: foreign,
            },
        );
        let mut lead = members.clone();
        lead[0].role = Role::Lead;
        rejected_without_change(
            &mut a,
            "b",
            Payload::Members {
                chat: id.clone(),
                members: lead,
            },
        );
        let mut wrong_agent = members.clone();
        wrong_agent
            .iter_mut()
            .find(|m| m.who.is_agent())
            .unwrap()
            .role = Role::Developer;
        rejected_without_change(
            &mut a,
            "b",
            Payload::Members {
                chat: id.clone(),
                members: wrong_agent,
            },
        );
        let own_removed = members
            .into_iter()
            .filter(|m| m.who != agent("b").who)
            .collect();
        let out = b.set_members(&id, own_removed).unwrap();
        deliver(&b, &mut a, out);
        assert_eq!(a.chat(&id).unwrap().members, b.chat(&id).unwrap().members);
        let mut updated = b.chat(&id).unwrap().members.clone();
        updated.push(agent("b"));
        let out = b.set_members(&id, updated).unwrap();
        deliver(&b, &mut a, out);
        assert!(a.chat(&id).unwrap().member(&agent("b").who).is_some());
    }
    #[test]
    fn membership_outbox_old_union_new_and_history_free_invite() {
        let (mut a, mut b, id) = joined(None);
        a.add_contact(contact("c", true)).unwrap();
        a.post(&id, a.me_participant(), text("privater alter Verlauf"))
            .unwrap();
        let mut members = a.chat(&id).unwrap().members.clone();
        members.push(member("c"));
        let out = a.set_members(&id, members).unwrap();
        assert_eq!(
            out.iter()
                .filter(|(_, p)| matches!(p, Payload::Invite { .. }))
                .count(),
            1
        );
        assert_eq!(
            out.iter()
                .filter(|(_, p)| matches!(p, Payload::Members { .. }))
                .count(),
            2
        );
        assert!(out.iter().any(|(d, p)| d == &PeerId::from("c")
            && matches!(p, Payload::Invite { chat, .. } if chat.messages.is_empty())));
        deliver(&a, &mut b, out);
        // B entfernt den eigenen Agenten selbst, bevor A B als Mensch entfernt.
        let members = b
            .chat(&id)
            .unwrap()
            .members
            .iter()
            .filter(|m| m.who != agent("b").who)
            .cloned()
            .collect();
        let out = b.set_members(&id, members).unwrap();
        deliver(&b, &mut a, out);
        let mut members = a.chat(&id).unwrap().members.clone();
        members.retain(|m| m.who.device() != &b.me);
        let out = a.set_members(&id, members).unwrap();
        assert!(out.iter().any(|(d, _)| d == &b.me));
        deliver(&a, &mut b, out);
        assert!(b.chat(&id).unwrap().archived);
        assert!(matches!(
            b.chat(&id).unwrap().messages.last().unwrap().body,
            Body::System { .. }
        ));
    }
    #[test]
    fn invite_validates_members_founder_size_and_verification() {
        let (mut a, mut b) = pair();
        let space = a.create_space("Seminar", SpaceKind::Seminar).unwrap();
        a.contacts[0].verified = false;
        assert!(
            a.create_group("Seminar", Some(space.clone()), vec![member("b")])
                .unwrap_err()
                .contains("geprüfte")
        );
        assert!(a.chats.is_empty());
        a.contacts[0].verified = true;
        let (_, out) = a
            .create_group("Seminar", Some(space), vec![member("b")])
            .unwrap();
        let valid = out[0].1.clone();
        let mut bad = valid.clone();
        if let Payload::Invite { chat, .. } = &mut bad {
            chat.members.retain(|m| m.who != member("b").who);
        }
        rejected_without_change(&mut b, "a", bad);
        let mut bad = valid.clone();
        if let Payload::Invite { chat, .. } = &mut bad {
            chat.members.retain(|m| m.who != member("a").who);
        }
        rejected_without_change(&mut b, "a", bad);
        let mut bad = valid.clone();
        if let Payload::Invite {
            space: Some(s),
            chat,
            ..
        } = &mut bad
        {
            s.founder = PeerId::from("b");
            for m in s.members.iter_mut().chain(chat.members.iter_mut()) {
                if m.who.is_human() {
                    m.role = if m.who.device() == &s.founder {
                        Role::Lead
                    } else {
                        Role::Attendee
                    };
                }
            }
        }
        rejected_without_change(&mut b, "a", bad);
        let mut bad = valid.clone();
        if let Payload::Invite { chat, .. } = &mut bad {
            chat.members[0].role = Role::Lead;
        }
        rejected_without_change(&mut b, "a", bad);
        let mut bad = valid.clone();
        if let Payload::Invite { chat, .. } = &mut bad {
            chat.messages.push(Message {
                id: MsgId(random_id()),
                author: a.me_participant(),
                body: text("Verlauf"),
                ts: 0,
                delivery: Delivery::Local,
            });
        }
        rejected_without_change(&mut b, "a", bad);
        b.contacts[0].verified = false;
        rejected_without_change(&mut b, "a", valid.clone());
        b.contacts[0].verified = true;
        b.apply(&a.me, valid).unwrap();
        b.contacts[0].blocked = true;
        let id = b.invites[0].chat.id.clone();
        assert!(b.accept_invite(&id).is_err());
        assert_eq!(b.invites.len(), 1);
    }
    #[test]
    fn known_seminar_cannot_change_founder_and_unknown_relay_rejected() {
        let (mut a, mut b, id) = joined(Some(SpaceKind::Seminar));
        b.add_contact(contact("c", true)).unwrap();
        let mut c = a.chat(&id).unwrap().clone();
        c.id = ChatId(random_id());
        c.messages.clear();
        c.members.push(Member {
            role: Role::Attendee,
            ..member("c")
        });
        let s = a.space(c.space.as_ref().unwrap()).unwrap().clone();
        // Bekanntes Seminar darf von einem Mitglied mit gleichem Vertrauensanker kommen.
        b.apply(
            &PeerId::from("c"),
            Payload::Invite {
                chat: c.clone(),
                space: Some(s.clone()),
                note: "".into(),
            },
        )
        .unwrap();
        let mut unknown = ChatStore::new(PeerId::from("b"), "Berta");
        unknown.add_contact(contact("c", true)).unwrap();
        rejected_without_change(
            &mut unknown,
            "c",
            Payload::Invite {
                chat: c.clone(),
                space: Some(s.clone()),
                note: "".into(),
            },
        );
        let mut altered = s;
        altered.founder = PeerId::from("c");
        for m in altered.members.iter_mut().chain(c.members.iter_mut()) {
            if m.who.is_human() {
                m.role = if m.who.device() == &altered.founder {
                    Role::Lead
                } else {
                    Role::Attendee
                };
            }
        }
        altered.members.push(Member {
            role: Role::Lead,
            ..member("c")
        });
        rejected_without_change(
            &mut b,
            "c",
            Payload::Invite {
                chat: c,
                space: Some(altered),
                note: "".into(),
            },
        );
        // Auch die lokale Leitung darf nicht ungeprüft neu einladen.
        a.contacts[0].verified = false;
        let members = a.chat(&id).unwrap().members.clone();
        assert!(a.set_members(&id, members).is_ok()); // kein neues Gerät
    }
    #[test]
    fn invite_flood_replacement_decline_and_answer_authorization() {
        let (mut a, mut b) = pair();
        let (id, out) = a.create_group("Gruppe", None, vec![member("b")]).unwrap();
        let template = out[0].1.clone();
        for i in 0..MAX_PENDING_INVITES {
            let mut p = template.clone();
            if let Payload::Invite { chat, .. } = &mut p {
                chat.id = ChatId(format!("invite{i}"));
            }
            b.apply(&a.me, p).unwrap();
        }
        rejected_without_change(&mut b, "a", template.clone());
        let mut replacement = template;
        if let Payload::Invite { chat, note, .. } = &mut replacement {
            chat.id = ChatId::from("invite0");
            *note = "ersetzt".into();
        }
        b.apply(&a.me, replacement).unwrap();
        assert_eq!(b.invites.len(), MAX_PENDING_INVITES);
        assert_eq!(b.invites.last().unwrap().note, "ersetzt");
        assert!(b.decline_invite(&ChatId::from("unbekannt")).is_empty());
        let out = b.decline_invite(&ChatId::from("invite0"));
        assert_eq!(out.len(), 1);
        b.invites.clear();
        let out = a.invite_outbox(a.chat(&id).unwrap(), vec![b.me.clone()]);
        deliver(&a, &mut b, out);
        let out = b.decline_invite(&id);
        deliver(&b, &mut a, out);
        assert!(!has_device(a.chat(&id).unwrap(), &b.me));
        a.add_contact(contact("x", true)).unwrap();
        for p in [
            Payload::InviteAnswer {
                chat: id.clone(),
                accept: true,
            },
            Payload::InviteAnswer {
                chat: id.clone(),
                accept: false,
            },
            Payload::Leave { chat: id.clone() },
            Payload::Ack {
                chat: id,
                msg: MsgId::from("x"),
            },
        ] {
            rejected_without_change(&mut a, "x", p);
        }
    }
    #[test]
    fn ack_only_own_pending_messages_and_remote_payloads_rejected() {
        let (mut a, mut b, id) = joined(None);
        let (_, out) = b.post(&id, b.me_participant(), text("von b")).unwrap();
        let ack = deliver(&b, &mut a, out);
        deliver(&a, &mut b, ack);
        let msg = a.chat(&id).unwrap().messages[0].id.clone();
        assert_eq!(
            a.apply(
                &b.me,
                Payload::Ack {
                    chat: id.clone(),
                    msg
                }
            )
            .unwrap()
            .0,
            Applied::Nothing
        );
        assert_eq!(
            a.chat(&id).unwrap().messages[0].delivery,
            Delivery::Received
        );
        rejected_without_change(
            &mut b,
            "a",
            Payload::RemoteEnd {
                req: "x".into(),
                reason: "x".into(),
            },
        );
    }
    #[test]
    fn leave_removes_device_agents_and_archives_local() {
        let (mut a, mut b, id) = joined(None);
        let out = b.leave(&id);
        assert_eq!(out.len(), 1);
        deliver(&b, &mut a, out);
        assert!(b.chat(&id).unwrap().archived);
        assert!(
            !a.chat(&id)
                .unwrap()
                .members
                .iter()
                .any(|m| m.who.device() == &b.me)
        );
        assert!(b.leave(&id).is_empty());
        assert!(b.post(&id, b.me_participant(), text("zu spät")).is_err());
        rejected_without_change(&mut b, "a", post(&id, a.me_participant()));
    }
    #[test]
    fn limits_sorting_muting_notes_and_retention() {
        let (mut a, mut b, id) = joined(None);
        b.set_muted(&id, true);
        for _ in 0..3 {
            let (_, out) = a.post(&id, a.me_participant(), text("still")).unwrap();
            deliver(&a, &mut b, out);
        }
        assert_eq!(b.chat(&id).unwrap().unread, 0);
        b.set_muted(&id, false);
        b.chat_mut(&id).unwrap().unread = 3;
        b.mark_read(&id);
        assert_eq!(b.chat(&id).unwrap().unread, 0);
        for i in 0..MAX_MESSAGES_PER_CHAT + 5 {
            a.note(&id, &format!("Hinweis {i}"));
        }
        assert_eq!(a.chat(&id).unwrap().messages.len(), MAX_MESSAGES_PER_CHAT);
        assert_eq!(a.chat(&id).unwrap().messages[0].body.text(), "Hinweis 5");
        let (local, _) = a.open_direct(agent("a")).unwrap();
        a.set_pinned(&local, true);
        assert_eq!(a.sorted(ChatFilter::All)[0].id, local);
        assert_eq!(a.sorted(ChatFilter::Direct).len(), 1);
        assert_eq!(a.sorted(ChatFilter::Mixed).len(), 2);
        assert_eq!(a.sorted(ChatFilter::Agents).len(), 0);
        a.set_archived(&local, true);
        assert_eq!(a.sorted(ChatFilter::Archived).len(), 1);
        assert!(a.sorted(ChatFilter::All).iter().all(|c| !c.archived));
        assert!(a.sorted(ChatFilter::Direct).is_empty());
        let mut many = vec![member("a"); MAX_MEMBERS + 1];
        assert!(a.create_group("zu viele", None, many.clone()).is_err());
        many.truncate(2);
        assert!(a.create_group("doppelt", None, many).is_err());
        let template = a.chat(&id).unwrap().clone();
        a.chats = (0..MAX_CHATS)
            .map(|i| {
                let mut c = template.clone();
                c.id = ChatId(i.to_string());
                c
            })
            .collect();
        assert!(a.create_group("zu viel", None, vec![]).is_err());
    }
    #[test]
    fn invitation_disables_automation_and_local_ui_flags() {
        let (mut a, mut b) = pair();
        let (id, _) = a.create_group("Gruppe", None, vec![member("b")]).unwrap();
        let c = a.chat_mut(&id).unwrap();
        c.a2a = Some(crate::a2a::A2aPolicy {
            enabled: true,
            ..Default::default()
        });
        c.pinned = true;
        c.muted = true;
        c.unread = 999;
        let out = a.invite_outbox(a.chat(&id).unwrap(), vec![b.me.clone()]);
        deliver(&a, &mut b, out);
        let c = &b.invites[0].chat;
        assert!(c.a2a.is_none());
        assert!(!c.pinned && !c.muted && c.unread == 0);
    }
    #[test]
    fn removing_human_archives_even_if_their_agent_remains() {
        let (mut a, mut b, id) = joined(None);
        let members = a
            .chat(&id)
            .unwrap()
            .members
            .iter()
            .filter(|m| m.who != b.me_participant())
            .cloned()
            .collect();
        let out = a.set_members(&id, members).unwrap();
        deliver(&a, &mut b, out);
        assert!(b.chat(&id).unwrap().archived);
        assert!(b.chat(&id).unwrap().member(&agent("b").who).is_some());
        assert!(
            b.post(&id, agent("b").who, text("nicht mehr erlaubt"))
                .is_err()
        );
    }
    #[test]
    fn incoming_members_and_step_require_known_authority() {
        let (mut a, mut b) = pair();
        let (id, out) = a.open_direct(member("b")).unwrap();
        deliver(&a, &mut b, out);
        let out = b.accept_invite(&id).unwrap();
        deliver(&b, &mut a, out);
        rejected_without_change(
            &mut b,
            "a",
            Payload::Members {
                chat: id,
                members: vec![member("a"), member("b")],
            },
        );
        rejected_without_change(
            &mut b,
            "a",
            Payload::Step {
                space: SpaceId::from("unbekannt"),
                step: 0,
            },
        );
        let project = b.create_space("Projekt", SpaceKind::Project).unwrap();
        rejected_without_change(
            &mut b,
            "a",
            Payload::Step {
                space: project,
                step: 0,
            },
        );
        rejected_without_change(
            &mut b,
            "a",
            Payload::Members {
                chat: ChatId::from("unbekannt"),
                members: vec![member("a"), member("b")],
            },
        );
    }
    #[test]
    fn incoming_invites_reject_member_overflow_duplicates_and_bad_dm() {
        let (mut a, mut b) = pair();
        let (_, out) = a.create_group("Gruppe", None, vec![member("b")]).unwrap();
        let valid = out[0].1.clone();
        let mut oversized = valid.clone();
        if let Payload::Invite { chat, .. } = &mut oversized {
            chat.members
                .extend((0..MAX_MEMBERS).map(|i| member(&format!("x{i}"))));
        }
        rejected_without_change(&mut b, "a", oversized);
        let mut duplicate = valid.clone();
        if let Payload::Invite { chat, .. } = &mut duplicate {
            chat.members.push(member("a"));
        }
        rejected_without_change(&mut b, "a", duplicate);
        let mut direct = valid.clone();
        if let Payload::Invite { chat, .. } = &mut direct {
            chat.kind = ChatKind::Direct;
            chat.members.push(member("c"));
        }
        rejected_without_change(&mut b, "a", direct);
        let mut bad_room = valid;
        if let Payload::Invite { chat, .. } = &mut bad_room {
            chat.space = Some(SpaceId::from("fehlt"));
        }
        rejected_without_change(&mut b, "a", bad_room);
    }
    #[test]
    fn failed_accept_at_capacity_keeps_invitation_and_pending_is_persisted() {
        let (mut a, mut b) = pair();
        let (id, out) = a.create_group("Gruppe", None, vec![member("b")]).unwrap();
        deliver(&a, &mut b, out);
        let dir = std::env::temp_dir().join(format!("cocopi-pending-{}", random_id()));
        let path = dir.join("chats.json");
        b.save(&path).unwrap();
        let loaded = ChatStore::load(&path).unwrap().unwrap();
        assert_eq!(b.invites, loaded.invites);
        let template = b.invites[0].chat.clone();
        b.chats = (0..MAX_CHATS)
            .map(|i| {
                let mut c = template.clone();
                c.id = ChatId(format!("local{i}"));
                c
            })
            .collect();
        assert!(b.accept_invite(&id).is_err());
        assert_eq!(b.invites.len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn received_messages_are_bounded_and_muted_counts_do_not_grow() {
        let (a, mut b, id) = joined(None);
        b.set_muted(&id, true);
        let template = Message {
            id: MsgId::from("old"),
            author: a.me_participant(),
            body: text("alt"),
            ts: 1,
            delivery: Delivery::Received,
        };
        b.chat_mut(&id).unwrap().messages = (0..MAX_MESSAGES_PER_CHAT)
            .map(|i| {
                let mut m = template.clone();
                m.id = MsgId(i.to_string());
                m
            })
            .collect();
        b.apply(&a.me, post(&id, a.me_participant())).unwrap();
        assert_eq!(b.chat(&id).unwrap().messages.len(), MAX_MESSAGES_PER_CHAT);
        assert_eq!(b.chat(&id).unwrap().messages[0].id, MsgId::from("1"));
        assert_eq!(b.chat(&id).unwrap().unread, 0);
    }
    #[test]
    fn all_remote_variants_belong_to_remote_book() {
        let (_, mut b) = pair();
        use crate::remote::{RemoteFrame, RemoteScope, SessionRef};
        let variants = [
            Payload::RemoteRequest {
                req: "r".into(),
                space: SpaceId::from("s"),
                scope: RemoteScope::View,
                minutes: 1,
                reason: "test".into(),
            },
            Payload::RemoteGrant {
                req: "r".into(),
                scope: RemoteScope::View,
                expires_ms: 0,
                session: SessionRef {
                    cwd: "".into(),
                    label: "test".into(),
                },
            },
            Payload::RemoteEnd {
                req: "r".into(),
                reason: "test".into(),
            },
            Payload::RemoteFrame {
                req: "r".into(),
                frame: RemoteFrame {
                    seq: 0,
                    reset: true,
                    items: vec![],
                    state: "".into(),
                },
            },
            Payload::RemoteInput {
                req: "r".into(),
                text: "test".into(),
            },
        ];
        for p in variants {
            let err = b.apply(&PeerId::from("a"), p).unwrap_err();
            assert_eq!(err, "gehört zum Fernzugriff");
        }
    }
    #[test]
    fn persistence_roundtrip_corruption_and_atomic_failure() {
        let (mut a, _, id) = joined(Some(SpaceKind::Seminar));
        a.note(&id, "gespeichert");
        let dir = std::env::temp_dir().join(format!("cocopi-store-{}", random_id()));
        let path = dir.join("nested/chats.json");
        assert!(ChatStore::load(&path).unwrap().is_none());
        a.save(&path).unwrap();
        let loaded = ChatStore::load(&path).unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(loaded).unwrap()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            a.save(&path).unwrap();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let old = std::fs::read(&path).unwrap();
        std::fs::write(path.with_file_name("chats.json.new"), b"Rest").unwrap();
        assert!(a.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), old);
        std::fs::write(&path, b"kaputt").unwrap();
        assert!(ChatStore::load(&path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
