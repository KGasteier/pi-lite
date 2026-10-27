//! Fernzugriff nach ausdrücklicher Einwilligung; reine Zustandsmaschine ohne I/O.
//! Die Gegenstelle stammt immer aus TLS, Rollen aus dem lokalen ChatStore.
use crate::{
    a2a::sanitize,
    model::{PeerId, SpaceId, random_id},
    wire::Payload,
};
use serde::{Deserialize, Serialize};

pub const MAX_VIEW_MINUTES: u32 = 120;
pub const MAX_CONTROL_MINUTES: u32 = 30;
pub const MAX_GRANTS: usize = 4;
pub const MAX_FRAME_ITEMS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteScope {
    View,
    Suggest,
    Control,
}
impl RemoteScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::View => "Nur ansehen",
            Self::Suggest => "Ansehen + Vorschläge",
            Self::Control => "Mitsteuern",
        }
    }
    pub fn max_minutes(self) -> u32 {
        if self == Self::Control {
            MAX_CONTROL_MINUTES
        } else {
            MAX_VIEW_MINUTES
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRef {
    pub label: String,
    pub cwd: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum MirrorItem {
    User {
        text: String,
    },
    Assistant {
        text: String,
        done: bool,
    },
    Tool {
        name: String,
        detail: String,
        state: String,
    },
    Note {
        text: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFrame {
    pub seq: u64,
    pub reset: bool,
    pub items: Vec<MirrorItem>,
    pub state: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Viewer,
    Host,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "s", rename_all = "snake_case")]
pub enum GrantState {
    Requested,
    Active { expires_ms: u64 },
    Ended { reason: String },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grant {
    pub req: String,
    pub side: Side,
    pub peer: PeerId,
    pub space: SpaceId,
    pub scope: RemoteScope,
    pub minutes: u32,
    pub reason: String,
    pub state: GrantState,
    pub session: Option<SessionRef>,
    pub mirror: Vec<MirrorItem>,
    pub mirror_state: String,
    pub last_seq: u64,
    pub suggestions: Vec<String>,
}
/// Freigaben sind flüchtig; die App darf sie nicht dauerhaft speichern.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct RemoteBook {
    pub grants: Vec<Grant>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum RemoteEffect {
    Asked { req: String },
    Granted { req: String },
    Ended { req: String, reason: String },
    Mirrored { req: String },
    Suggested { req: String },
    Inject { req: String, text: String },
}

impl RemoteBook {
    pub fn request(
        &mut self,
        peer: PeerId,
        space: SpaceId,
        scope: RemoteScope,
        minutes: u32,
        reason: &str,
        requester_is_lead: bool,
    ) -> Result<Payload, String> {
        if !requester_is_lead {
            return Err("Nur die Seminarleitung darf Fernzugriff anfragen".into());
        }
        valid_minutes(scope, minutes)?;
        self.check_relationship(&peer, &space, Side::Viewer)?;
        let req = random_id();
        let reason = sanitize(reason, 300);
        self.grants.push(new_grant(
            req.clone(),
            Side::Viewer,
            peer,
            space.clone(),
            scope,
            minutes,
            reason.clone(),
        ));
        self.prune();
        Ok(Payload::RemoteRequest {
            req,
            space,
            scope,
            minutes,
            reason,
        })
    }
    pub fn accept(
        &mut self,
        req: &str,
        scope: RemoteScope,
        minutes: u32,
        session: SessionRef,
        now_ms: u64,
    ) -> Result<Payload, String> {
        valid_minutes(scope, minutes)?;
        let g = self
            .grants
            .iter_mut()
            .find(|g| g.req == req)
            .ok_or("Unbekannte Anfrage")?;
        if g.side != Side::Host || g.state != GrantState::Requested {
            return Err("Keine offene Host-Anfrage".into());
        }
        if scope > g.scope || minutes > g.minutes {
            return Err("Umfang oder Dauer überschreiten Anfrage".into());
        }
        let expires_ms = now_ms.saturating_add(u64::from(minutes) * 60_000);
        let session = clean_session(&session);
        g.scope = scope;
        g.minutes = minutes;
        g.session = Some(session.clone());
        g.state = GrantState::Active { expires_ms };
        Ok(Payload::RemoteGrant {
            req: req.into(),
            scope,
            expires_ms,
            session,
        })
    }
    pub fn end(&mut self, req: &str, reason: &str) -> Option<Payload> {
        let g = self.grants.iter_mut().find(|g| g.req == req)?;
        if matches!(g.state, GrantState::Ended { .. }) {
            return None;
        }
        let reason = sanitize(reason, 300);
        g.state = GrantState::Ended {
            reason: reason.clone(),
        };
        g.suggestions.clear();
        self.prune();
        Some(Payload::RemoteEnd {
            req: req.into(),
            reason,
        })
    }
    pub fn expire(&mut self, now_ms: u64) -> Vec<(PeerId, Payload)> {
        let expired: Vec<_> = self
            .grants
            .iter()
            .filter(
                |g| matches!(g.state, GrantState::Active { expires_ms } if expires_ms <= now_ms),
            )
            .map(|g| (g.peer.clone(), g.req.clone()))
            .collect();
        let mut out = Vec::new();
        for (peer, req) in expired {
            if let Some(p) = self.end(&req, "Freigabe abgelaufen") {
                out.push((peer, p));
            }
        }
        self.prune();
        out
    }
    pub fn apply(
        &mut self,
        from: &PeerId,
        p: &Payload,
        now_ms: u64,
        lead_of: &dyn Fn(&PeerId, &SpaceId) -> bool,
    ) -> Result<Option<RemoteEffect>, String> {
        match p {
            Payload::RemoteRequest {
                req,
                space,
                scope,
                minutes,
                reason,
            } => {
                if self.get(req).is_some() {
                    return Ok(None);
                }
                if req.is_empty() || req.len() > 128 {
                    return Err("Ungültige Anfragekennung".into());
                }
                if !lead_of(from, space) {
                    return Err("Anfrage nicht von der Seminarleitung".into());
                }
                valid_minutes(*scope, *minutes)?;
                self.check_relationship(from, space, Side::Host)?;
                if self
                    .grants
                    .iter()
                    .filter(|g| {
                        g.side == Side::Host && !matches!(g.state, GrantState::Ended { .. })
                    })
                    .count()
                    >= MAX_GRANTS
                {
                    return Err("Zu viele offene Freigaben".into());
                }
                self.grants.push(new_grant(
                    req.clone(),
                    Side::Host,
                    from.clone(),
                    space.clone(),
                    *scope,
                    *minutes,
                    sanitize(reason, 300),
                ));
                self.prune();
                Ok(Some(RemoteEffect::Asked { req: req.clone() }))
            }
            Payload::RemoteGrant {
                req,
                scope,
                expires_ms,
                session,
            } => {
                let g = self.grant_from(req, from)?;
                if g.side != Side::Viewer || g.state != GrantState::Requested {
                    return Err("Keine eigene offene Anfrage".into());
                }
                if *scope > g.scope {
                    return Err("Umfang überschreitet Anfrage".into());
                }
                let expires_ms =
                    (*expires_ms)
                        .min(now_ms.saturating_add(
                            u64::from(g.minutes.min(scope.max_minutes())) * 60_000,
                        ));
                if expires_ms <= now_ms {
                    return Err("Freigabe bereits abgelaufen".into());
                }
                g.scope = *scope;
                g.session = Some(clean_session(session));
                g.state = GrantState::Active { expires_ms };
                Ok(Some(RemoteEffect::Granted { req: req.clone() }))
            }
            Payload::RemoteEnd { req, reason } => {
                let g = self.grant_from(req, from)?;
                if matches!(g.state, GrantState::Ended { .. }) {
                    return Ok(None);
                }
                let reason = sanitize(reason, 300);
                g.state = GrantState::Ended {
                    reason: reason.clone(),
                };
                g.suggestions.clear();
                self.prune();
                Ok(Some(RemoteEffect::Ended {
                    req: req.clone(),
                    reason,
                }))
            }
            Payload::RemoteFrame { req, frame } => {
                let g = self.grant_from(req, from)?;
                if g.side != Side::Viewer || !active(g, now_ms) {
                    return Err("Keine aktive Ansicht".into());
                }
                if frame.seq <= g.last_seq {
                    return Err("Veralteter Spiegelstand".into());
                }
                if frame.items.len() > MAX_FRAME_ITEMS {
                    return Err("Spiegelpaket zu groß".into());
                }
                if frame.reset {
                    g.mirror.clear();
                }
                g.mirror.extend(frame.items.iter().cloned());
                if g.mirror.len() > 500 {
                    g.mirror.drain(..g.mirror.len() - 500);
                }
                g.last_seq = frame.seq;
                g.mirror_state = sanitize(&frame.state, 300);
                Ok(Some(RemoteEffect::Mirrored { req: req.clone() }))
            }
            Payload::RemoteInput { req, text } => {
                let g = self.grant_from(req, from)?;
                if g.side != Side::Host || !active(g, now_ms) {
                    return Err("Keine aktive Freigabe".into());
                }
                let text = sanitize(text, 4000);
                match g.scope {
                    RemoteScope::View => Err("Nur Ansicht: Eingabe nicht erlaubt".into()),
                    RemoteScope::Suggest => {
                        if g.suggestions.len() >= 20 {
                            return Err("Zu viele offene Vorschläge".into());
                        }
                        g.suggestions.push(text);
                        Ok(Some(RemoteEffect::Suggested { req: req.clone() }))
                    }
                    RemoteScope::Control => Ok(Some(RemoteEffect::Inject {
                        req: req.clone(),
                        text,
                    })),
                }
            }
            _ => Err("Keine Fernzugriffsnachricht".into()),
        }
    }
    pub fn active_hosts(&self, now_ms: u64) -> Vec<&Grant> {
        self.grants
            .iter()
            .filter(|g| g.side == Side::Host && active(g, now_ms))
            .collect()
    }
    pub fn get(&self, req: &str) -> Option<&Grant> {
        self.grants.iter().find(|g| g.req == req)
    }
    fn grant_from(&mut self, req: &str, from: &PeerId) -> Result<&mut Grant, String> {
        let g = self
            .grants
            .iter_mut()
            .find(|g| g.req == req)
            .ok_or("Unbekannte Anfrage")?;
        if &g.peer != from {
            return Err("Falsche Gegenstelle".into());
        }
        Ok(g)
    }
    fn check_relationship(&self, peer: &PeerId, space: &SpaceId, side: Side) -> Result<(), String> {
        if self.grants.iter().any(|g| {
            &g.peer == peer
                && &g.space == space
                && g.side == side
                && !matches!(g.state, GrantState::Ended { .. })
        }) {
            return Err("Für diesen Raum besteht bereits eine Beziehung".into());
        }
        Ok(())
    }
    fn prune(&mut self) {
        // Der Vertrag hat keinen Ende-Zeitstempel: alternativ maximal 50 beendete
        // Beziehungen behalten. Offene Beziehungen werden niemals verdrängt.
        let mut remove = self
            .grants
            .iter()
            .filter(|g| matches!(g.state, GrantState::Ended { .. }))
            .count()
            .saturating_sub(50);
        self.grants.retain(|g| {
            if remove > 0 && matches!(g.state, GrantState::Ended { .. }) {
                remove -= 1;
                false
            } else {
                true
            }
        });
    }
}
fn active(g: &Grant, now: u64) -> bool {
    matches!(g.state, GrantState::Active { expires_ms } if expires_ms > now)
}
fn valid_minutes(scope: RemoteScope, minutes: u32) -> Result<(), String> {
    if minutes == 0 || minutes > scope.max_minutes() {
        Err("Ungültige Freigabedauer".into())
    } else {
        Ok(())
    }
}
fn clean_session(s: &SessionRef) -> SessionRef {
    SessionRef {
        label: sanitize(&s.label, 300),
        cwd: sanitize(&s.cwd, 1000),
    }
}
fn new_grant(
    req: String,
    side: Side,
    peer: PeerId,
    space: SpaceId,
    scope: RemoteScope,
    minutes: u32,
    reason: String,
) -> Grant {
    Grant {
        req,
        side,
        peer,
        space,
        scope,
        minutes,
        reason,
        state: GrantState::Requested,
        session: None,
        mirror: Vec::new(),
        mirror_state: String::new(),
        last_seq: 0,
        suggestions: Vec::new(),
    }
}

/// Typische Geheimnisse vor dem Spiegeln schwärzen. Keine vollständige Geheimniserkennung:
/// die App muss weiterhin ausschließlich die ausdrücklich gewählte Session verdichten.
pub fn redact(text: &str) -> String {
    let mut pem = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("-----BEGIN ") {
        pem.push_str(&rest[..start]);
        let block = &rest[start..];
        if let Some(header_end) = block
            .find("-----\n")
            .or_else(|| block[11..].find("-----").map(|i| i + 11))
        {
            if block[..header_end].contains("PRIVATE KEY") {
                pem.push_str("[GESCHWÄRZT]");
                if let Some(end) = block
                    .find("-----END ")
                    .and_then(|i| block[i + 9..].find("-----").map(|j| i + 9 + j + 5))
                {
                    rest = &block[end..];
                    continue;
                }
                rest = "";
                break;
            }
        }
        pem.push_str("-----BEGIN ");
        rest = &block[11..];
    }
    pem.push_str(rest);
    let mut out = String::new();
    let mut i = 0;
    while i < pem.len() {
        let tail = &pem[i..];
        let lower = tail
            .chars()
            .take(10)
            .collect::<String>()
            .to_ascii_lowercase();
        let value_prefix = ["password=", "token="]
            .into_iter()
            .find(|p| lower.starts_with(p));
        if let Some(prefix) = value_prefix {
            out.push_str(&tail[..prefix.len()]);
            let start = i + prefix.len();
            let quote = pem[start..]
                .chars()
                .next()
                .filter(|c| *c == '\'' || *c == '"');
            let skip = quote.map_or(0, char::len_utf8);
            let end = pem[start + skip..]
                .find(|c: char| {
                    if let Some(q) = quote {
                        c == q || c == '\n'
                    } else {
                        c.is_whitespace() || matches!(c, '&' | ';' | ',' | '"' | '\'')
                    }
                })
                .map_or(pem.len(), |n| start + skip + n);
            out.push_str("[GESCHWÄRZT]");
            i = end
                + if quote.is_some() && pem[end..].starts_with(quote.unwrap_or('"')) {
                    1
                } else {
                    0
                };
            continue;
        }
        let bearer = lower.starts_with("bearer ") || lower.starts_with("bearer\t");
        let secret = ["sk-", "ghp_", "github_pat_", "xox", "AKIA"]
            .iter()
            .any(|p| tail.starts_with(p));
        if bearer || secret {
            let offset = if bearer {
                6 + tail[6..]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .map(char::len_utf8)
                    .sum::<usize>()
            } else {
                0
            };
            let end = tail[offset..]
                .find(|c: char| {
                    c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | ')' | ']' | '}')
                })
                .map_or(tail.len(), |n| n + offset);
            if bearer {
                out.push_str(&tail[..offset]);
            }
            out.push_str("[GESCHWÄRZT]");
            i += end.max(1);
            continue;
        }
        if let Some(c) = tail.chars().next() {
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> SessionRef {
        SessionRef {
            label: "Test".into(),
            cwd: String::new(),
        }
    }
    fn request(req: &str, scope: RemoteScope, minutes: u32) -> Payload {
        Payload::RemoteRequest {
            req: req.into(),
            space: "seminar".into(),
            scope,
            minutes,
            reason: "Hilfe\0\u{202e}".into(),
        }
    }
    fn host(scope: RemoteScope) -> RemoteBook {
        let mut b = RemoteBook::default();
        b.apply(&"lead".into(), &request("r", scope, 2), 100, &|_, _| true)
            .unwrap();
        b.accept("r", scope, 1, session(), 100).unwrap();
        b
    }
    fn viewer(scope: RemoteScope) -> (RemoteBook, String) {
        let mut b = RemoteBook::default();
        let Payload::RemoteRequest { req, .. } = b
            .request("host".into(), "seminar".into(), scope, 2, "Hilfe", true)
            .unwrap()
        else {
            panic!()
        };
        (b, req)
    }
    fn activate(
        b: &mut RemoteBook,
        req: &str,
        scope: RemoteScope,
        expires_ms: u64,
    ) -> Result<Option<RemoteEffect>, String> {
        b.apply(
            &"host".into(),
            &Payload::RemoteGrant {
                req: req.into(),
                scope,
                expires_ms,
                session: session(),
            },
            100,
            &|_, _| false,
        )
    }
    #[test]
    fn request_requires_lead_duration_and_unique_relationship() {
        let mut b = RemoteBook::default();
        assert!(
            b.request("h".into(), "s".into(), RemoteScope::View, 1, "", false)
                .is_err()
        );
        for (scope, min) in [
            (RemoteScope::View, 0),
            (RemoteScope::View, 121),
            (RemoteScope::Control, 31),
        ] {
            assert!(
                b.request("h".into(), "s".into(), scope, min, "", true)
                    .is_err()
            );
        }
        let p = b
            .request(
                "h".into(),
                "s".into(),
                RemoteScope::View,
                1,
                &format!("\0\u{202e}{}", "ä".repeat(400)),
                true,
            )
            .unwrap();
        let Payload::RemoteRequest { req, reason, .. } = p else {
            panic!()
        };
        assert_eq!(req.len(), 32);
        assert!(reason.len() <= 300);
        assert!(!reason.contains('\0'));
        assert!(!reason.contains('\u{202e}'));
        assert_eq!(b.get(&req).unwrap().side, Side::Viewer);
        assert!(
            b.request("h".into(), "s".into(), RemoteScope::View, 1, "", true)
                .is_err()
        );
        assert!(b.end(&req, "Ende").is_some());
        assert!(
            b.request("h".into(), "s".into(), RemoteScope::View, 1, "", true)
                .is_ok()
        );
    }
    #[test]
    fn incoming_requests_need_lead_valid_duration_and_capacity() {
        let mut b = RemoteBook::default();
        let p = request("r", RemoteScope::Control, 2);
        assert!(b.apply(&"lead".into(), &p, 0, &|_, _| false).is_err());
        assert!(
            b.apply(
                &"lead".into(),
                &request("bad", RemoteScope::Control, 31),
                0,
                &|_, _| true
            )
            .is_err()
        );
        assert_eq!(
            b.apply(&"lead".into(), &p, 0, &|_, _| true).unwrap(),
            Some(RemoteEffect::Asked { req: "r".into() })
        );
        assert_eq!(b.get("r").unwrap().reason, "Hilfe");
        assert_eq!(b.apply(&"lead".into(), &p, 0, &|_, _| true).unwrap(), None);
        assert!(
            b.apply(
                &"lead".into(),
                &request("other", RemoteScope::View, 1),
                0,
                &|_, _| true
            )
            .is_err()
        );
        for n in 1..MAX_GRANTS {
            b.apply(
                &format!("lead{n}").as_str().into(),
                &request(&n.to_string(), RemoteScope::View, 1),
                0,
                &|_, _| true,
            )
            .unwrap();
        }
        assert!(
            b.apply(
                &"extra".into(),
                &request("extra", RemoteScope::View, 1),
                0,
                &|_, _| true
            )
            .is_err()
        );
        b.end("r", "Nein");
        assert!(
            b.apply(
                &"extra".into(),
                &request("extra", RemoteScope::View, 1),
                0,
                &|_, _| true
            )
            .is_ok()
        );
    }
    #[test]
    fn grants_bind_peer_side_scope_state_and_expiry() {
        let (mut b, req) = viewer(RemoteScope::Suggest);
        let p = Payload::RemoteGrant {
            req: req.clone(),
            scope: RemoteScope::View,
            expires_ms: u64::MAX,
            session: session(),
        };
        assert!(b.apply(&"attacker".into(), &p, 100, &|_, _| false).is_err());
        assert!(activate(&mut b, &req, RemoteScope::Control, 1000).is_err());
        assert!(activate(&mut b, &req, RemoteScope::View, 100).is_err());
        assert_eq!(
            b.apply(&"host".into(), &p, 100, &|_, _| false).unwrap(),
            Some(RemoteEffect::Granted { req: req.clone() })
        );
        assert_eq!(
            b.get(&req).unwrap().state,
            GrantState::Active { expires_ms: 120100 }
        );
        assert!(b.apply(&"host".into(), &p, 100, &|_, _| false).is_err());
        assert!(
            b.accept(&req, RemoteScope::View, 1, session(), 100)
                .is_err()
        );
        let mut h = host(RemoteScope::View);
        assert!(
            h.apply(
                &"lead".into(),
                &Payload::RemoteGrant {
                    req: "r".into(),
                    scope: RemoteScope::View,
                    expires_ms: 500,
                    session: session()
                },
                100,
                &|_, _| false
            )
            .is_err()
        );
    }
    #[test]
    fn accept_only_host_requested_and_never_escalates() {
        let mut b = RemoteBook::default();
        b.apply(
            &"lead".into(),
            &request("r", RemoteScope::Suggest, 2),
            0,
            &|_, _| true,
        )
        .unwrap();
        assert!(
            b.accept("missing", RemoteScope::View, 1, session(), 0)
                .is_err()
        );
        assert!(
            b.accept("r", RemoteScope::Control, 1, session(), 0)
                .is_err()
        );
        assert!(
            b.accept("r", RemoteScope::Suggest, 3, session(), 0)
                .is_err()
        );
        assert!(b.accept("r", RemoteScope::View, 0, session(), 0).is_err());
        assert!(matches!(
            b.accept("r", RemoteScope::View, 1, session(), 100).unwrap(),
            Payload::RemoteGrant {
                expires_ms: 60100,
                scope: RemoteScope::View,
                ..
            }
        ));
        assert!(b.accept("r", RemoteScope::View, 1, session(), 0).is_err());
    }
    #[test]
    fn end_only_counterpart_and_idempotent_on_both_sides() {
        let mut b = host(RemoteScope::Control);
        let p = Payload::RemoteEnd {
            req: "r".into(),
            reason: "Stop\0".into(),
        };
        assert!(b.apply(&"attacker".into(), &p, 0, &|_, _| false).is_err());
        assert_eq!(
            b.apply(&"lead".into(), &p, 0, &|_, _| false).unwrap(),
            Some(RemoteEffect::Ended {
                req: "r".into(),
                reason: "Stop".into()
            })
        );
        assert_eq!(b.apply(&"lead".into(), &p, 0, &|_, _| false).unwrap(), None);
        assert!(b.end("r", "nochmal").is_none());
        let (mut v, req) = viewer(RemoteScope::View);
        assert!(v.end(&req, "Nein").is_some());
        assert!(v.end("missing", "").is_none());
    }
    #[test]
    fn frames_require_active_viewer_peer_size_and_increasing_seq() {
        let (mut b, req) = viewer(RemoteScope::View);
        let frame = |seq, reset, count| Payload::RemoteFrame {
            req: req.clone(),
            frame: RemoteFrame {
                seq,
                reset,
                items: vec![MirrorItem::Note { text: "x".into() }; count],
                state: "bereit".into(),
            },
        };
        assert!(
            b.apply(&"host".into(), &frame(1, true, 1), 100, &|_, _| false)
                .is_err()
        );
        activate(&mut b, &req, RemoteScope::View, 60100).unwrap();
        assert!(
            b.apply(&"attacker".into(), &frame(1, true, 1), 100, &|_, _| false)
                .is_err()
        );
        assert!(
            b.apply(
                &"host".into(),
                &frame(1, true, MAX_FRAME_ITEMS + 1),
                100,
                &|_, _| false
            )
            .is_err()
        );
        for seq in 1..=10 {
            assert!(matches!(
                b.apply(&"host".into(), &frame(seq, false, 64), 100, &|_, _| false)
                    .unwrap(),
                Some(RemoteEffect::Mirrored { .. })
            ));
        }
        assert_eq!(b.get(&req).unwrap().mirror.len(), 500);
        assert!(
            b.apply(&"host".into(), &frame(10, true, 0), 100, &|_, _| false)
                .is_err()
        );
        b.apply(&"host".into(), &frame(11, true, 2), 100, &|_, _| false)
            .unwrap();
        assert_eq!(b.get(&req).unwrap().mirror.len(), 2);
        assert!(
            b.apply(&"host".into(), &frame(12, false, 0), 60100, &|_, _| false)
                .is_err()
        );
        assert!(
            host(RemoteScope::View)
                .apply(
                    &"lead".into(),
                    &Payload::RemoteFrame {
                        req: "r".into(),
                        frame: RemoteFrame {
                            seq: 1,
                            reset: true,
                            items: vec![],
                            state: String::new()
                        }
                    },
                    100,
                    &|_, _| false
                )
                .is_err()
        );
    }
    #[test]
    fn input_scope_expiry_peer_and_suggestion_limits() {
        let input = |text: &str| Payload::RemoteInput {
            req: "r".into(),
            text: text.into(),
        };
        assert!(
            host(RemoteScope::View)
                .apply(&"lead".into(), &input("x"), 100, &|_, _| false)
                .is_err()
        );
        let mut b = host(RemoteScope::Suggest);
        assert!(
            b.apply(&"attacker".into(), &input("x"), 100, &|_, _| false)
                .is_err()
        );
        for _ in 0..20 {
            assert_eq!(
                b.apply(
                    &"lead".into(),
                    &input(&format!("\0{}", "ä".repeat(3000))),
                    100,
                    &|_, _| false
                )
                .unwrap(),
                Some(RemoteEffect::Suggested { req: "r".into() })
            );
        }
        assert_eq!(b.get("r").unwrap().suggestions[0].len(), 4000);
        assert!(
            b.apply(&"lead".into(), &input("x"), 100, &|_, _| false)
                .is_err()
        );
        let mut c = host(RemoteScope::Control);
        assert_eq!(
            c.apply(&"lead".into(), &input("a\0\u{202e}b"), 100, &|_, _| false)
                .unwrap(),
            Some(RemoteEffect::Inject {
                req: "r".into(),
                text: "ab".into()
            })
        );
        assert!(
            c.apply(&"lead".into(), &input("x"), 60100, &|_, _| false)
                .is_err()
        );
        c.end("r", "Stop");
        assert!(
            c.apply(&"lead".into(), &input("x"), 100, &|_, _| false)
                .is_err()
        );
        let (mut v, _) = viewer(RemoteScope::Control);
        assert!(
            v.apply(&"host".into(), &input("x"), 100, &|_, _| false)
                .is_err()
        );
    }
    #[test]
    fn expire_both_sides_and_active_hosts() {
        let mut b = host(RemoteScope::Control);
        assert_eq!(b.active_hosts(60099).len(), 1);
        assert!(b.active_hosts(60100).is_empty());
        assert!(b.expire(60099).is_empty());
        assert!(
            matches!(b.expire(60100).as_slice(), [(peer, Payload::RemoteEnd { req, .. })] if peer.as_str() == "lead" && req == "r")
        );
        assert!(b.expire(60101).is_empty());
        let (mut v, req) = viewer(RemoteScope::View);
        activate(&mut v, &req, RemoteScope::View, 200).unwrap();
        assert!(v.active_hosts(100).is_empty());
        assert_eq!(v.expire(200).len(), 1);
    }
    #[test]
    fn prune_ended_and_reject_unrelated_payload() {
        let mut b = RemoteBook::default();
        for _ in 0..70 {
            let Payload::RemoteRequest { req, .. } = b
                .request("h".into(), "s".into(), RemoteScope::View, 1, "", true)
                .unwrap()
            else {
                panic!()
            };
            b.end(&req, "Ende");
        }
        assert_eq!(b.grants.len(), 50);
        assert!(
            b.apply(
                &"h".into(),
                &Payload::Leave { chat: "c".into() },
                0,
                &|_, _| true
            )
            .is_err()
        );
    }
    #[test]
    fn redacts_common_secrets_and_preserves_normal_text() {
        for secret in [
            "sk-example123",
            "ghp_example123",
            "github_pat_example123",
            "xoxb-example123",
            "AKIAEXAMPLE123",
            "Bearer example123",
            "Bearer   example123",
            "Bearer\t example123",
            "bearer example123",
            "password=example123",
            "token=example123",
            "token=\"example 123\"",
            "PASSWORD='example123'",
        ] {
            let out = redact(&format!("Anfang {secret}; Ende"));
            assert!(!out.contains("example"), "{out}");
            assert!(!out.contains("EXAMPLE"), "{out}");
            assert!(out.contains("[GESCHWÄRZT]"));
            assert!(out.ends_with("; Ende"));
        }
        for kind in [
            "RSA PRIVATE KEY",
            "EC PRIVATE KEY",
            "PRIVATE KEY",
            "OPENSSH PRIVATE KEY",
        ] {
            let out = redact(&format!(
                "vor\n-----BEGIN {kind}-----\nsecret\n-----END {kind}-----\nnach"
            ));
            assert_eq!(out, "vor\n[GESCHWÄRZT]\nnach");
        }
        assert_eq!(
            redact("-----BEGIN PRIVATE KEY-----\nsecret"),
            "[GESCHWÄRZT]"
        );
        assert_eq!(
            redact("Hallo äö \n-----BEGIN CERTIFICATE-----\nöffentlich"),
            "Hallo äö \n-----BEGIN CERTIFICATE-----\nöffentlich"
        );
    }
}
