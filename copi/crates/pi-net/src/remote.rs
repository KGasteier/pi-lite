//! Fernzugriff der Seminarleitung auf Sessions von Teilnehmenden.
//!
//! Grundsätze (siehe `docs/COCOPI_SECURITY.md`):
//! - Nur Leitung (`Role::Lead`) eines gemeinsamen Seminars darf **anfragen**.
//! - Teilnehmende geben **ausdrücklich** frei, wählen Umfang und Dauer; jederzeit beendbar.
//! - Umfang `View`: nur Spiegel des Verlaufs. `Suggest`: Leitung kann Vorschläge schicken,
//!   die als Karte erscheinen und erst nach Bestätigung in die Eingabe gehen.
//!   `Control`: Eingaben der Leitung gehen direkt an den Agenten der Teilnehmenden
//!   (Agent hat Werkzeuge → faktisch Ausführung auf deren Rechner); höchstens
//!   [`MAX_CONTROL_MINUTES`], dauerhaft sichtbares Banner, Not-Aus.
//! - Ablauf wird **lokal beim Teilnehmenden** durchgesetzt (Uhr der Leitung zählt nicht).
use serde::{Deserialize, Serialize};

use crate::model::{PeerId, SpaceId};

pub const MAX_VIEW_MINUTES: u32 = 120;
pub const MAX_CONTROL_MINUTES: u32 = 30;
/// Höchstzahl gleichzeitiger Freigaben je Gerät (Teilnehmendenseite).
pub const MAX_GRANTS: usize = 4;
/// Höchstzahl gespiegelter Einträge je Frame.
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
            RemoteScope::View => "Nur ansehen",
            RemoteScope::Suggest => "Ansehen + Vorschläge",
            RemoteScope::Control => "Mitsteuern",
        }
    }
    pub fn max_minutes(self) -> u32 {
        if self == RemoteScope::Control { MAX_CONTROL_MINUTES } else { MAX_VIEW_MINUTES }
    }
}

/// Welche Session freigegeben ist (nur Anzeige; die Zuordnung bleibt lokal).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRef {
    pub label: String,
    /// Arbeitsordner, gekürzt (z. B. `~/Work/x`). Leer, wenn Teilnehmende ihn verbergen.
    pub cwd: String,
}

/// Ein gespiegelter Verlaufseintrag (bereits verdichtet, keine Roh-Deltas).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum MirrorItem {
    User { text: String },
    Assistant { text: String, done: bool },
    Tool { name: String, detail: String, state: String },
    Note { text: String },
}

/// Paket gespiegelter Einträge. `reset` = Verlauf vollständig ersetzen (Erstsynchronisation).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFrame {
    pub seq: u64,
    pub reset: bool,
    pub items: Vec<MirrorItem>,
    /// Agentenzustand („bereit“, „arbeitet“ …).
    pub state: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Ich bin Leitung und sehe fremde Session.
    Viewer,
    /// Ich bin Teilnehmende und gebe frei.
    Host,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "s", rename_all = "snake_case")]
pub enum GrantState {
    /// Anfrage liegt vor (Host: wartet auf meine Entscheidung; Viewer: wartet auf Antwort).
    Requested,
    Active { expires_ms: u64 },
    Ended { reason: String },
}

/// Eine Fernzugriffs-Beziehung aus Sicht dieses Geräts.
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
    /// Viewer: gespiegelter Verlauf. Host: leer.
    pub mirror: Vec<MirrorItem>,
    pub mirror_state: String,
    pub last_seq: u64,
    /// Host: Vorschläge der Leitung, die auf Bestätigung warten (nur `Suggest`).
    pub suggestions: Vec<String>,
}

/// Verwaltung aller Freigaben dieses Geräts. Reine Zustandsmaschine (kein I/O):
/// Methoden liefern die zu sendenden Nutzlasten zurück.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct RemoteBook {
    pub grants: Vec<Grant>,
}

/// Was die Oberfläche nach einem eingehenden Fernzugriffs-Umschlag tun soll.
#[derive(Clone, Debug, PartialEq)]
pub enum RemoteEffect {
    /// Neue Anfrage an mich (Host) → Dialog zeigen.
    Asked { req: String },
    /// Freigabe angenommen (Viewer).
    Granted { req: String },
    Ended { req: String, reason: String },
    /// Neuer Spiegelinhalt (Viewer).
    Mirrored { req: String },
    /// Vorschlag der Leitung (Host, `Suggest`) → Karte zeigen.
    Suggested { req: String },
    /// Eingabe der Leitung direkt an den Agenten schicken (Host, `Control`, aktiv).
    Inject { req: String, text: String },
}

impl RemoteBook {
    /// Viewer: Anfrage stellen. Prüft Rolle (`requester_is_lead`) und Dauer.
    pub fn request(&mut self, peer: PeerId, space: SpaceId, scope: RemoteScope, minutes: u32, reason: &str, requester_is_lead: bool) -> Result<crate::wire::Payload, String> {
        let _ = (peer, space, scope, minutes, reason, requester_is_lead);
        Err("noch nicht implementiert".into())
    }
    /// Host: Anfrage annehmen (Umfang/Dauer dürfen nur verringert werden).
    pub fn accept(&mut self, req: &str, scope: RemoteScope, minutes: u32, session: SessionRef, now_ms: u64) -> Result<crate::wire::Payload, String> {
        let _ = (req, scope, minutes, session, now_ms);
        Err("noch nicht implementiert".into())
    }
    /// Beide Seiten: ablehnen/widerrufen/beenden.
    pub fn end(&mut self, req: &str, reason: &str) -> Option<crate::wire::Payload> {
        let _ = (req, reason);
        None
    }
    /// Abgelaufene Freigaben beenden; liefert (Peer, Nutzlast) für die Gegenseite.
    pub fn expire(&mut self, now_ms: u64) -> Vec<(PeerId, crate::wire::Payload)> {
        let _ = now_ms;
        Vec::new()
    }
    /// Eingehenden Fernzugriffs-Umschlag anwenden. `from` stammt aus der TLS-Identität,
    /// `lead_of` beantwortet, ob `from` Leitung im genannten Seminar ist (aus dem ChatStore).
    pub fn apply(&mut self, from: &PeerId, p: &crate::wire::Payload, now_ms: u64, lead_of: &dyn Fn(&PeerId, &SpaceId) -> bool) -> Result<Option<RemoteEffect>, String> {
        let _ = (from, p, now_ms, lead_of);
        Err("noch nicht implementiert".into())
    }
    /// Aktive Host-Freigaben (für das Spiegeln der eigenen Session).
    pub fn active_hosts(&self, now_ms: u64) -> Vec<&Grant> {
        let _ = now_ms;
        Vec::new()
    }
    pub fn get(&self, req: &str) -> Option<&Grant> {
        self.grants.iter().find(|g| g.req == req)
    }
}
