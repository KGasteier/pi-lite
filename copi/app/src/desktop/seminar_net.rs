//! Slot-Hub-Anbindung für den produktiven pi-net-Seminaradapter.
//! Kein eigener HTTP-/Loginvertrag und kein lokaler Agenten-Fallback.
use super::Seminar;
use pi_backend::hub_v1::HubV1;
use pi_net::{
    ausweis::Ausweis,
    identity::Identity,
    remote::SessionRef,
    seminar::{
        SeminarState, Von,
        net::{NodeNet, SeminarHub},
    },
};

/// N4-Knoten-ID validieren, ohne in der App eine zweite iroh-Abhängigkeit.
pub(super) fn directory_peer(key: &str) -> Result<pi_net::model::PeerId, String> {
    let peer: pi_net::model::PeerId = key.into();
    if key.len() != 64
        || !key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || pi_net::node::parse_address(&directory_address(&peer))? != peer
    {
        return Err("Ungültige Knoten-ID im Seminarverzeichnis".into());
    }
    Ok(peer)
}
pub(super) fn directory_address(peer: &pi_net::model::PeerId) -> String {
    directory_contact(peer, &[])
}

/// Keine Hostnamen/Relay-URLs; dieselbe Begrenzung gilt beim Melden und Finden.
fn bounded_addresses(addresses: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut direct = vec![];
    for address in addresses {
        let Ok(socket) = address.parse::<std::net::SocketAddr>() else {
            continue;
        };
        let address = socket.to_string();
        if !direct.contains(&address) {
            direct.push(address);
            if direct.len() == 8 {
                break;
            }
        }
    }
    direct
}

pub(super) fn directory_contact(peer: &pi_net::model::PeerId, addresses: &[String]) -> String {
    let addrs: Vec<_> = bounded_addresses(addresses.iter().cloned())
        .into_iter()
        .map(|address| serde_json::json!({"Ip": address}))
        .collect();
    serde_json::json!({"id":peer.as_str(), "addrs":addrs}).to_string()
}

pub(super) fn direct_addresses(address: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(address) else {
        return vec![];
    };
    bounded_addresses(
        value
            .get("addrs")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|a| a.get("Ip")?.as_str().map(str::to_owned)),
    )
}

pub struct SlotHub {
    hub: HubV1,
    /// Nur eigene, vom Hub bestätigte Sessions. cwd ist keine Remote-Adresse.
    sessions: Vec<(SessionRef, String)>,
}
impl SlotHub {
    /// `hub` ist bereits als teilnehmendes Gerät am eigenen Slot angemeldet.
    pub fn new(hub: HubV1, sessions: Vec<(SessionRef, String)>) -> Result<Self, String> {
        if sessions.len() > 2
            || sessions.iter().any(|(_, id)| id.is_empty())
            || sessions.iter().enumerate().any(|(i, (session, id))| {
                sessions[..i]
                    .iter()
                    .any(|(other, other_id)| other == session || other_id == id)
            })
        {
            return Err("Die eigenen Hub-Sessions sind nicht eindeutig bestätigt".into());
        }
        Ok(Self { hub, sessions })
    }
}
impl SeminarHub for SlotHub {
    fn update_sessions(
        &mut self,
        sessions: &[pi_net::wire::SessionAnkuendigung],
    ) -> Result<(), String> {
        pi_net::wire::SessionAnkuendigung::pruefen(sessions)?;
        if sessions.len() > 2 {
            return Err("Am eigenen Arbeitsplatz sind höchstens zwei Sessions erlaubt".into());
        }
        self.sessions = sessions
            .iter()
            .map(|s| {
                (
                    SessionRef {
                        label: s.label.clone(),
                        cwd: s.id.clone(),
                    },
                    s.id.clone(),
                )
            })
            .collect();
        Ok(())
    }
    fn senden(&mut self, session: &SessionRef, text: &str, von: &Von) -> Result<(), String> {
        let id = self
            .sessions
            .iter()
            .find(|(own, _)| own == session)
            .map(|(_, id)| id)
            .ok_or("Diese Session gehört nicht zum eigenen Arbeitsplatz")?;
        let rolle = match von.rolle {
            pi_net::ausweis::Rolle::Seminarleitung => "seminarleitung",
            _ => return Err("Nur die bestätigte Seminarleitung darf mitsteuern".into()),
        };
        self.hub.send_with_sender(id, text, &von.person, rolle)?;
        Ok(())
    }
}

/// Nach Check-in, Pin-Bestätigung und bewusstem Netzstart. Eigene P2P-Identität
/// unbedingt schon beim Check-in benutzen, nicht erst hier neu erzeugen.
pub fn production(
    mut state: SeminarState,
    identity: Identity,
    ausweis: Ausweis,
    hub: HubV1,
    sessions: Vec<(SessionRef, String)>,
) -> Result<Seminar, String> {
    if !state.checked_in || state.chats.me != identity.peer() {
        return Err("Check-in und Netzidentität passen nicht zusammen".into());
    }
    state.identify(&identity.peer(), &ausweis, pi_net::model::now_ms())?;
    // Ausschließlich bestätigte Hub-IDs ankündigen, nie den historischen cwd.
    let sessions: Vec<_> = sessions
        .into_iter()
        .map(|(session, id)| {
            (
                SessionRef {
                    label: session.label,
                    cwd: id.clone(),
                },
                id,
            )
        })
        .collect();
    let announcements: Vec<_> = sessions
        .iter()
        .map(|(session, id)| pi_net::wire::SessionAnkuendigung {
            id: id.clone(),
            label: session.label.clone(),
            zustand: "unbekannt".into(),
        })
        .collect();
    pi_net::wire::SessionAnkuendigung::pruefen(&announcements)?;
    let slot = SlotHub::new(hub, sessions)?;
    state.sessions = slot
        .sessions
        .iter()
        .map(|(session, _)| session.clone())
        .collect();
    let net = NodeNet::start(identity, ausweis, state.issuer().into(), Box::new(slot))?;
    Ok(Seminar::new(state, Box::new(net)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_net::ausweis::{Inhalt, Rolle, base64url, kanonisch};
    use pi_net::{
        model::{PeerId, now_ms},
        remote::{GrantState, RemoteScope},
    };
    use serde_json::json;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::{Duration, Instant},
    };

    fn wait_until(deadline: Instant, mut condition: impl FnMut() -> bool) {
        while !condition() {
            assert!(
                Instant::now() < deadline,
                "Zwei-Knoten-Ablauf muss unter 30 s bleiben"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn certificate(issuer: &Identity, peer: &PeerId, person: &str, rolle: Rolle) -> Ausweis {
        let inhalt = Inhalt {
            v: 1,
            typ: "geraeteausweis".into(),
            pid: format!("test-{person}"),
            person: person.into(),
            rolle,
            net_pubkey: peer.to_string(),
            geraet_pubkey: base64url(&[73; 32]),
            iat: now_ms() / 1000 - 1,
            exp: now_ms() / 1000 + 3600,
        };
        let signatur = base64url(&issuer.secret.sign(&kanonisch(&inhalt).unwrap()).to_bytes());
        Ausweis { inhalt, signatur }
    }
    fn loopback(addresses: Vec<String>) -> Vec<String> {
        assert!(!addresses.is_empty());
        addresses
            .into_iter()
            .map(|address| {
                let socket: std::net::SocketAddr = address.parse().unwrap();
                let ip = if socket.is_ipv4() { "127.0.0.1" } else { "::1" }
                    .parse()
                    .unwrap();
                std::net::SocketAddr::new(ip, socket.port()).to_string()
            })
            .collect()
    }
    fn deliver(seminar: &mut Seminar, out: pi_net::store::Outbox) {
        seminar.dispatch(Ok(out));
        assert!(seminar.error.is_none(), "{:?}", seminar.error);
    }
    fn directory_entry(
        peer: &PeerId,
        name: &str,
        adressen: Vec<String>,
    ) -> super::super::VerzeichnisEintrag {
        super::super::VerzeichnisEintrag {
            person: format!("test-{name}"),
            anzeige: name.into(),
            // Absichtlich gefälscht: diese Angabe darf keine Rechte verleihen.
            rolle: "seminarleitung".into(),
            net_pubkey: peer.to_string(),
            adressen,
        }
    }
    #[test]
    fn two_nodes_directory_chat_dm_consent_send_as_mock_hub_and_expiry_under_30s() {
        let started = Instant::now();
        let deadline = started + Duration::from_secs(25);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let hub_address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut accepted = None;
            wait_until(deadline, || match listener.accept() {
                Ok((socket, _)) => {
                    accepted = Some(socket);
                    true
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
                Err(e) => panic!("Mock-Hub: {e}"),
            });
            let mut socket = accepted.unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
                assert!(header.len() < 16_384);
            }
            let header = String::from_utf8(header).unwrap();
            assert!(
                header.starts_with("POST /v1/sessions/own-id/senden HTTP/1.1\r\n"),
                "{header}"
            );
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                body,
                json!({"protokoll":1,"text":"Bitte gliedern", "von":{"person":"Aylin","rolle":"seminarleitung"}})
            );
            let reply = json!({"protokoll":1,"typ":"gesendet"}).to_string();
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).unwrap();
            body
        });
        let issuer = Identity::ephemeral();
        let pin = base64url(issuer.secret.public().as_bytes());
        let ia = Identity::ephemeral();
        let ib = Identity::ephemeral();
        let pa = ia.peer();
        let pb = ib.peer();
        let ca = certificate(&issuer, &pa, "Aylin", Rolle::Seminarleitung);
        let cb = certificate(&issuer, &pb, "Mira", Rolle::Teilnehmende);
        let session = SessionRef {
            label: "Handout".into(),
            cwd: "own-id".into(),
        };
        let mut state_a = SeminarState::new(pa.clone(), "Aylin", pin.clone(), "seminar".into());
        state_a.identify(&pa, &ca, now_ms()).unwrap();
        let a_session = SessionRef {
            label: "Kursplan".into(),
            cwd: "a-own-id".into(),
        };
        state_a.sessions = vec![a_session.clone()];
        let mut state_b = SeminarState::new(pb.clone(), "Mira", pin.clone(), "seminar".into());
        state_b.identify(&pb, &cb, now_ms()).unwrap();
        state_b.sessions = vec![session.clone()];
        let hub = HubV1::new(&format!("http://{hub_address}")).unwrap();
        let slot = SlotHub::new(hub, vec![(a_session.clone(), "a-own-id".into())]).unwrap();
        let na = NodeNet::start(ia, ca, pin.clone(), Box::new(slot)).unwrap();
        let hub = HubV1::new(&format!("http://{hub_address}")).unwrap();
        let slot = SlotHub::new(hub, vec![(session.clone(), "own-id".into())]).unwrap();
        let nb = NodeNet::start(ib, cb, pin, Box::new(slot)).unwrap();
        let mut a = Seminar::new(state_a, Box::new(na));
        let mut b = Seminar::new(state_b, Box::new(nb));
        wait_until(deadline, || {
            a.tick();
            b.tick();
            !a.address().is_empty() && !b.address().is_empty()
        });
        // Nur Fixture-Adressen auf Loopback begrenzen; kein set_contacts oder
        // manueller Kontaktweg. Genau der N11-Verzeichnisabruf verbindet beide.
        let aa = loopback(a.address());
        let ba = loopback(b.address());
        a.verzeichnis_uebernehmen(vec![
            directory_entry(&pa, "Aylin", aa.clone()),
            directory_entry(&pb, "Mira", ba.clone()),
        ]);
        b.verzeichnis_uebernehmen(vec![
            directory_entry(&pa, "Aylin", aa),
            directory_entry(&pb, "Mira", ba),
        ]);
        assert_eq!(a.state.badge(&pb, now_ms()), "unbestätigt");
        assert!(a.state.people[0].sessions.is_empty());
        assert!(a.state.direct(&pb, now_ms()).is_err());
        wait_until(deadline, || {
            a.tick();
            b.tick();
            a.state.message_unavailable(&pb, now_ms()).is_none()
                && b.state.message_unavailable(&pa, now_ms()).is_none()
        });
        // Frühe Allowlist-Rennen dürfen vor bestätigtem Netzstart Fehler melden.
        a.error = None;
        b.error = None;
        assert_eq!(a.state.badge(&pb, now_ms()), "Teilnehmende");
        assert_eq!(b.state.badge(&pa, now_ms()), "Seminarleitung");
        // N15: Die Session erreicht die Leitung über QUIC, ohne fremde Fixture.
        wait_until(deadline, || {
            a.tick();
            b.tick();
            a.state.people[0].sessions == vec![session.clone()]
                && b.state.people[0].sessions == vec![a_session.clone()]
        });
        // Auch Zustandswechsel und Entfernen ohne Verbindungsneuaufbau.
        let item = pi_net::wire::SessionAnkuendigung {
            id: "a-own-id".into(),
            label: "Kursplan".into(),
            zustand: "arbeitet".into(),
        };
        a.sessions_aktualisieren(vec![item.clone()]).unwrap();
        wait_until(deadline, || {
            a.tick();
            b.tick();
            b.state
                .session_announcements
                .get(&pa)
                .is_some_and(|s| s == &vec![item.clone()])
        });
        a.sessions_aktualisieren(vec![]).unwrap();
        wait_until(deadline, || {
            a.tick();
            b.tick();
            b.state.people[0].sessions.is_empty()
        });
        assert!(b.state.people[0].sessions.is_empty());
        assert_eq!(a.state.session_announcements[&pb][0].id, "own-id");
        assert_eq!(a.state.session_announcements[&pb][0].zustand, "unbekannt");
        let out = a.state.create_group(now_ms()).unwrap();
        deliver(&mut a, out);
        wait_until(deadline, || {
            b.tick();
            !b.state.chats.invites.is_empty()
        });
        let group = b.state.chats.invites[0].chat.id.clone();
        let out = b.state.chats.accept_invite(&group).unwrap();
        deliver(&mut b, out);
        b.state.group = Some(group.clone());
        b.state.selected = Some(group.clone());
        let out = a.state.post("Hallo Seminar").unwrap();
        deliver(&mut a, out);
        wait_until(deadline, || {
            b.tick();
            !b.state.chats.chat(&group).unwrap().messages.is_empty()
        });
        assert_eq!(
            b.state
                .chats
                .chat(&group)
                .unwrap()
                .messages
                .last()
                .unwrap()
                .body
                .text(),
            "Hallo Seminar"
        );
        let out = a.state.direct(&pb, now_ms()).unwrap();
        deliver(&mut a, out);
        wait_until(deadline, || {
            b.tick();
            !b.state.chats.invites.is_empty()
        });
        let dm = b.state.chats.invites[0].chat.id.clone();
        assert_ne!(dm, group);
        let out = b.state.chats.accept_invite(&dm).unwrap();
        deliver(&mut b, out);
        let out = a.state.post("Nur für Mira").unwrap();
        deliver(&mut a, out);
        wait_until(deadline, || {
            b.tick();
            !b.state.chats.chat(&dm).unwrap().messages.is_empty()
        });
        assert_eq!(
            b.state
                .chats
                .chat(&dm)
                .unwrap()
                .messages
                .last()
                .unwrap()
                .body
                .text(),
            "Nur für Mira"
        );
        let out = a
            .state
            .request(&pb, &session, RemoteScope::Control, now_ms())
            .unwrap();
        deliver(&mut a, out);
        wait_until(deadline, || {
            b.tick();
            !b.state.remote.grants.is_empty()
        });
        assert!(b.marked_prompts.is_empty());
        let req = b.state.remote.grants[0].req.clone();
        assert!(a.state.control(&req, "Noch nicht", now_ms()).is_err());
        let out = b
            .state
            .accept(&req, RemoteScope::Control, &session, now_ms())
            .unwrap();
        deliver(&mut b, out);
        wait_until(deadline, || {
            a.tick();
            a.state
                .remote
                .get(&req)
                .is_some_and(|g| matches!(g.state, GrantState::Active { .. }))
        });
        let out = a.state.control(&req, "Bitte gliedern", now_ms()).unwrap();
        deliver(&mut a, out);
        wait_until(deadline, || {
            b.tick();
            !b.marked_prompts.is_empty()
        });
        assert_eq!(b.marked_prompts.len(), 1);
        assert_eq!(b.marked_prompts[0].von.label(), "Aylin · Seminarleitung");
        assert_eq!(server.join().unwrap()["text"], "Bitte gliedern");
        let GrantState::Active { expires_ms } = b.state.remote.get(&req).unwrap().state else {
            panic!()
        };
        // Logische Zeit statt 30 Minuten Schlaf. Ablauf-Ende geht über QUIC zurück.
        b.demo_now = Some(expires_ms);
        b.tick();
        assert!(b.state.remote.grants.is_empty());
        wait_until(deadline, || {
            a.tick();
            a.state
                .remote
                .get(&req)
                .is_none_or(|g| matches!(g.state, GrantState::Ended { .. }))
        });
        assert!(a.state.control(&req, "Zu spät", now_ms()).is_err());
        a.checkout();
        b.checkout();
        assert!(a.address().is_empty());
        assert!(b.address().is_empty());
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "{:?}",
            started.elapsed()
        );
        eprintln!(
            "Zwei-Knoten-Ablauf einschließlich Mock-Hub: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn slot_session_updates_are_atomic_bounded_and_remove_stale_hub_targets() {
        let hub = HubV1::new("http://127.0.0.1:1").unwrap();
        let item = pi_net::wire::SessionAnkuendigung {
            id: "hub-1".into(),
            label: "Plakat".into(),
            zustand: "ruht".into(),
        };
        let mut slot = SlotHub::new(hub, vec![]).unwrap();
        slot.update_sessions(std::slice::from_ref(&item)).unwrap();
        assert_eq!(slot.sessions[0].1, "hub-1");
        assert_eq!(slot.sessions[0].0.cwd, "hub-1");
        let before = slot.sessions.clone();
        let three: Vec<_> = (0..3)
            .map(|i| pi_net::wire::SessionAnkuendigung {
                id: format!("hub-{i}"),
                ..item.clone()
            })
            .collect();
        assert!(slot.update_sessions(&three).is_err());
        assert_eq!(slot.sessions, before);
        slot.update_sessions(&[]).unwrap();
        assert!(slot.sessions.is_empty());
        assert!(
            slot.senden(
                &before[0].0,
                "Nie",
                &Von {
                    person: "Aylin".into(),
                    rolle: Rolle::Seminarleitung
                }
            )
            .unwrap_err()
            .contains("Session gehört nicht")
        );
    }

    #[test]
    fn slot_hub_posts_sender_and_own_id_not_untrusted_cwd() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut header = vec![];
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
                assert!(header.len() < 16_384);
            }
            let header = String::from_utf8(header).unwrap();
            assert!(
                header.starts_with("POST /v1/sessions/own-id/senden HTTP/1.1\r\n"),
                "{header}"
            );
            let length: usize = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|n| n.trim().parse().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                json!({"protokoll":1,"text":"Bitte gliedern", "von":{"person":"Aylin","rolle":"seminarleitung"}})
            );
            let body = json!({"protokoll":1,"typ":"gesendet"}).to_string();
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let hub = HubV1::new(&format!("http://{address}")).unwrap();
        let own = SessionRef {
            label: "Handout".into(),
            cwd: "not-a-hub-id".into(),
        };
        let mut slot = SlotHub::new(hub, vec![(own.clone(), "own-id".into())]).unwrap();
        let von = Von {
            person: "Aylin".into(),
            rolle: Rolle::Seminarleitung,
        };
        assert!(
            slot.senden(
                &SessionRef {
                    label: own.label.clone(),
                    cwd: "foreign".into()
                },
                "Nie",
                &von
            )
            .is_err()
        );
        assert!(
            slot.senden(
                &own,
                "Nie",
                &Von {
                    person: "Mira".into(),
                    rolle: Rolle::Teilnehmende
                }
            )
            .is_err()
        );
        slot.senden(&own, "Bitte gliedern", &von).unwrap();
        server.join().unwrap();
    }
}
