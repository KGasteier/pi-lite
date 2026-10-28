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
        Self { v: PROTOCOL, payload }
    }
    /// Serialisieren mit Größenprüfung.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let b = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if b.len() > MAX_ENVELOPE {
            return Err(format!("Nachricht zu groß ({} > {} Bytes)", b.len(), MAX_ENVELOPE));
        }
        Ok(b)
    }
    /// Deserialisieren mit Größen- und Versionsprüfung.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        if b.len() > MAX_ENVELOPE {
            return Err("Nachricht zu groß".into());
        }
        let e: Envelope = serde_json::from_slice(b).map_err(|e| format!("Ungültiges Format: {e}"))?;
        if e.v != PROTOCOL {
            return Err(format!("Protokollversion {} nicht unterstützt", e.v));
        }
        Ok(e)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Payload {
    /// Additiver A6-Handshake; fehlend/ungültig bleibt unbestätigt.
    Geraeteausweis { ausweis: crate::ausweis::Ausweis },
    /// A9 Eingabe; identische Consent-Prüfung wie RemoteInput (v2 bleibt gültig).
    Steuern { req: String, text: String },
    /// Einladung in einen Chat (ohne Verlauf). Annahme nur nach Bestätigung durch die Person.
    Invite { chat: Chat, space: Option<Space>, note: String },
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
    Step { space: crate::model::SpaceId, step: usize },

    // ── Fernzugriff Seminarleitung → Teilnehmenden-Session ──
    /// Leitung bittet um Einsicht/Mitarbeit; Teilnehmende entscheiden.
    RemoteRequest { req: String, space: crate::model::SpaceId, scope: RemoteScope, minutes: u32, reason: String },
    /// Freigabe (ggf. mit geringerem Umfang/kürzerer Dauer als erbeten).
    RemoteGrant { req: String, scope: RemoteScope, expires_ms: u64, session: SessionRef },
    /// Ablehnung, Widerruf oder Ablauf (beide Richtungen).
    RemoteEnd { req: String, reason: String },
    /// Gespiegelter Verlauf (Teilnehmende → Leitung).
    RemoteFrame { req: String, frame: RemoteFrame },
    /// Eingabe der Leitung (Leitung → Teilnehmende); Behandlung je nach Umfang.
    RemoteInput { req: String, text: String },
}
