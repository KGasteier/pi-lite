//! cocopi-Kern: frontendunabhängig (egui-App heute, Tauri-Adapter möglich).
//!
//! - [`model`]: Teilnehmende, Rollen, Räume, Chats, Nachrichten (Vertrag).
//! - [`store`]: Chat- und Rollenverwaltung + Eingangsprüfung.
//! - [`wire`]: Umschlagformat zwischen Geräten.
//! - [`identity`]/[`node`]: dauerhafte Identität und iroh-Transport.
//! - [`a2a`]/[`persona`]: Agent2Agent-Dirigent und Nebenagenten.
//! - [`remote`]: Fernzugriff Seminarleitung → Teilnehmenden-Session.
//! - [`direct`]: älterer Prototyp (flüchtige Einzelverbindung), bleibt bis zur Ablösung.
pub mod ausweis;
pub mod a2a;
pub mod direct;
pub mod identity;
pub mod model;
pub mod node;
pub mod persona;
pub mod remote;
pub mod seminar;
pub mod store;
pub mod wire;

// Kompatibilität für bestehende Aufrufer (`pi_net::Node`, `pi_net::Card` …).
pub use direct::{Card, Event, Kind, Node, Space, SpaceKind, MAX_BYTES};
