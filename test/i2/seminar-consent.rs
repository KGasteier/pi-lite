//! S6-Angriffstests gegen den öffentlichen, modellfreien S8-Automaten.
use pi_net::{
    remote::{RemoteScope, SessionRef},
    seminar::{demo_pair, MemoryNet, SeminarNet},
    wire::Payload,
};
const NOW: u64 = 1_000_000;
fn deliver(effects: pi_net::seminar::Effects, net: &mut MemoryNet) {
    for prompt in effects.hub {
        net.senden(&prompt.session, &prompt.text, &prompt.von)
            .unwrap();
    }
}
#[test]
fn no_consent_never_reaches_hub_then_explicit_consent_can_send() {
    let (mut leader, mut owner) = demo_pair(NOW);
    let mut net = MemoryNet::default();
    let session = owner.sessions[0].clone();
    // Selbst mit gültigem Leitungsausweis berechtigt eine erfundene Anfrage nicht.
    assert!(owner
        .receive(
            &leader.chats.me,
            Payload::Steuern {
                req: "erfunden".into(),
                text: "ANGRIFF".into()
            },
            NOW
        )
        .is_err());
    assert!(net.hub.is_empty());
    let (_, request) = leader
        .request(&owner.chats.me, &session, RemoteScope::Control, NOW)
        .unwrap()
        .remove(0);
    let Payload::RemoteRequest { ref req, .. } = request else {
        panic!("Keine Anfrage")
    };
    let req = req.clone();
    deliver(
        owner.receive(&leader.chats.me, request, NOW).unwrap(),
        &mut net,
    );
    assert!(leader.control(&req, "ANGRIFF vor Zustimmung", NOW).is_err());
    let input = || Payload::Steuern {
        req: req.clone(),
        text: "ANGRIFF ohne Zustimmung".into(),
    };
    assert!(owner.receive(&leader.chats.me, input(), NOW).is_err());
    assert!(net.hub.is_empty(), "Kein Prompt vor expliziter Zustimmung");
    let grant = owner
        .accept(&req, RemoteScope::Control, &session, NOW)
        .unwrap()
        .remove(0)
        .1;
    leader.receive(&owner.chats.me, grant, NOW).unwrap();
    let p = leader
        .control(&req, "Erlaubter Testprompt", NOW + 1)
        .unwrap()
        .remove(0)
        .1;
    deliver(
        owner.receive(&leader.chats.me, p, NOW + 1).unwrap(),
        &mut net,
    );
    assert_eq!(net.hub.len(), 1);
    assert_eq!(net.hub[0].text, "Erlaubter Testprompt");
    assert_eq!(net.hub[0].session, session);
    assert_eq!(net.hub[0].von.label(), "Aylin · Seminarleitung");
    // Nach Beenden darf auch ein wiederholter, vorher gültiger Request nicht senden.
    owner.end(&req, "Beenden");
    assert!(owner.receive(&leader.chats.me, input(), NOW + 2).is_err());
    assert_eq!(net.hub.len(), 1);
}
#[test]
fn view_only_rejection_checkout_and_wrong_session_never_send() {
    for mode in ["view", "reject", "checkout"] {
        let (mut leader, mut owner) = demo_pair(NOW);
        let mut net = MemoryNet::default();
        let session = owner.sessions[0].clone();
        let request = leader
            .request(&owner.chats.me, &session, RemoteScope::Control, NOW)
            .unwrap()
            .remove(0)
            .1;
        let Payload::RemoteRequest { ref req, .. } = request else {
            panic!()
        };
        let req = req.clone();
        owner.receive(&leader.chats.me, request, NOW).unwrap();
        let other = SessionRef {
            label: "Fremde Session".into(),
            cwd: "session-fremd".into(),
        };
        assert!(owner
            .accept(&req, RemoteScope::Control, &other, NOW)
            .is_err());
        match mode {
            "view" => {
                owner
                    .accept(&req, RemoteScope::View, &session, NOW)
                    .unwrap();
            }
            "reject" => {
                owner.end(&req, "Ablehnen");
            }
            _ => {
                owner.checkout();
            }
        }
        let result = owner.receive(
            &leader.chats.me,
            Payload::Steuern {
                req,
                text: "Nicht senden".into(),
            },
            NOW + 1,
        );
        assert!(result.is_err(), "{mode}");
        if let Ok(effects) = result {
            deliver(effects, &mut net);
        }
        assert!(net.hub.is_empty(), "{mode}");
    }
}
