//! Dauerhafte Geräteidentität (Ed25519, iroh `SecretKey`) und Sicherheitswörter.
//!
//! Speicherort: `<config>/copi/cocopi/identity.key` (32 Byte roh, Modus 0600).
//! Keine Passphrase/Keychain in dieser Stufe – dokumentiertes Restrisiko
//! (siehe `docs/COCOPI_SECURITY.md`); die Datei liegt nur lokal.
use crate::model::PeerId;

pub struct Identity {
    pub secret: iroh::SecretKey,
}

impl Identity {
    /// Laden oder neu erzeugen. Prüft Dateirechte (Unix: Gruppe/Andere ohne Rechte, sonst Fehler).
    pub fn load_or_create(path: &std::path::Path) -> Result<Identity, String> {
        let _ = path;
        Err("noch nicht implementiert".into())
    }
    /// Nur für Tests/Demo: flüchtig.
    pub fn ephemeral() -> Identity {
        let mut b = [0u8; 32];
        getrandom::fill(&mut b).expect("Zufall");
        Identity { secret: iroh::SecretKey::from_bytes(&b) }
    }
    pub fn peer(&self) -> PeerId {
        peer_of(&self.secret.public())
    }
}

pub fn peer_of(pk: &iroh::PublicKey) -> PeerId {
    PeerId(pk.to_string())
}

/// Sechs Sicherheitswörter zum Vergleich zweier Schlüssel (beide Seiten berechnen
/// dieselben Wörter aus beiden IDs, unabhängig von der Reihenfolge).
pub fn safety_words(a: &PeerId, b: &PeerId) -> Vec<&'static str> {
    let _ = (a, b);
    Vec::new()
}

/// Kurzform einer ID für die Anzeige (erste 8 Hex-Zeichen, in Vierergruppen).
pub fn short(p: &PeerId) -> String {
    let s = p.as_str();
    let s = &s[..s.len().min(8)];
    if s.len() > 4 { format!("{}·{}", &s[..4], &s[4..]) } else { s.to_string() }
}
