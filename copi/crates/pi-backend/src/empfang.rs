//! Empfangs-Client (A6). Blocking: ausschließlich in einem App-Worker verwenden.
use super::{DeviceKey, agent, check_packet, identifier, packet_error, validate_url};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::{Value, json};

/// Gemeinsame kanonische Darstellung für Empfang, Belegung und Geräteausweis.
/// Sortierung nach UTF-16 wie JavaScript; nur interoperable sichere Ganzzahlen.
pub fn canonical_json(value: &Value) -> Result<String, String> {
    Ok(match value {
        Value::Null | Value::Bool(_) | Value::String(_) => {
            serde_json::to_string(value).map_err(|_| "Ungültiges JSON.")?
        }
        Value::Number(n) => {
            let safe = n
                .as_i64()
                .is_some_and(|v| v.unsigned_abs() <= 9_007_199_254_740_991)
                || n.as_u64().is_some_and(|v| v <= 9_007_199_254_740_991);
            if !safe {
                return Err("Signierte Zahlen müssen sichere Ganzzahlen sein.".into());
            }
            n.to_string()
        }
        Value::Array(a) => format!(
            "[{}]",
            a.iter()
                .map(canonical_json)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        Value::Object(o) => {
            let mut keys: Vec<_> = o.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            format!(
                "{{{}}}",
                keys.iter()
                    .map(|k| Ok(format!(
                        "{}:{}",
                        serde_json::to_string(k).map_err(|_| "Ungültiger Schlüssel.")?,
                        canonical_json(&o[*k])?
                    )))
                    .collect::<Result<Vec<_>, String>>()?
                    .join(",")
            )
        }
    })
}

/// Prüft nur Integrität/Herkunft, NICHT Rolle, Zweck, Gültigkeitszeit oder Pin-Vertrauen.
/// Aufrufer (S8) müssen `typ`, Schlüsselbindung und `iat`/`exp` zusätzlich prüfen.
/// Flaches signiertes JSON, Feld `signatur` wird aus den signierten Nutzdaten entfernt.
pub fn verify_signed(json: &Value, pubkey: &str) -> Result<Value, String> {
    let mut object = json
        .as_object()
        .cloned()
        .ok_or("Der Ausweis ist kein JSON-Objekt.")?;
    let signature = object
        .remove("signatur")
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or("Die Signatur fehlt.")?;
    let key: [u8; 32] = B64
        .decode(pubkey)
        .map_err(|_| "Ungültiger Empfangsschlüssel.")?
        .try_into()
        .map_err(|_| "Ungültiger Empfangsschlüssel.")?;
    let sig: [u8; 64] = B64
        .decode(signature)
        .map_err(|_| "Ungültige Signatur.")?
        .try_into()
        .map_err(|_| "Ungültige Signatur.")?;
    let value = Value::Object(object);
    VerifyingKey::from_bytes(&key)
        .map_err(|_| "Ungültiger Empfangsschlüssel.")?
        .verify_strict(
            canonical_json(&value)?.as_bytes(),
            &Signature::from_bytes(&sig),
        )
        .map_err(|_| "Der Ausweis wurde verändert oder gehört zu einem anderen Empfang.")?;
    Ok(value)
}

pub struct Empfang {
    origin: String,
    agent: ureq::Agent,
    token: Option<String>,
}
impl Empfang {
    pub fn new(origin: &str) -> Result<Self, String> {
        let origin = origin.trim_end_matches('/');
        validate_url(origin)?;
        Ok(Self {
            origin: origin.to_string(),
            agent: agent(false),
            token: None,
        })
    }
    fn request(&self, method: &str, path: &str, mut body: Value) -> Result<Value, String> {
        let url = format!("{}/v1/empfang/{path}", self.origin);
        let mut response = if method == "GET" {
            let mut req = self.agent.get(&url);
            if let Some(t) = &self.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.call()
        } else {
            body["protokoll"] = json!(1);
            let mut req = self
                .agent
                .post(&url)
                .header("Content-Type", "application/json");
            if let Some(t) = &self.token {
                req = req.header("Authorization", format!("Bearer {t}"));
            }
            req.send(serde_json::to_vec(&body).map_err(|_| "Die Anfrage ist ungültig.")?)
        }
        .map_err(|_| "Der Empfang ist nicht erreichbar. Prüfe die Verbindung.")?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .with_config()
            .limit(2 * 1024 * 1024)
            .read_to_string()
            .map_err(|_| "Die Antwort des Empfangs ist zu groß oder unvollständig.")?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|_| "Der Empfang hat keine gültige Antwort gesendet.")?;
        check_packet(&value)?;
        if !(200..300).contains(&status) || value["typ"] == "fehler" {
            return Err(packet_error(&value));
        }
        Ok(value)
    }
    pub fn hallo(&self) -> Result<Value, String> {
        self.request("POST", "hallo", json!({}))
    }
    /// 201 `eingecheckt` oder 202 `warteschlange`; 202 ist ausdrücklich kein Fehler.
    /// Body enthält nachweis, geraet, optional net_pubkey und archiv_pubkey.
    pub fn checkin(&self, body: Value) -> Result<Value, String> {
        if !body.is_object() {
            return Err("Die Check-in-Anfrage ist unvollständig.".into());
        }
        let v = self.request("POST", "checkin", body)?;
        if !matches!(v["typ"].as_str(), Some("eingecheckt" | "warteschlange")) {
            return Err("Der Empfang hat den Check-in nicht bestätigt.".into());
        }
        Ok(v)
    }
    pub fn warten(&self, id: &str) -> Result<Value, String> {
        self.request("GET", &format!("warten/{}", identifier(id)?), Value::Null)
    }
    pub fn anmelden(&mut self, key: &DeviceKey) -> Result<Value, String> {
        let id = key
            .geraet_id
            .as_ref()
            .ok_or("Dieses Gerät ist noch nicht eingecheckt.")?;
        let challenge = self.request("POST", "anmelden/challenge", json!({"geraet_id":id}))?;
        let cid = challenge["challenge_id"]
            .as_str()
            .ok_or("Die Anmeldeprüfung fehlt.")?;
        let nonce = challenge["nonce"]
            .as_str()
            .ok_or("Die Anmeldeprüfung fehlt.")?;
        let payload = super::login_payload(&self.origin, "empfang", id, cid, nonce)?.replacen(
            "cocopi-v1-anmelden",
            "cocopi-v1-empfang",
            1,
        );
        let v = self.request(
            "POST",
            "anmelden",
            json!({"challenge_id":cid,"signatur":key.sign(payload.as_bytes())}),
        )?;
        self.token = Some(
            v["zugangstoken"]
                .as_str()
                .ok_or("Der Empfang hat die Anmeldung nicht bestätigt.")?
                .to_string(),
        );
        Ok(v)
    }
    /// Erst nach Bestätigung des Empfangsschlüssels durch die Person aufrufen.
    /// Prüft Belegung/Ausweis gegen denselben Pin und bindet beide an das lokale Gerät.
    pub fn uebernehmen(
        &self,
        result: &Value,
        key: &mut DeviceKey,
        pubkey: &str,
        now_ms: u64,
    ) -> Result<(), String> {
        if result["typ"] != "eingecheckt" {
            return Err("Der Check-in ist noch nicht abgeschlossen.".into());
        }
        let belegung = verify_signed(&result["belegung"], pubkey)?;
        let ausweis = verify_signed(&result["geraeteausweis"], pubkey)?;
        if belegung["v"] != 1
            || belegung["typ"] != "belegung"
            || belegung["slot"] != result["slot"]
            || belegung["checkin_id"] != result["checkin_id"]
            || belegung["person"] != result["person"]
            || belegung["rolle"] != result["rolle"]
            || belegung["gueltig_bis"].as_u64().is_none_or(|t| t <= now_ms)
            || ausweis["v"] != 1
            || ausweis["typ"] != "geraeteausweis"
            || ausweis["geraet_pubkey"] != key.public_key()
            || ausweis["person"] != belegung["person"]
            || ausweis["rolle"] != belegung["rolle"]
            || ausweis["exp"].as_u64().is_none_or(|t| t <= now_ms / 1000)
            || ausweis["iat"]
                .as_u64()
                .is_none_or(|t| t > now_ms / 1000 + 60)
        {
            return Err(
                "Die Belegung und der Geräteausweis passen nicht zu diesem Check-in.".into(),
            );
        }
        let device = belegung["geraete"]
            .as_array()
            .and_then(|a| a.iter().find(|d| d["pubkey"] == key.public_key()))
            .ok_or("Dieses Gerät steht nicht in der Belegung.")?;
        key.remember(identifier(
            device["geraet_id"].as_str().ok_or("Die Geräte-ID fehlt.")?,
        )?)
    }
    pub fn checkout(&self, checkin_id: &str) -> Result<Value, String> {
        self.request(
            "POST",
            "checkout",
            json!({"checkin_id":identifier(checkin_id)?}),
        )
    }
    pub fn erhalten(&self, checkout_id: &str, sha256: &str) -> Result<Value, String> {
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Die Archiv-Prüfsumme ist ungültig.".into());
        }
        self.request(
            "POST",
            &format!("checkout/{}/erhalten", identifier(checkout_id)?),
            json!({"sha256":sha256}),
        )
    }
    pub fn status(&self) -> Result<Value, String> {
        self.request("GET", "status", Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn canonical_signature_tamper_and_unsafe_numbers() {
        let k = SigningKey::from_bytes(&[0; 32]);
        let mut v = json!({"z":[{"b":2,"a":"ä"}],"a":1});
        assert_eq!(
            canonical_json(&v).unwrap(),
            "{\"a\":1,\"z\":[{\"a\":\"ä\",\"b\":2}]}"
        );
        let sig = k.sign(canonical_json(&v).unwrap().as_bytes());
        v["signatur"] = json!(B64.encode(sig.to_bytes()));
        let pk = B64.encode(k.verifying_key().as_bytes());
        assert_eq!(verify_signed(&v, &pk).unwrap()["a"], 1);
        v["a"] = json!(2);
        assert!(verify_signed(&v, &pk).is_err());
        assert!(canonical_json(&json!(1.2)).is_err());
        assert!(canonical_json(&json!(9007199254740992u64)).is_err());
        assert_eq!(
            canonical_json(&json!({"\u{e000}":1,"\u{10000}":2})).unwrap(),
            "{\"\u{10000}\":2,\"\u{e000}\":1}"
        );
    }
    #[test]
    fn client_local_mock_201_202_checkout_and_limits() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            for (route, status, packet) in [
                (
                    "hallo",
                    200,
                    json!({"protokoll":1,"typ":"empfang_willkommen"}),
                ),
                ("checkin", 201, json!({"protokoll":1,"typ":"eingecheckt"})),
                (
                    "checkin",
                    202,
                    json!({"protokoll":1,"typ":"warteschlange","platz":2}),
                ),
                (
                    "warten/warte-1",
                    200,
                    json!({"protokoll":1,"typ":"warteschlange","platz":1}),
                ),
                (
                    "checkout",
                    200,
                    json!({"protokoll":1,"typ":"checkout_bereit"}),
                ),
                (
                    "checkout/out-1/erhalten",
                    200,
                    json!({"protokoll":1,"typ":"ausgecheckt"}),
                ),
                (
                    "status",
                    403,
                    json!({"protokoll":1,"typ":"fehler","code":"keine_berechtigung"}),
                ),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut buf = [0; 8192];
                let n = stream.read(&mut buf).unwrap();
                assert!(
                    String::from_utf8_lossy(&buf[..n]).contains(&format!("/v1/empfang/{route} "))
                );
                let text = packet.to_string();
                write!(stream, "HTTP/1.1 {status} Mock\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).unwrap();
            }
        });
        let c = Empfang::new(&origin).unwrap();
        assert!(c.agent.config().proxy().is_none());
        assert_eq!(c.hallo().unwrap()["typ"], "empfang_willkommen");
        assert_eq!(c.checkin(json!({})).unwrap()["typ"], "eingecheckt");
        assert_eq!(c.checkin(json!({})).unwrap()["platz"], 2);
        assert_eq!(c.warten("warte-1").unwrap()["platz"], 1);
        assert_eq!(c.checkout("checkin-1").unwrap()["typ"], "checkout_bereit");
        assert_eq!(
            c.erhalten("out-1", &"a".repeat(64)).unwrap()["typ"],
            "ausgecheckt"
        );
        assert!(c.status().unwrap_err().starts_with("[keine_berechtigung]"));
        assert!(c.warten("../status").is_err());
        thread.join().unwrap();
    }
}
