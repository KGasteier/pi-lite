//! Produktiver Seminartransport auf Node. Keine Rollen aus Roster/Client-JSON.
//! Der Besitzer pollt regelmäßig (auch bei verborgenem Tab); nur geprüfte Pakete
//! verlassen diesen Adapter. Kein stiller Netzstart, kein Dateisystemzugriff.
use super::{SeminarNet, Von};
use crate::{
    ausweis::{Ausweis, Bestaetigt},
    identity::Identity,
    model::{PeerId, now_ms, random_id},
    node::{Node, NodeEvent, Outgoing, parse_address},
    remote::SessionRef,
    wire::{Envelope, Payload},
};
use std::collections::{BTreeMap, VecDeque};

const HEARTBEAT_MS: u64 = 15_000;
pub const PRESENCE_TIMEOUT_MS: u64 = 60_000;
const PACKET_TTL_MS: u64 = 30_000;
const MAX_SEEN: usize = 512;
const MAX_CONTACTS: usize = 64;

/// Implementierung benutzt den eigenen, bereits angemeldeten Slot-Hub.
/// Erfolg heißt vom Hub bestätigt, nicht nur in eine Warteschlange gelegt.
pub trait SeminarHub {
    fn senden(&mut self, session: &SessionRef, text: &str, von: &Von) -> Result<(), String>;
}

pub struct NodeNet {
    node: Node,
    own: Ausweis,
    issuer: String,
    contacts: BTreeMap<PeerId, String>,
    verified: BTreeMap<PeerId, Bestaetigt>,
    last_seen: BTreeMap<PeerId, u64>,
    seen: BTreeMap<PeerId, VecDeque<(String, u64)>>,
    presence: Vec<(PeerId, bool, u64)>,
    errors: VecDeque<String>,
    address: Option<String>,
    last_heartbeat: u64,
    stopped: bool,
    hub: Box<dyn SeminarHub>,
}
impl NodeNet {
    /// Erst nach bestätigtem Empfangs-Pin und explizitem Netzstart aufrufen.
    pub fn start(
        identity: Identity,
        own: Ausweis,
        confirmed_issuer: String,
        hub: Box<dyn SeminarHub>,
    ) -> Result<Self, String> {
        own.pruefen(&confirmed_issuer, &identity.peer(), now_ms() / 1000)?;
        Ok(Self {
            node: Node::start_with_ausweis(identity, Some(own.clone())),
            own,
            issuer: confirmed_issuer,
            contacts: BTreeMap::new(),
            verified: BTreeMap::new(),
            last_seen: BTreeMap::new(),
            seen: BTreeMap::new(),
            presence: vec![],
            errors: VecDeque::new(),
            address: None,
            last_heartbeat: 0,
            stopped: false,
            hub,
        })
    }
    pub fn peer(&self) -> &PeerId {
        &self.node.me
    }
    /// Enthält direkte IP-Adressen. Nur bewusst an Seminarteilnehmende teilen.
    pub fn address(&self) -> Option<&str> {
        self.address.as_deref()
    }
    pub fn set_contacts(&mut self, peers: Vec<(PeerId, String)>) -> Result<(), String> {
        if self.stopped || peers.len() > MAX_CONTACTS {
            return Err("Seminarnetz beendet oder zu viele Kontakte".into());
        }
        let mut contacts = BTreeMap::new();
        for (peer, address) in &peers {
            if peer == self.peer()
                || parse_address(address)? != *peer
                || contacts.insert(peer.clone(), address.clone()).is_some()
            {
                return Err("Kontaktadresse passt nicht oder ist doppelt".into());
            }
        }
        let removed: Vec<_> = self
            .contacts
            .keys()
            .filter(|p| !contacts.contains_key(*p))
            .cloned()
            .collect();
        for peer in removed {
            self.forget(&peer);
        }
        self.contacts = contacts;
        self.node.set_allowed(peers);
        // Eigener Ausweis ist die einzige Nachricht, die ohne Peer-Ausweis rausgeht.
        for peer in self.contacts.keys() {
            self.raw(
                peer,
                Payload::Geraeteausweis {
                    ausweis: self.own.clone(),
                },
            )?;
        }
        Ok(())
    }
    fn raw(&self, to: &PeerId, payload: Payload) -> Result<(), String> {
        self.node.send(Outgoing {
            to: to.clone(),
            env: Envelope::new(payload),
            tag: None,
        })
    }
    fn error(&mut self, error: String) {
        if self.errors.len() == 32 {
            self.errors.pop_front();
        }
        self.errors.push_back(error);
    }
    fn forget(&mut self, peer: &PeerId) {
        self.verified.remove(peer);
        let since = self.last_seen.remove(peer).unwrap_or_else(now_ms);
        self.seen.remove(peer);
        self.presence.push((peer.clone(), false, since));
    }
    fn receive(
        &mut self,
        from: PeerId,
        payload: Payload,
        now: u64,
        out: &mut Vec<(PeerId, Payload)>,
    ) {
        if !self.contacts.contains_key(&from) {
            return;
        }
        match payload {
            Payload::Geraeteausweis { ausweis } => {
                match ausweis.pruefen(&self.issuer, &from, now / 1000) {
                    Ok(cert) => {
                        self.verified.insert(from.clone(), cert);
                        self.last_seen.insert(from.clone(), now);
                        self.presence.push((from.clone(), true, now));
                        out.push((from, Payload::Geraeteausweis { ausweis }));
                    }
                    Err(e) => {
                        self.forget(&from);
                        self.error(e);
                    }
                }
            }
            Payload::SeminarPacket {
                id,
                sent_ms,
                payload,
            } => {
                if !self
                    .verified
                    .get(&from)
                    .is_some_and(|c| c.gueltig(now / 1000))
                {
                    self.error("Nachricht ohne gültigen Geräteausweis verworfen".into());
                    return;
                }
                if id.len() != 32
                    || !id.bytes().all(|b| b.is_ascii_hexdigit())
                    || now.saturating_sub(sent_ms) > PACKET_TTL_MS
                    || sent_ms > now.saturating_add(5_000)
                    || matches!(
                        *payload,
                        Payload::SeminarPacket { .. } | Payload::Geraeteausweis { .. }
                    )
                {
                    self.error("Seminarnachricht veraltet oder ungültig".into());
                    return;
                }
                let seen = self.seen.entry(from.clone()).or_default();
                while seen
                    .front()
                    .is_some_and(|(_, t)| now.saturating_sub(*t) > PACKET_TTL_MS)
                {
                    seen.pop_front();
                }
                if seen.iter().any(|(previous, _)| *previous == id) {
                    return;
                }
                if seen.len() >= MAX_SEEN {
                    self.error("Zu viele Seminarnachrichten; bitte kurz warten".into());
                    return;
                }
                seen.push_back((id, now));
                self.last_seen.insert(from.clone(), now);
                out.push((from, *payload));
            }
            // Keine Legacy-Nutzlast am Seminaradapter vorbei; Node bleibt v1-kompatibel.
            _ => self.error("Seminarnachricht ohne geprüfte Hülle verworfen".into()),
        }
    }
    fn poll_at(&mut self, now: u64) -> Vec<(PeerId, Payload)> {
        if self.stopped {
            return vec![];
        }
        let mut out = vec![];
        // Bounded UI-Arbeit pro Frame, der Node begrenzt zusätzlich Rate/Größe.
        for _ in 0..256 {
            let Ok(event) = self.node.events.try_recv() else {
                break;
            };
            match event {
                NodeEvent::Address(address) => self.address = Some(address),
                NodeEvent::Received { from, env } => self.receive(from, env.payload, now, &mut out),
                NodeEvent::Failed { to, error, .. } => self.error(format!(
                    "Nicht zugestellt an {}: {error}",
                    crate::identity::short(&to)
                )),
                NodeEvent::Rejected { why, .. } | NodeEvent::Error(why) => self.error(why),
                NodeEvent::Stopped => {
                    let peers: Vec<_> = self.verified.keys().cloned().collect();
                    for peer in peers {
                        self.forget(&peer);
                    }
                    self.stopped = true;
                }
                // Transport-Erreichbarkeit alleine bestätigt weder Identität noch Presence.
                NodeEvent::Reachable { .. } | NodeEvent::Delivered { .. } => {}
            }
        }
        if self
            .own
            .pruefen(&self.issuer, self.peer(), now / 1000)
            .is_err()
        {
            self.checkout();
            self.error("Dein Geräteausweis ist abgelaufen. Checke erneut ein.".into());
            return vec![];
        }
        let stale: Vec<_> = self
            .verified
            .iter()
            .filter(|(peer, cert)| {
                !cert.gueltig(now / 1000)
                    || self
                        .last_seen
                        .get(*peer)
                        .is_none_or(|t| now.saturating_sub(*t) > PRESENCE_TIMEOUT_MS)
            })
            .map(|(peer, _)| peer.clone())
            .collect();
        for peer in stale {
            self.forget(&peer);
        }
        if now.saturating_sub(self.last_heartbeat) >= HEARTBEAT_MS {
            self.last_heartbeat = now;
            for peer in self.contacts.keys().cloned().collect::<Vec<_>>() {
                if let Err(e) = self.raw(
                    &peer,
                    Payload::Geraeteausweis {
                        ausweis: self.own.clone(),
                    },
                ) {
                    self.error(e);
                }
            }
        }
        out
    }
}
impl SeminarNet for NodeNet {
    fn send(&mut self, to: &PeerId, payload: Payload) -> Result<(), String> {
        let now = now_ms();
        if self.stopped
            || self
                .own
                .pruefen(&self.issuer, self.peer(), now / 1000)
                .is_err()
            || !self.verified.get(to).is_some_and(|c| c.gueltig(now / 1000))
        {
            return Err("Die Person ist nicht mit gültigem Empfangsausweis verbunden".into());
        }
        if matches!(
            payload,
            Payload::Geraeteausweis { .. } | Payload::SeminarPacket { .. }
        ) {
            return Err("Diese Nachricht ist dem Seminartransport vorbehalten".into());
        }
        self.raw(
            to,
            Payload::SeminarPacket {
                id: random_id(),
                sent_ms: now,
                payload: Box::new(payload),
            },
        )
    }
    fn senden(&mut self, session: &SessionRef, text: &str, von: &Von) -> Result<(), String> {
        if self.stopped {
            return Err("Seminarnetz beendet".into());
        }
        self.hub.senden(session, text, von)
    }
    fn poll(&mut self) -> Vec<(PeerId, Payload)> {
        self.poll_at(now_ms())
    }
    fn presence(&mut self) -> Vec<(PeerId, bool, u64)> {
        std::mem::take(&mut self.presence)
    }
    fn errors(&mut self) -> Vec<String> {
        self.errors.drain(..).collect()
    }
    fn set_contacts(&mut self, peers: Vec<(PeerId, String)>) -> Result<(), String> {
        NodeNet::set_contacts(self, peers)
    }
    fn checkout(&mut self) {
        self.node.set_allowed(vec![]);
        self.node.stop();
        let peers: Vec<_> = self.verified.keys().cloned().collect();
        for peer in peers {
            self.forget(&peer);
        }
        self.contacts.clear();
        self.address = None;
        self.stopped = true;
    }
}
