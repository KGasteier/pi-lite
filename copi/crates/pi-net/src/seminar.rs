//! Flüchtiges v3-Seminar-Modell; kein Netz, Dateisystem, Modellaufruf oder GUI.
//! ChatStore und RemoteBook bleiben die einzigen Chat-/Freigabeautomaten.
use crate::{
    ausweis::{Ausweis, Bestaetigt, Rolle},
    model::*,
    remote::*,
    store::{ChatStore, Contact, Outbox},
    wire::Payload,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Person {
    pub peer: PeerId,
    /// Nur unbestätigte Anzeige. Bei gültigem Ausweis wird dessen Name verwendet.
    pub name: String,
    pub online: bool,
    /// Lokaler Empfang-/Roster-Adapter; keine Rollenquelle.
    pub slot: Option<String>,
    pub sessions: Vec<SessionRef>,
    pub offline_since_ms: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Von {
    pub person: String,
    pub rolle: Rolle,
}
impl Von {
    pub fn label(&self) -> String {
        format!("{} · {}", self.person, self.rolle.label())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct HubPrompt {
    pub session: SessionRef,
    pub text: String,
    pub von: Von,
}
/// Ingress `from` muss aus NodeEvent::Received (TLS) stammen, nie aus Nutzlast.
pub trait SeminarNet {
    fn send(&mut self, to: &PeerId, payload: Payload) -> Result<(), String>;
    /// Teilnehmer-Client → eigener Hub. Keine Tokens der Leitung verwenden.
    fn senden(&mut self, session: &SessionRef, text: &str, von: &Von) -> Result<(), String>;
    fn poll(&mut self) -> Vec<(PeerId, Payload)> {
        vec![]
    }
}
#[derive(Default)]
pub struct MemoryNet {
    pub sent: Outbox,
    pub hub: Vec<HubPrompt>,
    pub incoming: Outbox,
}
impl SeminarNet for MemoryNet {
    fn send(&mut self, to: &PeerId, payload: Payload) -> Result<(), String> {
        self.sent.push((to.clone(), payload));
        Ok(())
    }
    fn senden(&mut self, session: &SessionRef, text: &str, von: &Von) -> Result<(), String> {
        self.hub.push(HubPrompt {
            session: session.clone(),
            text: text.into(),
            von: von.clone(),
        });
        Ok(())
    }
    fn poll(&mut self) -> Outbox {
        std::mem::take(&mut self.incoming)
    }
}
#[derive(Default)]
pub struct Effects {
    pub out: Outbox,
    pub hub: Vec<HubPrompt>,
}
/// Absichtlich nicht Serialize: Ausweise, DMs und Freigaben bleiben flüchtig.
pub struct SeminarState {
    pub chats: ChatStore,
    pub remote: RemoteBook,
    pub people: Vec<Person>,
    pub sessions: Vec<SessionRef>,
    pub space: SpaceId,
    pub group: Option<ChatId>,
    pub selected: Option<ChatId>,
    pub checked_in: bool,
    issuer: String,
    verified: BTreeMap<PeerId, Bestaetigt>,
}
impl SeminarState {
    pub fn new(me: PeerId, name: &str, issuer: String, space: SpaceId) -> Self {
        Self {
            chats: ChatStore::new(me, name),
            remote: RemoteBook::default(),
            people: vec![],
            sessions: vec![],
            space,
            group: None,
            selected: None,
            checked_in: true,
            issuer,
            verified: BTreeMap::new(),
        }
    }
    pub fn verified(&self, peer: &PeerId, now: u64) -> Option<&Bestaetigt> {
        self.verified.get(peer).filter(|c| c.gueltig(now / 1000))
    }
    pub fn name(&self, peer: &PeerId, now: u64) -> String {
        self.verified(peer, now)
            .map(|c| c.person().to_string())
            .or_else(|| {
                self.people
                    .iter()
                    .find(|p| &p.peer == peer)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_else(|| "Du".into())
    }
    pub fn badge(&self, peer: &PeerId, now: u64) -> &'static str {
        self.verified(peer, now)
            .map(|c| c.rolle().label())
            .unwrap_or("unbestätigt")
    }
    pub fn add_person(&mut self, p: Person) -> Result<(), String> {
        if self.people.len() >= 64 || self.people.iter().any(|x| x.peer == p.peer) {
            return Err("Teilnehmendenliste voll oder doppelt".into());
        }
        self.chats.add_contact(Contact {
            peer: p.peer.clone(),
            name: p.name.clone(),
            addr: String::new(),
            verified: false,
            blocked: false,
            added: 0,
        })?;
        self.people.push(p);
        Ok(())
    }
    pub fn identify(&mut self, peer: &PeerId, c: &Ausweis, now: u64) -> Result<(), String> {
        // Bei Erneuerung niemals eine alte Autorisierung nach Fehler stehen lassen.
        self.verified.remove(peer);
        let c = c.pruefen(&self.issuer, peer, now / 1000)?;
        self.verified.insert(peer.clone(), c);
        Ok(())
    }
    fn member(&self, peer: &PeerId, now: u64) -> Member {
        Member {
            who: Participant::Human { peer: peer.clone() },
            name: self.name(peer, now),
            role: Role::Developer,
        }
    }
    pub fn create_group(&mut self, now: u64) -> Result<Outbox, String> {
        if self.group.is_some() {
            return Err("Seminarchat bereits vorhanden".into());
        }
        // v2-Raumrollen sind NICHT Ausweisrollen; für Chat/DM unprivilegierte
        // Gruppen verwenden, v3-Badges/Control ausschließlich aus verified.
        let members = self
            .people
            .iter()
            .map(|p| self.member(&p.peer, now))
            .collect();
        let (id, out) = self.chats.create_group("Seminarchat", None, members)?;
        self.group = Some(id.clone());
        self.selected = Some(id);
        Ok(out)
    }
    pub fn direct(&mut self, peer: &PeerId, now: u64) -> Result<Outbox, String> {
        if !self.chats.accepts_from(peer) {
            return Err("Person nicht im Seminar".into());
        }
        let (id, out) = self.chats.open_direct(self.member(peer, now))?;
        self.selected = Some(id);
        Ok(out)
    }
    pub fn post(&mut self, text: &str) -> Result<Outbox, String> {
        if !self.checked_in {
            return Err("Du bist ausgecheckt".into());
        }
        let id = self.selected.as_ref().ok_or("Wähle einen Chat")?;
        let (_, out) = self.chats.post(
            id,
            self.chats.me_participant(),
            Body::Text { text: text.into() },
        )?;
        Ok(out)
    }
    fn available(&self, peer: &PeerId) -> bool {
        self.people
            .iter()
            .any(|p| &p.peer == peer && p.online && p.slot.is_some())
    }
    fn may_request(&self, peer: &PeerId, now: u64) -> bool {
        self.checked_in
            && self
                .verified(&self.chats.me, now)
                .is_some_and(|c| c.rolle() == Rolle::Seminarleitung)
            && self.verified(peer, now).is_some()
            && self.available(peer)
    }
    pub fn can_request(&self, peer: &PeerId, now: u64) -> bool {
        self.may_request(peer, now)
    }
    pub fn request(
        &mut self,
        peer: &PeerId,
        session: &SessionRef,
        scope: RemoteScope,
        now: u64,
    ) -> Result<Outbox, String> {
        if !self.may_request(peer, now) {
            return Err("Mitsteuern braucht gültige Ausweise, Seminarleitung und einen belegten Arbeitsplatz".into());
        }
        if !self
            .people
            .iter()
            .any(|p| &p.peer == peer && p.sessions.contains(session))
        {
            return Err("Session nicht verfügbar".into());
        }
        // v2 reason trägt den gewünschten Sitzungstitel. Host wählt ausschließlich
        // aus eigenen Sessions; kein fremder cwd wird als Ausführungsziel benutzt.
        let p = self.remote.request(
            peer.clone(),
            self.space.clone(),
            scope,
            scope.max_minutes(),
            &session.label,
            true,
        )?;
        Ok(vec![(peer.clone(), p)])
    }
    pub fn accept(
        &mut self,
        req: &str,
        scope: RemoteScope,
        session: &SessionRef,
        now: u64,
    ) -> Result<Outbox, String> {
        let g = self.remote.get(req).ok_or("Anfrage nicht vorhanden")?;
        if !self.checked_in
            || self.verified(&self.chats.me, now).is_none()
            || !self.available(&g.peer)
            || !self
                .verified(&g.peer, now)
                .is_some_and(|c| c.rolle() == Rolle::Seminarleitung)
        {
            return Err("Ausweis oder Verbindung nicht mehr gültig".into());
        }
        if !self.sessions.contains(session) || session.label != g.reason {
            return Err("Nur die angefragte eigene Session kann freigegeben werden".into());
        }
        let peer = g.peer.clone();
        let minutes = g.minutes.min(scope.max_minutes());
        let p = self
            .remote
            .accept(req, scope, minutes, session.clone(), now)?;
        Ok(vec![(peer, p)])
    }
    pub fn end(&mut self, req: &str, reason: &str) -> Outbox {
        let peer = self.remote.get(req).map(|g| g.peer.clone());
        let p = self.remote.end(req, reason);
        // Ablehnung speichert keine Freigabe und keinen Personen-/Anfrageverlauf.
        self.remote.grants.retain(|g| g.req != req);
        peer.zip(p).into_iter().collect()
    }
    pub fn status(&mut self, peer: &PeerId, online: bool, now: u64) {
        if let Some(p) = self.people.iter_mut().find(|p| &p.peer == peer) {
            p.online = online;
            if online {
                p.offline_since_ms = None;
            } else if p.offline_since_ms.is_none() {
                p.offline_since_ms = Some(now);
            }
        }
    }
    pub fn tick(&mut self, now: u64) -> Outbox {
        let mut out = self.remote.expire(now);
        let end: Vec<_> = self
            .remote
            .grants
            .iter()
            .filter(|g| {
                !matches!(g.state, GrantState::Ended { .. })
                    && (!self.checked_in
                        || self.verified(&self.chats.me, now).is_none()
                        || self.verified(&g.peer, now).is_none()
                        || self
                            .people
                            .iter()
                            .find(|p| p.peer == g.peer)
                            .is_none_or(|p| {
                                p.slot.is_none()
                                    || p.offline_since_ms
                                        .is_some_and(|t| now.saturating_sub(t) > 60_000)
                            }))
            })
            .map(|g| g.req.clone())
            .collect();
        for req in end {
            out.extend(self.end(&req, "Freigabe beendet: Check-out, Ausweis oder Verbindung"));
        }
        self.remote
            .grants
            .retain(|g| !matches!(g.state, GrantState::Ended { .. }));
        out
    }
    pub fn checkout(&mut self) -> Outbox {
        self.checked_in = false;
        let ids: Vec<_> = self.remote.grants.iter().map(|g| g.req.clone()).collect();
        let mut out = vec![];
        for req in ids {
            out.extend(self.end(&req, "Check-out"));
        }
        self.verified.clear();
        self.chats.chats.clear();
        self.chats.invites.clear();
        self.chats.contacts.clear();
        self.people.clear();
        self.sessions.clear();
        self.group = None;
        self.selected = None;
        self.chats.my_name.clear();
        out
    }
    pub fn control(&mut self, req: &str, text: &str, now: u64) -> Result<Outbox, String> {
        let g = self.remote.get(req).ok_or("Keine Freigabe")?;
        if !self.may_request(&g.peer, now)
            || g.side != Side::Viewer
            || g.scope != RemoteScope::Control
            || !matches!(g.state,GrantState::Active{expires_ms} if expires_ms>now)
        {
            return Err("Keine aktive Mitsteuerung".into());
        }
        if text.trim().is_empty() || text.len() > 4000 {
            return Err("Bitte 1 bis 4000 Zeichen eingeben".into());
        }
        Ok(vec![(
            g.peer.clone(),
            Payload::Steuern {
                req: req.into(),
                text: text.into(),
            },
        )])
    }
    /// Explizit freigegebene eigene Session spiegeln; Runtime darf ausschließlich
    /// deren Verlauf liefern. Kein automatisches Lesen von Dateien/anderen Chats.
    pub fn mirror(
        &mut self,
        req: &str,
        session: &SessionRef,
        frame: RemoteFrame,
        now: u64,
    ) -> Result<Outbox, String> {
        let g = self.remote.get(req).ok_or("Keine Freigabe")?;
        if g.side != Side::Host
            || g.session.as_ref() != Some(session)
            || !self.checked_in
            || self.verified(&self.chats.me, now).is_none()
            || !self
                .verified(&g.peer, now)
                .is_some_and(|c| c.rolle() == Rolle::Seminarleitung)
            || !self.available(&g.peer)
            || !matches!(g.state,GrantState::Active{expires_ms} if expires_ms>now)
        {
            return Err("Keine aktive Freigabe für diese Session".into());
        }
        if frame.items.len() > MAX_FRAME_ITEMS {
            return Err("Spiegelpaket zu groß".into());
        }
        let items = frame
            .items
            .into_iter()
            .map(|item| match item {
                MirrorItem::User { text } => MirrorItem::User {
                    text: redact(&text),
                },
                MirrorItem::Assistant { text, done } => MirrorItem::Assistant {
                    text: redact(&text),
                    done,
                },
                MirrorItem::Note { text } => MirrorItem::Note {
                    text: redact(&text),
                },
                MirrorItem::Tool {
                    name,
                    detail,
                    state,
                } => MirrorItem::Tool {
                    name: redact(&name),
                    detail: redact(&detail),
                    state: redact(&state),
                },
            })
            .collect();
        let p = Payload::RemoteFrame {
            req: req.into(),
            frame: RemoteFrame {
                items,
                state: redact(&frame.state),
                ..frame
            },
        };
        crate::wire::Envelope::new(p.clone()).encode()?;
        Ok(vec![(g.peer.clone(), p)])
    }
    pub fn receive(&mut self, from: &PeerId, p: Payload, now: u64) -> Result<Effects, String> {
        if !self.checked_in || !self.chats.accepts_from(from) {
            return Err("Person nicht im Seminar".into());
        }
        let mut e = Effects::default();
        match p {
            Payload::Geraeteausweis { ausweis } => {
                self.identify(from, &ausweis, now)?;
            }
            Payload::RemoteRequest { ref space, .. } if space != &self.space => {
                return Err("Fremdes Seminar".into())
            }
            Payload::RemoteRequest { .. }
            | Payload::RemoteGrant { .. }
            | Payload::RemoteEnd { .. }
            | Payload::RemoteFrame { .. }
            | Payload::RemoteInput { .. }
            | Payload::Steuern { .. } => {
                if !matches!(p, Payload::RemoteEnd { .. })
                    && (self.verified(&self.chats.me, now).is_none()
                        || self.verified(from, now).is_none()
                        || !self.available(from))
                {
                    return Err("Keine bestätigte, verbundene Person".into());
                }
                let p = match p {
                    Payload::Steuern { req, text } => Payload::RemoteInput { req, text },
                    p => p,
                };
                let lead = self
                    .verified(from, now)
                    .is_some_and(|c| c.rolle() == Rolle::Seminarleitung);
                // Auch nach einer Erneuerung/Downgrade darf ein früherer Grant
                // niemals neue Eingaben durch eine Nicht-Leitung autorisieren.
                if matches!(p, Payload::RemoteInput { .. }) && !lead {
                    return Err("Nur die Seminarleitung darf mitsteuern".into());
                }
                if let Some(RemoteEffect::Inject { req, text }) =
                    self.remote
                        .apply(from, &p, now, &|_, space| lead && space == &self.space)?
                {
                    let g = self.remote.get(&req).ok_or("Freigabe beendet")?;
                    let session = g.session.clone().ok_or("Session fehlt")?;
                    if !self.sessions.contains(&session) {
                        return Err("Session nicht mehr verfügbar".into());
                    }
                    let c = self.verified(from, now).ok_or("Ausweis abgelaufen")?;
                    e.hub.push(HubPrompt {
                        session,
                        text,
                        von: Von {
                            person: c.person().into(),
                            rolle: c.rolle(),
                        },
                    });
                }
            }
            p => {
                let (_, out) = self.chats.apply(from, p)?;
                e.out = out;
            }
        }
        Ok(e)
    }
}

/// Ausschließlich Screenshot-/Testdaten: eigener öffentlicher Demo-Empfang,
/// niemals als produktiven Vertrauensanker verwenden. Kein I/O und kein Netz.
pub fn demo_pair(now: u64) -> (SeminarState, SeminarState) {
    use crate::ausweis::{base64url, kanonisch, Inhalt};
    let issuer = iroh::SecretKey::from_bytes(&[101; 32]);
    let peers =
        [102, 103].map(|b| PeerId(iroh::SecretKey::from_bytes(&[b; 32]).public().to_string()));
    let certs: Vec<_> = peers
        .iter()
        .zip([
            ("Aylin", Rolle::Seminarleitung),
            ("Mira", Rolle::Teilnehmende),
        ])
        .map(|(peer, (name, rolle))| {
            let inhalt = Inhalt {
                v: 1,
                typ: "geraeteausweis".into(),
                pid: format!("demo-{name}"),
                person: name.into(),
                rolle,
                net_pubkey: peer.to_string(),
                geraet_pubkey: base64url(&[104; 32]),
                iat: (now / 1000).saturating_sub(60),
                exp: now / 1000 + 3600,
            };
            let signatur = base64url(
                &issuer
                    .sign(&kanonisch(&inhalt).expect("Demo JSON"))
                    .to_bytes(),
            );
            Ausweis { inhalt, signatur }
        })
        .collect();
    let mut states: Vec<_> = peers
        .iter()
        .zip(["Aylin", "Mira"])
        .map(|(peer, name)| {
            SeminarState::new(
                peer.clone(),
                name,
                base64url(issuer.public().as_bytes()),
                "demo-seminar".into(),
            )
        })
        .collect();
    for (idx, s) in states.iter_mut().enumerate() {
        s.sessions = vec![
            SessionRef {
                label: "Seminar-Handout".into(),
                cwd: "demo-session-1".into(),
            },
            SessionRef {
                label: "Plakat-Entwurf".into(),
                cwd: "demo-session-2".into(),
            },
        ];
        s.identify(&peers[idx], &certs[idx], now)
            .expect("Demoausweis");
        let other = 1 - idx;
        s.add_person(Person {
            peer: peers[other].clone(),
            name: certs[other].inhalt.person.clone(),
            online: true,
            slot: Some(format!("account{:02}", other + 1)),
            sessions: s.sessions.clone(),
            offline_since_ms: None,
        })
        .expect("Demoperson");
        s.identify(&peers[other], &certs[other], now)
            .expect("Demoausweis");
        s.add_person(Person {
            peer: PeerId(iroh::SecretKey::from_bytes(&[105; 32]).public().to_string()),
            name: "Jonas".into(),
            online: false,
            slot: None,
            sessions: vec![],
            offline_since_ms: Some(now),
        })
        .expect("Demoperson");
    }
    let b = states.pop().expect("Demo Mira");
    let a = states.pop().expect("Demo Aylin");
    (a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ausweis::{base64url, kanonisch, Inhalt};
    const NOW: u64 = 1_000_000;
    fn session() -> SessionRef {
        SessionRef {
            label: "Seminar-Handout".into(),
            cwd: "session-1".into(),
        }
    }
    fn pair() -> (SeminarState, SeminarState, Ausweis, Ausweis) {
        let issuer = iroh::SecretKey::from_bytes(&[1; 32]);
        let a = iroh::SecretKey::from_bytes(&[2; 32]).public().to_string();
        let b = iroh::SecretKey::from_bytes(&[3; 32]).public().to_string();
        let cert = |peer: String, name: &str, rolle| {
            let inhalt = Inhalt {
                v: 1,
                typ: "geraeteausweis".into(),
                pid: peer.clone(),
                person: name.into(),
                rolle,
                net_pubkey: peer,
                geraet_pubkey: base64url(&[4; 32]),
                iat: 900,
                exp: 43_200,
            };
            let signatur = base64url(&issuer.sign(&kanonisch(&inhalt).unwrap()).to_bytes());
            Ausweis { inhalt, signatur }
        };
        let ca = cert(a.clone(), "Aylin", Rolle::Seminarleitung);
        let cb = cert(b.clone(), "Mira", Rolle::Teilnehmende);
        let pk = base64url(issuer.public().as_bytes());
        let mut a = SeminarState::new(PeerId(a), "Aylin", pk.clone(), "seminar".into());
        let mut b = SeminarState::new(PeerId(b), "Mira", pk, "seminar".into());
        for (s, c) in [(&mut a, &ca), (&mut b, &cb)] {
            s.identify(&s.chats.me.clone(), c, NOW).unwrap();
            s.sessions = vec![session()];
        }
        for (s, p, c, name) in [
            (&mut a, b.chats.me.clone(), &cb, "Mira"),
            (&mut b, ca.inhalt.net_pubkey.as_str().into(), &ca, "Aylin"),
        ] {
            s.add_person(Person {
                peer: p.clone(),
                name: name.into(),
                online: true,
                slot: Some("account01".into()),
                sessions: vec![session()],
                offline_since_ms: None,
            })
            .unwrap();
            s.identify(&p, c, NOW).unwrap();
        }
        (a, b, ca, cb)
    }
    fn ask(a: &mut SeminarState, b: &mut SeminarState) -> String {
        let out = a
            .request(&b.chats.me, &session(), RemoteScope::Control, NOW)
            .unwrap();
        let p = out.into_iter().next().unwrap().1;
        let Payload::RemoteRequest { ref req, .. } = p else {
            panic!()
        };
        let req = req.clone();
        b.receive(&a.chats.me, p, NOW).unwrap();
        req
    }
    fn active(a: &mut SeminarState, b: &mut SeminarState) -> String {
        let req = ask(a, b);
        let out = b
            .accept(&req, RemoteScope::Control, &session(), NOW)
            .unwrap();
        a.receive(&b.chats.me, out.into_iter().next().unwrap().1, NOW)
            .unwrap();
        req
    }
    #[test]
    fn no_certificate_no_control() {
        let (mut a, mut b, _, _) = pair();
        a.verified.remove(&b.chats.me);
        assert!(a
            .request(&b.chats.me, &session(), RemoteScope::Control, NOW)
            .is_err());
        b.verified.remove(&a.chats.me);
        assert!(b
            .receive(
                &a.chats.me,
                Payload::RemoteRequest {
                    req: "r".into(),
                    space: "seminar".into(),
                    scope: RemoteScope::Control,
                    minutes: 30,
                    reason: session().label
                },
                NOW
            )
            .is_err());
        assert!(a.remote.grants.is_empty());
        assert!(b.remote.grants.is_empty());
    }
    #[test]
    fn participant_cannot_request_control_or_spoof_role() {
        let (mut a, mut b, _, _) = pair();
        assert!(b
            .request(&a.chats.me, &session(), RemoteScope::Control, NOW)
            .is_err());
        assert!(a
            .receive(
                &b.chats.me,
                Payload::RemoteRequest {
                    req: "r".into(),
                    space: "seminar".into(),
                    scope: RemoteScope::Control,
                    minutes: 30,
                    reason: session().label
                },
                NOW
            )
            .is_err());
        assert!(a.remote.grants.is_empty());
    }
    #[test]
    fn rejection_persists_nothing() {
        let (mut a, mut b, _, _) = pair();
        let req = ask(&mut a, &mut b);
        let before = serde_json::to_string(&b.chats).unwrap();
        let out = b.end(&req, "Abgelehnt");
        assert_eq!(out.len(), 1);
        assert!(b.remote.grants.is_empty());
        assert_eq!(before, serde_json::to_string(&b.chats).unwrap());
        assert!(b
            .receive(
                &a.chats.me,
                Payload::Steuern {
                    req,
                    text: "Nicht senden".into()
                },
                NOW
            )
            .is_err());
    }
    #[test]
    fn expiry_stops_control() {
        let (mut a, mut b, _, _) = pair();
        let req = active(&mut a, &mut b);
        let expired = NOW + 30 * 60_000;
        assert!(b
            .receive(
                &a.chats.me,
                Payload::Steuern {
                    req: req.clone(),
                    text: "Nicht senden".into()
                },
                expired
            )
            .is_err());
        assert_eq!(b.tick(expired).len(), 1);
        assert!(b.remote.grants.is_empty());
        assert!(a.control(&req, "Text", expired).is_err());
    }
    #[test]
    fn leader_prompt_visibly_marked_and_no_requester_attribution() {
        let (mut a, mut b, _, _) = pair();
        let req = active(&mut a, &mut b);
        let (_, p) = a.control(&req, "Bitte kürzen", NOW + 1).unwrap().remove(0);
        let effects = b.receive(&a.chats.me, p, NOW + 1).unwrap();
        assert_eq!(effects.hub.len(), 1);
        let h = &effects.hub[0];
        assert_eq!(h.von.label(), "Aylin · Seminarleitung");
        assert_eq!(h.session, session());
        let mut net = MemoryNet::default();
        net.senden(&h.session, &h.text, &h.von).unwrap();
        assert_eq!(net.hub[0], *h);
    }
    #[test]
    fn view_only_stop_disconnect_checkout_and_session_binding() {
        let (mut a, mut b, _, _) = pair();
        let req = ask(&mut a, &mut b);
        assert!(b
            .accept(
                &req,
                RemoteScope::Control,
                &SessionRef {
                    label: "Andere".into(),
                    cwd: "session-2".into()
                },
                NOW
            )
            .is_err());
        let p = b
            .accept(&req, RemoteScope::View, &session(), NOW)
            .unwrap()
            .remove(0)
            .1;
        a.receive(&b.chats.me, p, NOW).unwrap();
        assert!(a.control(&req, "Text", NOW).is_err());
        assert!(b
            .receive(
                &a.chats.me,
                Payload::Steuern {
                    req: req.clone(),
                    text: "Text".into()
                },
                NOW
            )
            .is_err());
        b.end(&req, "Beendet");
        a.end(&req, "Zurückgegeben");
        assert!(b.remote.grants.is_empty());
        let req = active(&mut a, &mut b);
        b.status(&a.chats.me, false, NOW);
        assert!(b.tick(NOW + 60_000).is_empty());
        assert_eq!(b.tick(NOW + 60_001).len(), 1);
        assert!(b.remote.get(&req).is_none());
        a.end(&req, "Verbindung");
        b.status(&a.chats.me, true, NOW + 60_002);
        active(&mut a, &mut b);
        assert_eq!(b.checkout().len(), 1);
        assert!(b.remote.grants.is_empty());
        assert!(b.people.is_empty());
        assert!(b.verified.is_empty());
    }
    #[test]
    fn mirror_only_selected_session_and_redacts_secrets() {
        let (mut a, mut b, _, _) = pair();
        let req = active(&mut a, &mut b);
        let frame = RemoteFrame {
            seq: 1,
            reset: true,
            items: vec![MirrorItem::User {
                text: "token=secret normal".into(),
            }],
            state: "ruht".into(),
        };
        let wrong = SessionRef {
            label: "Nicht freigegeben".into(),
            cwd: "session-2".into(),
        };
        assert!(b.mirror(&req, &wrong, frame.clone(), NOW).is_err());
        let (_, p) = b.mirror(&req, &session(), frame, NOW).unwrap().remove(0);
        a.receive(&b.chats.me, p, NOW).unwrap();
        assert_eq!(
            a.remote.get(&req).unwrap().mirror,
            vec![MirrorItem::User {
                text: "token=[GESCHWÄRZT] normal".into()
            }]
        );
        b.end(&req, "Beendet");
        assert!(b
            .mirror(
                &req,
                &session(),
                RemoteFrame {
                    seq: 2,
                    reset: false,
                    items: vec![],
                    state: "ruht".into()
                },
                NOW
            )
            .is_err());
    }
    #[test]
    fn chats_and_dms_reuse_v2_and_mark_unverified() {
        let (mut a, mut b, _, _) = pair();
        b.verified.remove(&a.chats.me);
        let out = a.create_group(NOW).unwrap();
        b.receive(&a.chats.me, out[0].1.clone(), NOW).unwrap();
        let id = b.chats.invites[0].chat.id.clone();
        b.chats.accept_invite(&id).unwrap();
        a.post("Hallo Seminar")
            .unwrap()
            .into_iter()
            .for_each(|(_, p)| {
                b.receive(&a.chats.me, p, NOW).unwrap();
            });
        assert_eq!(
            b.chats
                .chat(&id)
                .unwrap()
                .messages
                .last()
                .unwrap()
                .body
                .text(),
            "Hallo Seminar"
        );
        assert_eq!(b.badge(&a.chats.me, NOW), "unbestätigt");
        let group = a.selected.clone();
        a.direct(&b.chats.me, NOW).unwrap();
        assert_ne!(group, a.selected);
        assert_eq!(
            a.chats.chat(a.selected.as_ref().unwrap()).unwrap().kind,
            ChatKind::Direct
        );
    }
}
