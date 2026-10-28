//! Empfang-Ausweis A6. Rollen werden ausschließlich nach Signatur-, Zeit- und
//! TLS-Peer-Bindungsprüfung freigegeben. Keine Speicherung von Freigaben/Personen.
use crate::model::PeerId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rolle {
    Seminarleitung,
    Teilnehmende,
    Gast,
}
impl Rolle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Seminarleitung => "Seminarleitung",
            Self::Teilnehmende => "Teilnehmende",
            Self::Gast => "Gast",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inhalt {
    pub v: u8,
    pub typ: String,
    pub pid: String,
    pub person: String,
    pub rolle: Rolle,
    pub net_pubkey: String,
    pub geraet_pubkey: String,
    /// Unix-Sekunden, nicht Millisekunden.
    pub iat: u64,
    pub exp: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Ausweis {
    /// N1: flache Ausgabe; die ältere verschachtelte Hülle bleibt lesbar.
    #[serde(flatten)]
    pub inhalt: Inhalt,
    pub signatur: String,
}
impl<'de> Deserialize<'de> for Ausweis {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let mut object = serde_json::Value::deserialize(d)?
            .as_object()
            .cloned()
            .ok_or_else(|| D::Error::custom("Ausweis muss ein Objekt sein"))?;
        let signatur = object
            .remove("signatur")
            .and_then(|v| v.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("Ausweissignatur fehlt"))?;
        let value = if object.contains_key("inhalt") {
            if object.len() != 1 {
                return Err(D::Error::custom("Unsignierte Angaben in Ausweishülle"));
            }
            object.remove("inhalt").expect("zuvor geprüft")
        } else {
            serde_json::Value::Object(object)
        };
        let inhalt = serde_json::from_value(value).map_err(D::Error::custom)?;
        Ok(Self { inhalt, signatur })
    }
}

/// Nicht deserialisierbar/öffentlich konstruierbar: kein Rollen-Bypass über JSON.
#[derive(Clone, Debug)]
pub struct Bestaetigt {
    inhalt: Inhalt,
}
impl Bestaetigt {
    pub fn person(&self) -> &str {
        &self.inhalt.person
    }
    pub fn rolle(&self) -> Rolle {
        self.inhalt.rolle
    }
    pub fn peer(&self) -> PeerId {
        self.inhalt.net_pubkey.as_str().into()
    }
    pub fn gueltig(&self, now_s: u64) -> bool {
        self.inhalt.iat <= now_s && now_s < self.inhalt.exp
    }
}
/// A6 enthält nur ASCII-Schlüssel, Strings und Integer. Explizit rekursiv
/// sortieren, unabhängig von serde_jsons preserve_order-Feature.
pub fn kanonisch(value: &impl Serialize) -> Result<Vec<u8>, String> {
    fn emit(v: &serde_json::Value, out: &mut String) -> Result<(), String> {
        match v {
            serde_json::Value::Object(o) => {
                out.push('{');
                let mut keys: Vec<_> = o.keys().collect();
                keys.sort();
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(k).map_err(|_| "JSON")?);
                    out.push(':');
                    emit(&o[*k], out)?;
                }
                out.push('}');
            }
            serde_json::Value::Array(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    emit(v, out)?;
                }
                out.push(']');
            }
            _ => out.push_str(&serde_json::to_string(v).map_err(|_| "JSON")?),
        }
        Ok(())
    }
    let mut out = String::new();
    emit(&serde_json::to_value(value).map_err(|_| "JSON")?, &mut out)?;
    Ok(out.into_bytes())
}
/// Base64url ohne Padding, kanonisch (inklusive unbenutzter Restbits).
/// Nur Transportkodierung; Kryptografie kommt aus iroh/Ed25519.
pub fn base64url(bytes: &[u8]) -> String {
    const ALPH: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut bits = 0u32;
    let mut n = 0;
    for &b in bytes {
        bits = (bits << 8) | u32::from(b);
        n += 8;
        while n >= 6 {
            n -= 6;
            out.push(ALPH[((bits >> n) & 63) as usize] as char);
        }
    }
    if n > 0 {
        out.push(ALPH[((bits << (6 - n)) & 63) as usize] as char);
    }
    out
}
fn decode<const N: usize>(s: &str) -> Result<[u8; N], String> {
    const ALPH: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    if s.len() != (N * 8).div_ceil(6) {
        return Err("Ungültige Schlüssellänge".into());
    }
    let mut out = Vec::new();
    let mut bits = 0u32;
    let mut n = 0;
    for b in s.bytes() {
        let v = ALPH
            .iter()
            .position(|x| *x == b)
            .ok_or("Ungültiges Base64url")?;
        bits = (bits << 6) | v as u32;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push(((bits >> n) & 255) as u8);
        }
    }
    let out: [u8; N] = out.try_into().map_err(|_| "Ungültige Länge")?;
    if base64url(&out) != s {
        return Err("Nichtkanonisches Base64url".into());
    }
    Ok(out)
}
impl Ausweis {
    /// `empfang_pubkey` muss zuvor bewusst bestätigt/gepinnt worden sein.
    /// Niemals den vom Peer mitgelieferten Schlüssel als Vertrauensanker nehmen.
    pub fn pruefen(
        &self,
        empfang_pubkey: &str,
        tls_peer: &PeerId,
        now_s: u64,
    ) -> Result<Bestaetigt, String> {
        let i = &self.inhalt;
        if i.v != 1
            || i.typ != "geraeteausweis"
            || i.pid.is_empty()
            || i.pid.len() > 256
            || i.person.trim().is_empty()
            || i.person.len() > 320
            || i.person.chars().any(char::is_control)
        {
            return Err("Ungültiger Geräteausweis".into());
        }
        if i.iat > now_s || i.exp <= now_s || i.exp <= i.iat || i.exp - i.iat > 12 * 60 * 60 {
            return Err("Geräteausweis abgelaufen oder Zeit ungültig".into());
        }
        // N4: ausschließlich 64 kleine Hexzeichen, kein Base32/Uppercase-Alias.
        if i.net_pubkey.len() != 64
            || !i
                .net_pubkey
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("Netzidentität muss aus 64 kleinen Hexzeichen bestehen".into());
        }
        let peer: iroh::PublicKey = i
            .net_pubkey
            .parse()
            .map_err(|_| "Ungültige Netzidentität")?;
        if peer.to_string() != tls_peer.as_str() || i.net_pubkey != tls_peer.as_str() {
            return Err("Geräteausweis gehört nicht zur Gegenstelle".into());
        }
        decode::<32>(&i.geraet_pubkey)?;
        let pk = iroh::PublicKey::from_bytes(&decode::<32>(empfang_pubkey)?)
            .map_err(|_| "Ungültiger Empfangsschlüssel")?;
        pk.verify(
            &kanonisch(i)?,
            &iroh::Signature::from_bytes(&decode::<64>(&self.signatur)?),
        )
        .map_err(|_| "Ausweis nicht vom bestätigten Empfang signiert")?;
        Ok(Bestaetigt { inhalt: i.clone() })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cert() -> (Ausweis, iroh::SecretKey, PeerId) {
        let issuer = iroh::SecretKey::from_bytes(&[41; 32]);
        let peer = iroh::SecretKey::from_bytes(&[42; 32]).public();
        let inhalt = Inhalt {
            v: 1,
            typ: "geraeteausweis".into(),
            pid: "p1".into(),
            person: "Aylin".into(),
            rolle: Rolle::Seminarleitung,
            net_pubkey: peer.to_string(),
            geraet_pubkey: base64url(&[43; 32]),
            iat: 100,
            exp: 43300,
        };
        let signatur = base64url(&issuer.sign(&kanonisch(&inhalt).unwrap()).to_bytes());
        (
            Ausweis { inhalt, signatur },
            issuer,
            PeerId(peer.to_string()),
        )
    }
    #[test]
    fn signature_time_binding_and_tampering() {
        let (mut c, k, p) = cert();
        let pk = base64url(k.public().as_bytes());
        assert_eq!(
            c.pruefen(&pk, &p, 100).unwrap().rolle(),
            Rolle::Seminarleitung
        );
        assert!(c.pruefen(&pk, &p, 99).is_err());
        assert!(c.pruefen(&pk, &p, 43300).is_err());
        assert!(c.pruefen(&pk, &PeerId::from("other"), 101).is_err());
        assert!(
            c.pruefen(
                &base64url(iroh::SecretKey::from_bytes(&[44; 32]).public().as_bytes()),
                &p,
                101
            )
            .is_err()
        );
        c.inhalt.person = "Mallory".into();
        assert!(c.pruefen(&pk, &p, 101).is_err());
    }
    #[test]
    fn n1_flat_output_nested_input_and_no_unsigned_fields() {
        let (c, _, _) = cert();
        let flat = serde_json::to_value(&c).unwrap();
        assert!(flat.get("inhalt").is_none());
        assert_eq!(flat["iat"], 100);
        assert_eq!(serde_json::from_value::<Ausweis>(flat.clone()).unwrap(), c);
        let nested = serde_json::json!({"inhalt":c.inhalt,"signatur":c.signatur});
        assert_eq!(
            serde_json::from_value::<Ausweis>(nested.clone()).unwrap(),
            c
        );
        for mut v in [flat, nested] {
            v["rolle_extra"] = serde_json::json!("seminarleitung");
            assert!(serde_json::from_value::<Ausweis>(v).is_err());
        }
    }
    #[test]
    fn n4_seconds_lowercase_hex_and_no_time_unit_guessing() {
        let (mut c, k, p) = cert();
        let pk = base64url(k.public().as_bytes());
        assert_eq!(p.as_str().len(), 64);
        assert!(
            p.as_str()
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        c.inhalt.iat = 1_800_000_000;
        c.inhalt.exp = c.inhalt.iat + 43_200;
        let sign = |c: &mut Ausweis| {
            c.signatur = base64url(&k.sign(&kanonisch(&c.inhalt).unwrap()).to_bytes())
        };
        sign(&mut c);
        assert!(c.pruefen(&pk, &p, 1_800_000_000).is_ok());
        assert!(c.pruefen(&pk, &p, 1_800_043_200).is_err());
        c.inhalt.iat *= 1000;
        c.inhalt.exp *= 1000;
        sign(&mut c);
        assert!(c.pruefen(&pk, &p, 1_800_000_000).is_err());
        c.inhalt.iat = 100;
        c.inhalt.exp = 200;
        c.inhalt.net_pubkey.make_ascii_uppercase();
        sign(&mut c);
        assert!(
            c.pruefen(&pk, &PeerId(c.inhalt.net_pubkey.clone()), 100)
                .is_err()
        );
    }
    #[test]
    fn recursive_canonical_json_and_encoding() {
        assert_eq!(
            kanonisch(&serde_json::json!({"z":[{"b":2,"a":"ä"}],"a":1})).unwrap(),
            "{\"a\":1,\"z\":[{\"a\":\"ä\",\"b\":2}]}".as_bytes()
        );
        assert_eq!(base64url(b"hello"), "aGVsbG8");
        assert_eq!(decode::<5>("aGVsbG8").unwrap(), *b"hello");
        assert!(decode::<5>("aGVsbG9").is_err());
    }
}
