//! Agent2Agent-Modus: mehrere Agenten (laufende Entwicklungs-Sessions und Nebenagenten
//! mit Persönlichkeit) sprechen in einem Chat miteinander – mit Leitplanken.
//!
//! Reine Steuerlogik ([`Conductor`]); das tatsächliche Senden an pi-Instanzen und das
//! Einsammeln fertiger Antworten macht die App (`app/src/a2a_run.rs`).
//!
//! Leitplanken:
//! - Nur Agenten, die **diesem Gerät gehören**, werden von hier aus angesprochen. Fremde
//!   Agenten antworten über ihr eigenes Gerät (dort gelten deren Regeln).
//! - Rundenbudget (`max_turns`), danach Pause bis ein Mensch fortsetzt.
//! - Optional „Mensch bestätigt jeden Zug“ (`approve_each`).
//! - Nachrichten an Agenten werden als **zitierte Daten** mit Absender eingerahmt
//!   ([`frame_for_agent`]), nie als Systemanweisung.
use serde::{Deserialize, Serialize};

use crate::model::{Chat, Message, Participant, PeerId};

/// Reihenfolge, in der Agenten an die Reihe kommen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TurnOrder {
    /// Reihum in Mitgliederreihenfolge.
    #[default]
    RoundRobin,
    /// Nur wer mit `@Name` angesprochen wird.
    Mention,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A2aPolicy {
    pub enabled: bool,
    pub order: TurnOrder,
    /// Höchstzahl automatischer Agentenzüge, bevor ein Mensch fortsetzen muss.
    pub max_turns: u32,
    /// Bereits verbrauchte Züge seit der letzten menschlichen Nachricht/Freigabe.
    pub turns_used: u32,
    /// Jeder Agentenzug braucht eine Bestätigung.
    pub approve_each: bool,
    /// Pausiert (Not-Aus oder Budget erschöpft).
    pub paused: bool,
}

impl Default for A2aPolicy {
    fn default() -> Self {
        Self { enabled: false, order: TurnOrder::RoundRobin, max_turns: 6, turns_used: 0, approve_each: false, paused: false }
    }
}

/// Höchstwert für `max_turns` (Kosten-/Schleifenschutz).
pub const MAX_TURNS_LIMIT: u32 = 50;
/// Höchstlänge des Kontexts, der einem Agenten pro Zug übergeben wird (Zeichen).
pub const MAX_CONTEXT_CHARS: usize = 12_000;

/// Nächster Schritt laut Dirigent.
#[derive(Clone, Debug, PartialEq)]
pub enum Next {
    /// Nichts zu tun (A2A aus, pausiert, letzter Beitrag nicht relevant …).
    Idle,
    /// Diesen eigenen Agenten mit diesem Text ansprechen.
    Speak { agent: Participant, prompt: String },
    /// Wartet auf menschliche Bestätigung für diesen Zug.
    NeedsApproval { agent: Participant, prompt: String },
    /// Budget erschöpft → pausiert.
    BudgetExhausted,
}

/// Dirigent für einen Chat.
pub struct Conductor;

impl Conductor {
    /// Entscheidet nach einer neuen Nachricht, ob und welcher **eigene** Agent als
    /// Nächstes spricht. `me` = dieses Gerät, `busy` = Agenten, die gerade arbeiten.
    pub fn next(chat: &Chat, me: &PeerId, busy: &[Participant]) -> Next {
        let _ = (chat, me, busy);
        Next::Idle
    }
    /// Zug wurde ausgeführt → Budget zählen.
    pub fn record_turn(policy: &mut A2aPolicy) {
        let _ = policy;
    }
    /// Mensch hat geschrieben oder „Fortsetzen“ gedrückt → Budget zurücksetzen.
    pub fn human_resume(policy: &mut A2aPolicy) {
        let _ = policy;
    }
}

/// Rahmt den Chatverlauf seit dem letzten eigenen Beitrag des Agenten als Daten ein.
pub fn frame_for_agent(chat: &Chat, agent: &Participant, names: &dyn Fn(&Participant) -> String) -> String {
    let _ = (chat, agent, names);
    String::new()
}

/// Entfernt Steuerzeichen/Bidi und kürzt; für alles, was Agenten erreicht.
pub fn sanitize(text: &str, max: usize) -> String {
    let t: String = text
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .filter(|c| !matches!(*c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'))
        .collect();
    if t.len() <= max {
        return t;
    }
    let mut end = max;
    while !t.is_char_boundary(end) {
        end -= 1;
    }
    t[..end].to_string()
}

/// Hilfsfunktion für die App: ist diese Nachricht von einem Agenten?
pub fn by_agent(m: &Message) -> bool {
    m.author.is_agent()
}
