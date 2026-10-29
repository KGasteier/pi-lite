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
    wire::{Envelope, Payload, SessionAnkuendigung},
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
    fn update_sessions(&mut self, _sessions: &[SessionAnkuendigung]) -> Result<(), String> {
        Err("Dieser Hub unterstützt keine Session-Aktualisierung".into())
    }
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
        self.seen.retain(|peer, _| self.contacts.contains_key(peer));
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
        // Über Ausweis-/Verbindungswechsel behalten: Retry bleibt dedupliziert.
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
                        self.last_seen.entry(from.clone()).or_insert(now);
                        // Wiederholte Ausweis-Pakete sind kein frischer Lebensnachweis.
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
                if let Payload::SeminarSessions { sessions } = payload.as_ref() {
                    if let Err(error) = SessionAnkuendigung::pruefen(sessions) {
                        self.error(error);
                        return;
                    }
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
                // Bis zum Ende der Gültigkeit aufbewahren, auch wenn die
                // Senderuhr innerhalb der erlaubten 5 s leicht vorausläuft.
                seen.push_back((id, sent_ms));
                self.last_seen.insert(from.clone(), now);
                self.presence.push((from.clone(), true, now));
                if !matches!(*payload, Payload::SeminarPresence) {
                    out.push((from, *payload));
                }
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
                        .is_none_or(|t| now.saturating_sub(*t) >= PRESENCE_TIMEOUT_MS)
            })
            .map(|(peer, _)| peer.clone())
            .collect();
        for peer in stale {
            self.forget(&peer);
        }
        if now.saturating_sub(self.last_heartbeat) >= HEARTBEAT_MS {
            self.last_heartbeat = now;
            for peer in self.contacts.keys().cloned().collect::<Vec<_>>() {
                if let Err(e) = self
                    .raw(
                        &peer,
                        Payload::Geraeteausweis {
                            ausweis: self.own.clone(),
                        },
                    )
                    .and_then(|()| {
                        self.raw(
                            &peer,
                            Payload::SeminarPacket {
                                id: random_id(),
                                sent_ms: now,
                                payload: Box::new(Payload::SeminarPresence),
                            },
                        )
                    })
                {
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
            || self
                .last_seen
                .get(to)
                .is_none_or(|t| now.saturating_sub(*t) >= PRESENCE_TIMEOUT_MS)
        {
            return Err("Die Person ist nicht mit gültigem Empfangsausweis verbunden".into());
        }
        if matches!(
            payload,
            Payload::Geraeteausweis { .. } | Payload::SeminarPacket { .. }
        ) {
            return Err("Diese Nachricht ist dem Seminartransport vorbehalten".into());
        }
        if let Payload::SeminarSessions { sessions } = &payload {
            SessionAnkuendigung::pruefen(sessions)?;
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
        let now = now_ms();
        if self.stopped
            || self
                .own
                .pruefen(&self.issuer, self.peer(), now / 1000)
                .is_err()
            || !self.verified.iter().any(|(peer, cert)| {
                cert.gueltig(now / 1000)
                    && cert.person() == von.person
                    && cert.rolle() == von.rolle
                    && self
                        .last_seen
                        .get(peer)
                        .is_some_and(|t| now.saturating_sub(*t) < PRESENCE_TIMEOUT_MS)
            })
        {
            return Err("Mitsteuern braucht eine aktuell bestätigte Person".into());
        }
        self.hub.senden(session, text, von)
    }
    fn poll(&mut self) -> Vec<(PeerId, Payload)> {
        self.poll_at(now_ms())
    }
    fn presence(&mut self) -> Vec<(PeerId, bool, u64)> {
        // Doppelte Heartbeats innerhalb eines Frames auf den letzten Status kürzen.
        let mut latest = BTreeMap::new();
        for (peer, online, since) in self.presence.drain(..) {
            latest.insert(peer, (online, since));
        }
        latest
            .into_iter()
            .map(|(peer, (online, since))| (peer, online, since))
            .collect()
    }
    fn errors(&mut self) -> Vec<String> {
        self.errors.drain(..).collect()
    }
    fn address(&self) -> Option<&str> {
        NodeNet::address(self)
    }
    fn set_contacts(&mut self, peers: Vec<(PeerId, String)>) -> Result<(), String> {
        NodeNet::set_contacts(self, peers)
    }
    fn update_sessions(&mut self, sessions: &[SessionAnkuendigung]) -> Result<(), String> {
        SessionAnkuendigung::pruefen(sessions)?;
        self.hub.update_sessions(sessions)
    }
    fn checkout(&mut self) {
        self.node.set_allowed(vec![]);
        self.node.stop();
        let peers: Vec<_> = self.verified.keys().cloned().collect();
        for peer in peers {
            self.forget(&peer);
        }
        self.contacts.clear();
        self.seen.clear();
        self.address = None;
        self.stopped = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ausweis::{Inhalt, Rolle, base64url, kanonisch},
        remote::{GrantState, RemoteScope},
        seminar::{HubPrompt, Person, SeminarState},
        store::Outbox,
    };
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    #[derive(Clone, Default)]
    struct MockHub(Arc<Mutex<Vec<HubPrompt>>>);
    impl SeminarHub for MockHub {
        fn senden(&mut self, session: &SessionRef, text: &str, von: &Von) -> Result<(), String> {
            self.0.lock().unwrap().push(HubPrompt {
                session: session.clone(),
                text: text.into(),
                von: von.clone(),
            });
            Ok(())
        }
    }
    fn certificate(issuer: &iroh::SecretKey, peer: &PeerId, role: Rolle, now: u64) -> Ausweis {
        let inhalt = Inhalt {
            v: 1,
            typ: "geraeteausweis".into(),
            pid: peer.to_string(),
            person: if role == Rolle::Seminarleitung {
                "Aylin"
            } else {
                "Mira"
            }
            .into(),
            rolle: role,
            net_pubkey: peer.to_string(),
            geraet_pubkey: base64url(&[77; 32]),
            iat: now / 1000 - 1,
            exp: now / 1000 + 3600,
        };
        let signatur = base64url(&issuer.sign(&kanonisch(&inhalt).unwrap()).to_bytes());
        Ausweis { inhalt, signatur }
    }
    fn wait(mut condition: impl FnMut() -> bool) {
        let until = Instant::now() + Duration::from_secs(12);
        while !condition() {
            assert!(
                Instant::now() < until,
                "Lokaler Seminartransport hat nicht rechtzeitig geantwortet"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn loopback(net: &mut NodeNet) -> String {
        wait(|| {
            net.poll();
            net.address().is_some()
        });
        let addr: iroh::EndpointAddr = serde_json::from_str(net.address().unwrap()).unwrap();
        let ips = addr.ip_addrs().map(|socket| {
            let ip = if socket.is_ipv4() { "127.0.0.1" } else { "::1" }
                .parse()
                .unwrap();
            iroh::TransportAddr::Ip(std::net::SocketAddr::new(ip, socket.port()))
        });
        // Kein LAN, Relay oder Discovery: nur Loopback-Adressen in der Allowlist.
        serde_json::to_string(&iroh::EndpointAddr::from_parts(addr.id, ips)).unwrap()
    }
    fn dispatch(net: &mut NodeNet, out: Outbox) {
        for (peer, payload) in out {
            net.send(&peer, payload).unwrap();
        }
    }
    fn pump(state: &mut SeminarState, net: &mut NodeNet, now: u64) -> Vec<String> {
        let incoming = net.poll_at(now);
        for (peer, online, since) in net.presence() {
            state.status(&peer, online, since);
        }
        let mut errors = vec![];
        for (from, payload) in incoming {
            match state.receive(&from, payload, now) {
                Ok(effects) => {
                    dispatch(net, effects.out);
                    for prompt in effects.hub {
                        net.senden(&prompt.session, &prompt.text, &prompt.von)
                            .unwrap();
                    }
                }
                Err(e) => errors.push(e),
            }
        }
        errors
    }
    fn session() -> SessionRef {
        SessionRef {
            label: "Handout".into(),
            cwd: "own-session".into(),
        }
    }
    fn pair() -> (SeminarState, NodeNet, SeminarState, NodeNet, MockHub) {
        let now = now_ms();
        let issuer = iroh::SecretKey::from_bytes(&[71; 32]);
        let pin = base64url(issuer.public().as_bytes());
        let ia = Identity::ephemeral();
        let ib = Identity::ephemeral();
        let ca = certificate(&issuer, &ia.peer(), Rolle::Seminarleitung, now);
        let cb = certificate(&issuer, &ib.peer(), Rolle::Teilnehmende, now);
        let mut a = SeminarState::new(
            ia.peer(),
            "Lokaler Anzeigename",
            pin.clone(),
            "seminar".into(),
        );
        let mut b = SeminarState::new(ib.peer(), "Mira", pin.clone(), "seminar".into());
        a.identify(&ia.peer(), &ca, now).unwrap();
        b.identify(&ib.peer(), &cb, now).unwrap();
        for (state, peer) in [(&mut a, ib.peer()), (&mut b, ia.peer())] {
            state.sessions = vec![session()];
            state
                .add_person(Person {
                    peer,
                    name: "Unbestätigter Rostername".into(),
                    online: false,
                    slot: Some("account01".into()),
                    sessions: vec![],
                    offline_since_ms: None,
                })
                .unwrap();
        }
        let hub = MockHub::default();
        let mut na = NodeNet::start(ia, ca, pin.clone(), Box::new(MockHub::default())).unwrap();
        let mut nb = NodeNet::start(ib, cb, pin, Box::new(hub.clone())).unwrap();
        let aa = loopback(&mut na);
        let ba = loopback(&mut nb);
        na.set_contacts(vec![(nb.peer().clone(), ba)]).unwrap();
        nb.set_contacts(vec![(na.peer().clone(), aa)]).unwrap();
        // set_contacts darf initial vor der gegenseitigen Allowlist senden. Neuer
        // frischer Heartbeat nach vollständiger Konfiguration verhindert Race.
        na.last_heartbeat = 0;
        nb.last_heartbeat = 0;
        wait(|| {
            assert!(pump(&mut a, &mut na, now_ms()).is_empty());
            assert!(pump(&mut b, &mut nb, now_ms()).is_empty());
            a.verified(&b.chats.me, now_ms()).is_some()
                && b.verified(&a.chats.me, now_ms()).is_some()
                && a.people[0].online
                && b.people[0].online
        });
        // Nur eigene Sessions sind bekannt. Fremde Listen kommen über den Draht.
        assert!(a.people[0].sessions.is_empty() && b.people[0].sessions.is_empty());
        let announce = || Payload::SeminarSessions {
            sessions: vec![SessionAnkuendigung {
                id: session().cwd,
                label: session().label,
                zustand: "ruht".into(),
            }],
        };
        na.send(&b.chats.me, announce()).unwrap();
        nb.send(&a.chats.me, announce()).unwrap();
        wait(|| {
            assert!(pump(&mut a, &mut na, now_ms()).is_empty());
            assert!(pump(&mut b, &mut nb, now_ms()).is_empty());
            a.people[0].sessions == vec![session()] && b.people[0].sessions == vec![session()]
        });
        (a, na, b, nb, hub)
    }
    fn request(
        a: &mut SeminarState,
        na: &mut NodeNet,
        b: &mut SeminarState,
        nb: &mut NodeNet,
    ) -> String {
        let out = a
            .request(&b.chats.me, &session(), RemoteScope::Control, now_ms())
            .unwrap();
        let Payload::RemoteRequest { req, .. } = &out[0].1 else {
            panic!()
        };
        let req = req.clone();
        dispatch(na, out);
        wait(|| {
            assert!(pump(b, nb, now_ms()).is_empty());
            b.remote.get(&req).is_some()
        });
        req
    }
    #[test]
    fn two_nodes_consent_von_refusal_expiry_chat_dm_and_presence() {
        let (mut a, mut na, mut b, mut nb, hub) = pair();
        // Bestehende Chat-/DM-Verträge unverändert über denselben QUIC-Kanal.
        dispatch(&mut na, a.create_group(now_ms()).unwrap());
        wait(|| {
            assert!(pump(&mut b, &mut nb, now_ms()).is_empty());
            !b.chats.invites.is_empty()
        });
        let group = b.chats.invites[0].chat.id.clone();
        dispatch(&mut nb, b.chats.accept_invite(&group).unwrap());
        b.group = Some(group.clone());
        b.selected = Some(group.clone());
        dispatch(&mut na, a.post("Hallo Seminar").unwrap());
        wait(|| {
            assert!(pump(&mut b, &mut nb, now_ms()).is_empty());
            !b.chats.chat(&group).unwrap().messages.is_empty()
        });
        dispatch(&mut na, a.direct(&b.chats.me, now_ms()).unwrap());
        wait(|| {
            assert!(pump(&mut b, &mut nb, now_ms()).is_empty());
            !b.chats.invites.is_empty()
        });
        let dm = b.chats.invites[0].chat.id.clone();
        dispatch(&mut nb, b.chats.accept_invite(&dm).unwrap());
        dispatch(&mut na, a.post("Direktnachricht").unwrap());
        wait(|| {
            assert!(pump(&mut b, &mut nb, now_ms()).is_empty());
            !b.chats.chat(&dm).unwrap().messages.is_empty()
        });

        let req = request(&mut a, &mut na, &mut b, &mut nb);
        assert!(hub.0.lock().unwrap().is_empty());
        dispatch(
            &mut nb,
            b.accept(&req, RemoteScope::Control, &session(), now_ms())
                .unwrap(),
        );
        wait(|| {
            assert!(pump(&mut a, &mut na, now_ms()).is_empty());
            a.remote
                .get(&req)
                .is_some_and(|g| matches!(g.state, GrantState::Active { .. }))
        });
        dispatch(
            &mut na,
            a.control(&req, "Bitte gliedern", now_ms()).unwrap(),
        );
        wait(|| {
            assert!(pump(&mut b, &mut nb, now_ms()).is_empty());
            !hub.0.lock().unwrap().is_empty()
        });
        let prompts = hub.0.lock().unwrap().clone();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].text, "Bitte gliedern");
        assert_eq!(prompts[0].von.person, "Aylin");
        assert_eq!(prompts[0].von.rolle, Rolle::Seminarleitung);
        assert_eq!(prompts[0].session, session());

        dispatch(&mut nb, b.end(&req, "Beendet"));
        wait(|| {
            pump(&mut a, &mut na, now_ms());
            a.remote
                .get(&req)
                .is_none_or(|g| matches!(g.state, GrantState::Ended { .. }))
        });
        let refused = request(&mut a, &mut na, &mut b, &mut nb);
        dispatch(&mut nb, b.end(&refused, "Abgelehnt"));
        // Auch ein böswilliger Sender, der die Viewer-UI umgeht, kommt nicht durch.
        dispatch(
            &mut na,
            vec![(
                b.chats.me.clone(),
                Payload::Steuern {
                    req: refused.clone(),
                    text: "Nicht senden".into(),
                },
            )],
        );
        let mut rejected = vec![];
        wait(|| {
            rejected.extend(pump(&mut b, &mut nb, now_ms()));
            !rejected.is_empty()
        });
        assert_eq!(hub.0.lock().unwrap().len(), 1);
        wait(|| {
            pump(&mut a, &mut na, now_ms());
            a.remote
                .get(&refused)
                .is_none_or(|g| matches!(g.state, GrantState::Ended { .. }))
        });

        let expiring = request(&mut a, &mut na, &mut b, &mut nb);
        dispatch(
            &mut nb,
            b.accept(&expiring, RemoteScope::Control, &session(), now_ms())
                .unwrap(),
        );
        wait(|| {
            pump(&mut a, &mut na, now_ms());
            a.remote
                .get(&expiring)
                .is_some_and(|g| matches!(g.state, GrantState::Active { .. }))
        });
        let GrantState::Active {
            expires_ms: expired,
        } = b.remote.get(&expiring).unwrap().state
        else {
            panic!()
        };
        // Nutzlast gelangt noch über echtes QUIC, erst Host prüft die Ablaufzeit.
        dispatch(&mut na, a.control(&expiring, "Zu spät", now_ms()).unwrap());
        let mut rejected = vec![];
        wait(|| {
            let packets = nb.poll();
            for (from, payload) in packets {
                if matches!(payload, Payload::Steuern { .. }) {
                    rejected.push(
                        b.receive(&from, payload, expired)
                            .err()
                            .expect("Abgelaufene Freigabe muss abweisen"),
                    );
                }
            }
            !rejected.is_empty()
        });
        b.tick(expired);
        assert!(
            b.remote
                .get(&expiring)
                .is_none_or(|g| matches!(g.state, GrantState::Ended { .. }))
        );
        assert_eq!(hub.0.lock().unwrap().len(), 1);

        // Zeit injiziert, kein 60-s-Schlaf; letzter authentifizierter Lebensnachweis
        // wird dem UI mitgegeben, damit es nicht weitere 60 Sekunden wartet.
        let last = *nb.last_seen.get(&a.chats.me).unwrap();
        na.checkout();
        nb.poll_at(last + PRESENCE_TIMEOUT_MS + 1);
        assert!(
            nb.presence()
                .iter()
                .any(|(peer, online, since)| peer == &a.chats.me && !online && *since == last)
        );
        assert!(!nb.verified.contains_key(&a.chats.me));
        assert!(nb.send(&a.chats.me, Payload::SeminarPresence).is_err());
        nb.checkout();
    }
    #[test]
    fn oversized_session_lists_cross_quic_but_never_replace_the_roster() {
        let (a, mut na, mut b, mut nb, hub) = pair();
        let before = b.people[0].sessions.clone();
        let item = SessionAnkuendigung {
            id: "wire-session".into(),
            label: "Plakat".into(),
            zustand: "ruht".into(),
        };
        for sessions in [
            (0..33)
                .map(|i| SessionAnkuendigung {
                    id: format!("session-{i}"),
                    ..item.clone()
                })
                .collect(),
            vec![SessionAnkuendigung {
                label: "ä".repeat(121),
                ..item.clone()
            }],
        ] {
            assert!(
                na.send(
                    &b.chats.me,
                    Payload::SeminarSessions {
                        sessions: sessions.clone()
                    }
                )
                .is_err()
            );
            nb.errors();
            // Absichtlich unterhalb der send-Prüfung einspeisen, damit nicht nur
            // der lokale Validator, sondern der reale QUIC-Empfang geprüft wird.
            na.raw(
                &b.chats.me,
                Payload::SeminarPacket {
                    id: random_id(),
                    sent_ms: now_ms(),
                    payload: Box::new(Payload::SeminarSessions { sessions }),
                },
            )
            .unwrap();
            let mut errors = vec![];
            wait(|| {
                assert!(pump(&mut b, &mut nb, now_ms()).is_empty());
                errors.extend(nb.errors());
                errors.iter().any(|e| e.contains("Sessionliste ungültig"))
            });
            assert_eq!(b.people[0].sessions, before);
            assert_eq!(b.session_announcements[&a.chats.me][0].id, session().cwd);
        }
        assert!(hub.0.lock().unwrap().is_empty());
        na.checkout();
        nb.checkout();
    }

    #[test]
    fn session_packets_require_wrapper_contact_certificate_and_valid_bounds() {
        let (a, mut na, _b, mut nb, _) = pair();
        let now = now_ms();
        let item = SessionAnkuendigung {
            id: "hub-1".into(),
            label: "Plakat".into(),
            zustand: "ruht".into(),
        };
        let packet = |sessions| Payload::SeminarPacket {
            id: random_id(),
            sent_ms: now,
            payload: Box::new(Payload::SeminarSessions { sessions }),
        };
        let peer = a.chats.me.clone();
        let mut out = vec![];
        nb.receive(
            peer.clone(),
            Payload::SeminarSessions {
                sessions: vec![item.clone()],
            },
            now,
            &mut out,
        );
        assert!(out.is_empty(), "Ohne SeminarPacket keine Ankündigung");
        nb.receive(peer.clone(), packet(vec![item.clone()]), now, &mut out);
        assert_eq!(out.len(), 1);
        out.clear();
        for sessions in [
            (0..33)
                .map(|i| SessionAnkuendigung {
                    id: format!("hub-{i}"),
                    ..item.clone()
                })
                .collect(),
            vec![SessionAnkuendigung {
                label: "ä".repeat(121),
                ..item.clone()
            }],
        ] {
            nb.receive(peer.clone(), packet(sessions), now, &mut out);
            assert!(out.is_empty(), "Grenzen schon vor der UI prüfen");
        }
        nb.forget(&peer);
        nb.receive(peer.clone(), packet(vec![item.clone()]), now, &mut out);
        assert!(out.is_empty(), "Ohne Ausweis keine Ankündigung");
        nb.receive(
            peer.clone(),
            Payload::Geraeteausweis {
                ausweis: na.own.clone(),
            },
            now,
            &mut out,
        );
        out.clear();
        nb.contacts.remove(&peer);
        nb.receive(peer, packet(vec![item]), now, &mut out);
        assert!(
            out.is_empty(),
            "Ein Ausweis allein ersetzt nicht das Verzeichnis"
        );
        na.checkout();
        nb.checkout();
    }
    #[test]
    fn admission_binding_replay_and_stale_packet_fail_closed() {
        let (a, mut na, b, mut nb, hub) = pair();
        let now = now_ms();
        let mut out = vec![];
        let packet = Payload::SeminarPacket {
            id: random_id(),
            sent_ms: now,
            payload: Box::new(Payload::Steuern {
                req: "not-granted".into(),
                text: "Nie".into(),
            }),
        };
        nb.receive(a.chats.me.clone(), packet.clone(), now, &mut out);
        nb.receive(a.chats.me.clone(), packet.clone(), now, &mut out);
        nb.forget(&a.chats.me);
        nb.receive(
            a.chats.me.clone(),
            Payload::Geraeteausweis {
                ausweis: na.own.clone(),
            },
            now,
            &mut out,
        );
        out.retain(|(_, p)| !matches!(p, Payload::Geraeteausweis { .. }));
        nb.receive(a.chats.me.clone(), packet, now, &mut out);
        assert_eq!(
            out.len(),
            1,
            "Worker-Retry darf keinen zweiten Hub-Prompt erzeugen"
        );
        nb.receive(
            a.chats.me.clone(),
            Payload::SeminarPacket {
                id: random_id(),
                sent_ms: now - PACKET_TTL_MS - 1,
                payload: Box::new(Payload::SeminarPresence),
            },
            now,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        let mut wrong = na.own.clone();
        wrong.inhalt.person = "Gefälscht".into();
        nb.receive(
            a.chats.me.clone(),
            Payload::Geraeteausweis { ausweis: wrong },
            now,
            &mut out,
        );
        assert!(!nb.verified.contains_key(&a.chats.me));
        nb.receive(
            a.chats.me.clone(),
            Payload::Steuern {
                req: "r".into(),
                text: "Ohne Hülle".into(),
            },
            now,
            &mut out,
        );
        nb.receive(
            a.chats.me.clone(),
            Payload::SeminarPacket {
                id: random_id(),
                sent_ms: now,
                payload: Box::new(Payload::SeminarPresence),
            },
            now,
            &mut out,
        );
        nb.receive(
            b.chats.me.clone(),
            Payload::Geraeteausweis {
                ausweis: na.own.clone(),
            },
            now,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert!(hub.0.lock().unwrap().is_empty());
        let other_issuer = iroh::SecretKey::from_bytes(&[72; 32]);
        nb.receive(
            a.chats.me.clone(),
            Payload::Geraeteausweis {
                ausweis: certificate(&other_issuer, &a.chats.me, Rolle::Seminarleitung, now),
            },
            now,
            &mut out,
        );
        assert!(!nb.verified.contains_key(&a.chats.me));
        let mut wrong_tls = na.own.clone();
        wrong_tls.inhalt.net_pubkey = b.chats.me.to_string();
        nb.receive(
            a.chats.me.clone(),
            Payload::Geraeteausweis { ausweis: wrong_tls },
            now,
            &mut out,
        );
        assert!(!nb.verified.contains_key(&a.chats.me));
        nb.receive(
            a.chats.me.clone(),
            Payload::Geraeteausweis {
                ausweis: na.own.clone(),
            },
            na.own.inhalt.exp * 1000,
            &mut out,
        );
        assert!(!nb.verified.contains_key(&a.chats.me));
        assert!(
            nb.senden(
                &session(),
                "Nie",
                &Von {
                    person: "Aylin".into(),
                    rolle: Rolle::Seminarleitung
                }
            )
            .is_err()
        );
        assert!(
            NodeNet::start(
                Identity::ephemeral(),
                na.own.clone(),
                na.issuer.clone(),
                Box::new(MockHub::default())
            )
            .is_err()
        );
        na.checkout();
        nb.checkout();
    }
}
