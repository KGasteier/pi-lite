//! Nachrichtenformat zwischen Geräten (eine QUIC-Bi-Stream-Übertragung je [`Envelope`]).
//!
//! Sicherheitsgrundsatz: Der **Absender** eines Umschlags ist nie Teil der Nutzlast,
//! sondern wird vom Empfänger aus der TLS-Gegenstelle (`remote_id`) bestimmt und als
//! `from` an [`crate::policy`] / [`crate::store::ChatStore::apply`] übergeben.
use serde::{Deserialize, Serialize};

use crate::model::{Chat, ChatId, Member, Message, MsgId, Space};
use crate::remote::{RemoteFrame, RemoteScope, SessionRef};

/// Protokollversion; unbekannte Versionen werden abgewiesen.
pub const PROTOCOL: u16 = 1;
/// Obergrenze für einen serialisierten Umschlag.
pub const MAX_ENVELOPE: usize = 64 * 1024;
/// Obergrenze für Text in einer Nachricht (Bytes, UTF-8).
pub const MAX_TEXT: usize = 32 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub v: u16,
    pub payload: Payload,
}

impl Envelope {
    pub fn new(payload: Payload) -> Self {
        Self {
            v: PROTOCOL,
            payload,
        }
    }
    /// Serialisieren mit Größenprüfung.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let b = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if b.len() > MAX_ENVELOPE {
            return Err(format!(
                "Nachricht zu groß ({} > {} Bytes)",
                b.len(),
                MAX_ENVELOPE
            ));
        }
        Ok(b)
    }
    /// Deserialisieren mit Größen- und Versionsprüfung.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        if b.len() > MAX_ENVELOPE {
            return Err("Nachricht zu groß".into());
        }
        let e: Envelope =
            serde_json::from_slice(b).map_err(|e| format!("Ungültiges Format: {e}"))?;
        if e.v != PROTOCOL {
            return Err(format!("Protokollversion {} nicht unterstützt", e.v));
        }
        Ok(e)
    }
}

/// N15: nur Metadaten eigener Hub-Sessions, niemals Inhalte oder Dateipfade.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAnkuendigung {
    /// Undurchsichtige Hub-ID; ausdrücklich kein Arbeitsverzeichnis.
    pub id: String,
    pub label: String,
    pub zustand: String,
}
impl SessionAnkuendigung {
    /// Ganze Listen atomar prüfen; fehlerhafte Listen niemals abschneiden.
    pub fn pruefen(sessions: &[Self]) -> Result<(), String> {
        let mut ids = std::collections::BTreeSet::new();
        if sessions.len() > 32
            || sessions.iter().any(|s| {
                s.id.is_empty()
                    || s.id.len() > 256
                    || !s
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    || !ids.insert(&s.id)
                    || s.label.trim().is_empty()
                    || s.label.chars().count() > 120
                    || s.label.chars().any(char::is_control)
                    || !matches!(
                        s.zustand.as_str(),
                        "bereit"
                            | "ruht"
                            | "arbeitet"
                            | "wartet auf dich"
                            | "fertig"
                            | "Fehler"
                            | "unbekannt"
                    )
            })
        {
            return Err("Sessionliste ungültig: höchstens 32 Sessions mit kurzem Titel und gültiger ID sowie Zustand erwartet".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Payload {
    /// Additiver A6-Handshake; fehlend/ungültig bleibt unbestätigt.
    Geraeteausweis { ausweis: crate::ausweis::Ausweis },
    /// Additive Seminarhülle: TLS-Absender bleibt maßgeblich; ID/Zeit begrenzen
    /// Wiederholungen des Transportworkers, insbesondere bei Mitsteuer-Prompts.
    SeminarPacket {
        id: String,
        sent_ms: u64,
        payload: Box<Payload>,
    },
    /// Frischer, in SeminarPacket eingehüllter Lebensnachweis (keine Rollenquelle).
    SeminarPresence,
    /// N15: ersetzt vollständig die Sessionliste; nur innerhalb SeminarPacket.
    /// Protokoll bleibt 1. Alt-Knoten ohne diesen serde-Tag lehnen den gesamten
    /// Umschlag ab (auch in SeminarPacket), nicht stilles Ignorieren. Der Node
    /// antwortet `err`/Error und der Sender kann transportüblich wiederholen;
    /// bekannte Folgestreams bleiben dekodierbar. Kein Capability-Handshake.
    SeminarSessions { sessions: Vec<SessionAnkuendigung> },
    /// A9 Eingabe; identische Consent-Prüfung wie RemoteInput (v2 bleibt gültig).
    Steuern { req: String, text: String },
    /// Einladung in einen Chat (ohne Verlauf). Annahme nur nach Bestätigung durch die Person.
    Invite {
        chat: Chat,
        space: Option<Space>,
        note: String,
    },
    /// Annahme bzw. Ablehnung einer Einladung.
    InviteAnswer { chat: ChatId, accept: bool },
    /// Neue Chatnachricht.
    Post { chat: ChatId, msg: Message },
    /// Empfangsbestätigung auf Anwendungsebene (zusätzlich zum Transport-ACK).
    Ack { chat: ChatId, msg: MsgId },
    /// Mitgliederliste ersetzt (nur wer laut [`crate::policy`] verwalten darf).
    Members { chat: ChatId, members: Vec<Member> },
    /// Chat verlassen.
    Leave { chat: ChatId },
    /// Seminar: Tutorial-Schritt (nur Leitung).
    Step {
        space: crate::model::SpaceId,
        step: usize,
    },

    // ── Fernzugriff Seminarleitung → Teilnehmenden-Session ──
    /// Leitung bittet um Einsicht/Mitarbeit; Teilnehmende entscheiden.
    RemoteRequest {
        req: String,
        space: crate::model::SpaceId,
        scope: RemoteScope,
        minutes: u32,
        reason: String,
    },
    /// Freigabe (ggf. mit geringerem Umfang/kürzerer Dauer als erbeten).
    RemoteGrant {
        req: String,
        scope: RemoteScope,
        expires_ms: u64,
        session: SessionRef,
    },
    /// Ablehnung, Widerruf oder Ablauf (beide Richtungen).
    RemoteEnd { req: String, reason: String },
    /// Gespiegelter Verlauf (Teilnehmende → Leitung).
    RemoteFrame { req: String, frame: RemoteFrame },
    /// Eingabe der Leitung (Leitung → Teilnehmende); Behandlung je nach Umfang.
    RemoteInput { req: String, text: String },
}

#[cfg(test)]
mod session_tests {
    use super::*;

    #[test]
    fn sessions_wire_roundtrip_is_additive_without_paths_or_contents() {
        let env = Envelope::new(Payload::SeminarPacket {
            id: "a".repeat(32),
            sent_ms: 123,
            payload: Box::new(Payload::SeminarSessions {
                sessions: vec![SessionAnkuendigung {
                    id: "hub-1".into(),
                    label: "Plakat".into(),
                    zustand: "ruht".into(),
                }],
            }),
        });
        let encoded = env.encode().unwrap();
        assert_eq!(Envelope::decode(&encoded).unwrap(), env);
        let json: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(json["v"], 1);
        assert_eq!(json["payload"]["payload"]["t"], "seminar_sessions");
        let item = json["payload"]["payload"]["sessions"][0]
            .as_object()
            .unwrap();
        assert_eq!(item.len(), 3);
        for field in ["id", "label", "zustand"] {
            assert!(item.contains_key(field));
        }
    }

    #[test]
    fn old_node_rejects_unknown_session_variant_but_still_decodes_next_message() {
        // Gleicher serde-Vertrag wie vor N15: intern getaggt, kein serde(other).
        // Exakte Altversion siehe Git-Elternstand von wire.rs.
        #[derive(Debug, Deserialize)]
        #[serde(tag = "t", rename_all = "snake_case")]
        enum OldPayload {
            SeminarPresence,
            SeminarPacket { payload: Box<OldPayload> },
        }
        let unknown = serde_json::json!({"t":"seminar_packet", "id":"a".repeat(32), "sent_ms":123,
            "payload":{"t":"seminar_sessions","sessions":[]}});
        let error = serde_json::from_value::<OldPayload>(unknown).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unknown variant `seminar_sessions`"),
            "{error}"
        );
        let next: OldPayload = serde_json::from_value(serde_json::json!({"t":"seminar_packet",
            "payload":{"t":"seminar_presence"}}))
        .unwrap();
        let OldPayload::SeminarPacket { payload } = next else {
            panic!()
        };
        assert!(matches!(*payload, OldPayload::SeminarPresence));
    }
}
