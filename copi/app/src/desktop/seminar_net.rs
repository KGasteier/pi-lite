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
