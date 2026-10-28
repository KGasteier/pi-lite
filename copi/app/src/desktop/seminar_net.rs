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
    serde_json::json!({"id":peer.as_str(), "addrs":[]}).to_string()
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
    use pi_net::ausweis::Rolle;
    use serde_json::json;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Duration,
    };

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
