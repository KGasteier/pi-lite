//! P2P-Transport v1: iroh-QUIC direkt (keine öffentlichen Relays, keine Discovery),
//! dauerhafte Identität, Allowlist bestätigter Kontakte, ein Bi-Stream je [`Envelope`].
//!
//! Eingehend: Verbindung nur von Geräten in der Allowlist (`set_allowed`); alles andere
//! wird geschlossen und als [`NodeEvent::Rejected`] gemeldet. Pro Gegenstelle
//! Ratenbegrenzung ([`RATE_PER_MIN`]). Antwort `ok` erst nach erfolgreichem Decode.
//! Ausgehend: Warteschlange je Gerät mit Wiederholung (Backoff) bis [`MAX_QUEUE`].
use std::sync::mpsc;

use crate::identity::Identity;
use crate::model::{MsgId, PeerId};
use crate::wire::Envelope;

pub const ALPN: &[u8] = b"cocopi/1";
pub const RATE_PER_MIN: u32 = 120;
pub const MAX_QUEUE: usize = 200;

/// Zustellauftrag; `tag` erlaubt die Zuordnung der Rückmeldung (z. B. Nachrichten-ID).
#[derive(Clone, Debug)]
pub struct Outgoing {
    pub to: PeerId,
    pub env: Envelope,
    pub tag: Option<MsgId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeEvent {
    /// Eigene Kontaktadresse (JSON von `EndpointAddr`; enthält IP-Adressen).
    Address(String),
    /// Umschlag empfangen; `from` = TLS-geprüfte Gegenstelle.
    Received { from: PeerId, env: Envelope },
    Delivered { to: PeerId, tag: Option<MsgId> },
    Failed { to: PeerId, tag: Option<MsgId>, error: String },
    /// Unbekanntes/gesperrtes Gerät abgewiesen (nur Kurz-ID in der Anzeige).
    Rejected { from: PeerId, why: String },
    /// Gegenstelle erreichbar/nicht erreichbar (nach Zustellversuch).
    Reachable { peer: PeerId, ok: bool },
    Error(String),
    Stopped,
}

pub struct Node {
    pub me: PeerId,
    pub events: mpsc::Receiver<NodeEvent>,
    // Implementierung: Befehlskanal zum Tokio-Thread.
}

impl Node {
    /// Start nur nach ausdrücklichem Klick.
    pub fn start(identity: Identity) -> Node {
        let me = identity.peer();
        let (_tx, events) = mpsc::channel();
        Node { me, events }
    }
    /// Allowlist ersetzen: (Gerät, Kontaktadresse-JSON).
    pub fn set_allowed(&self, peers: Vec<(PeerId, String)>) {
        let _ = peers;
    }
    pub fn send(&self, o: Outgoing) -> Result<(), String> {
        let _ = o;
        Err("noch nicht implementiert".into())
    }
    pub fn stop(&self) {}
}

/// Kontaktadresse prüfen und die enthaltene Geräte-ID liefern.
pub fn parse_address(addr: &str) -> Result<PeerId, String> {
    let a: iroh::EndpointAddr = serde_json::from_str(addr.trim()).map_err(|e| format!("Kontaktadresse ungültig: {e}"))?;
    Ok(crate::identity::peer_of(&a.id))
}
