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
/// Kanonisches Empfangsformat `{...inhalt,signatur}`; alte Hüllen bleiben lesbar.
/// Zusätzliche unsignierte Hüllfelder werden abgewiesen.
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
    let value = if object.contains_key("inhalt") {
        if object.len() != 1 {
            return Err("Die Ausweishülle enthält unsignierte Angaben.".into());
        }
        object.remove("inhalt").ok_or("Der Ausweisinhalt fehlt.")?
    } else {
        Value::Object(object)
    };
    if !value.is_object() {
        return Err("Der Ausweisinhalt ist kein Objekt.".into());
    }
    VerifyingKey::from_bytes(&key)
        .map_err(|_| "Ungültiger Empfangsschlüssel.")?
        .verify_strict(
            canonical_json(&value)?.as_bytes(),
            &Signature::from_bytes(&sig),
        )
        .map_err(|_| "Der Ausweis wurde verändert oder gehört zu einem anderen Empfang.")?;
    Ok(value)
}

#[derive(Debug)]
enum RequestError {
    Verbindung(String),
    Dauerhaft(String),
}
fn request_error(e: ureq::Error, message: &str) -> RequestError {
    if matches!(
        e,
        ureq::Error::Io(_)
            | ureq::Error::Timeout(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
    ) {
        RequestError::Verbindung(message.into())
    } else {
        RequestError::Dauerhaft(message.into())
    }
}
impl From<&str> for RequestError {
    fn from(value: &str) -> Self {
        Self::Dauerhaft(value.into())
    }
}
impl From<String> for RequestError {
    fn from(value: String) -> Self {
        Self::Dauerhaft(value)
    }
}
fn retry_idempotent<T>(
    allowed: bool,
    mut call: impl FnMut() -> Result<T, RequestError>,
    mut wait: impl FnMut(std::time::Duration),
) -> Result<T, String> {
    let mut delays = [1, 3, 9].into_iter();
    loop {
        match call() {
            Ok(value) => return Ok(value),
            Err(RequestError::Verbindung(message)) if allowed => {
                if let Some(seconds) = delays.next() {
                    wait(std::time::Duration::from_secs(seconds));
                } else {
                    return Err(message);
                }
            }
            Err(RequestError::Verbindung(message) | RequestError::Dauerhaft(message)) => {
                return Err(message);
            }
        }
    }
}

pub struct Empfang {
    origin: String,
    agent: ureq::Agent,
    auth: Option<(String, ed25519_dalek::SigningKey)>,
}
impl Empfang {
    pub fn new(origin: &str) -> Result<Self, String> {
        let origin = origin.trim_end_matches('/');
        validate_url(origin)?;
        Ok(Self {
            origin: origin.to_string(),
            agent: agent(false),
            auth: None,
        })
    }
    fn request(&self, method: &str, path: &str, body: Value) -> Result<Value, String> {
        self.request_with_wait(method, path, body, std::thread::sleep)
    }
    fn request_with_wait(
        &self,
        method: &str,
        path: &str,
        body: Value,
        wait: impl FnMut(std::time::Duration),
    ) -> Result<Value, String> {
        // Kein erneuter Check-in/Tokenverbrauch. erhalten ist ohne Empfangs-Tombstone
        // NICHT sicher wiederholbar. checkout liefert serverseitig denselben Auftrag.
        let allowed = matches!(
            (method, path),
            ("POST", "hallo" | "checkout") | ("GET", "status")
        ) || (method == "GET" && path.starts_with("warten/"));
        retry_idempotent(
            allowed,
            || self.request_once(method, path, body.clone()),
            wait,
        )
    }
    fn request_once(
        &self,
        method: &str,
        path: &str,
        mut body: Value,
    ) -> Result<Value, RequestError> {
        let url = format!("{}/v1/empfang/{path}", self.origin);
        let auth_headers =
            if !matches!(path, "hallo" | "checkin" | "challenge") && !path.starts_with("warten/") {
                if let Some((id, key)) = &self.auth {
                    let c = self.request_once("POST", "challenge", json!({"checkin_id":id}))?;
                    let cid = c["challenge_id"]
                        .as_str()
                        .ok_or("Die Anmeldeprüfung fehlt.")?;
                    let nonce = c["nonce"].as_str().ok_or("Die Anmeldeprüfung fehlt.")?;
                    let payload = super::login_payload(&self.origin, "empfang", id, cid, nonce)?
                        .replacen("cocopi-v1-anmelden", "cocopi-v1-empfang", 1)
                        .replacen("\nempfang\n", "\n", 1);
                    use ed25519_dalek::Signer;
                    Some((
                        id.clone(),
                        cid.to_string(),
                        B64.encode(key.sign(payload.as_bytes()).to_bytes()),
                    ))
                } else {
                    None
                }
            } else {
                None
            };
        let mut response = if method == "GET" {
            let mut req = self.agent.get(&url);
            if let Some((id, cid, sig)) = &auth_headers {
                req = req
                    .header("X-Cocopi-Checkin", id)
                    .header("X-Cocopi-Challenge", cid)
                    .header("X-Cocopi-Signatur", sig);
            }
            req.call()
        } else {
            body["protokoll"] = json!(1);
            let mut req = self
                .agent
                .post(&url)
                .header("Content-Type", "application/json");
            if let Some((id, cid, sig)) = &auth_headers {
                req = req
                    .header("X-Cocopi-Checkin", id)
                    .header("X-Cocopi-Challenge", cid)
                    .header("X-Cocopi-Signatur", sig);
            }
            req.send(serde_json::to_vec(&body).map_err(|_| "Die Anfrage ist ungültig.")?)
        }
        .map_err(|e| request_error(e, "Der Empfang ist nicht erreichbar. Prüfe die Verbindung."))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .with_config()
            .limit(2 * 1024 * 1024)
            .read_to_string()
            .map_err(|e| {
                request_error(
                    e,
                    "Die Antwort des Empfangs ist zu groß oder unvollständig.",
                )
            })?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|_| "Der Empfang hat keine gültige Antwort gesendet.")?;
        check_packet(&value)?;
        if !(200..300).contains(&status) || value["typ"] == "fehler" {
            return Err(RequestError::Dauerhaft(packet_error(&value)));
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
    /// Gerätegebundene Empfangsanfragen nutzen je eine frische Challenge, keinen Bearer.
    /// Nach geprüftem Check-in automatisch durch `uebernehmen` eingerichtet.
    pub fn anmelden(&mut self, checkin_id: &str, key: &DeviceKey) -> Result<(), String> {
        self.auth = Some((identifier(checkin_id)?.to_string(), key.signing.clone()));
        Ok(())
    }
    /// Erst nach Bestätigung des Empfangsschlüssels durch die Person aufrufen.
    /// Prüft Belegung/Ausweis gegen denselben Pin und bindet beide an das lokale Gerät.
    pub fn uebernehmen(
        &mut self,
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
            || belegung["gueltig_bis"]
                .as_u64()
                .is_none_or(|t| t <= now_ms / 1000)
            || belegung["epoche"].as_u64().is_none()
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
        let slot = result["slot"].as_str().ok_or("Der Arbeitsplatz fehlt.")?;
        if !matches!(
            belegung["rolle"].as_str(),
            Some("seminarleitung" | "teilnehmende" | "gast")
        ) || result["hub_pfad"] != format!("/slot/{slot}")
            || super::HubV1::new(&format!("{}/slot/{slot}", self.origin)).is_err()
            || belegung["gueltig_bis"] != result["gueltig_bis"]
            || ausweis["exp"]
                .as_u64()
                .zip(ausweis["iat"].as_u64())
                .is_none_or(|(exp, iat)| exp <= iat || exp - iat > 43200)
        {
            return Err("Der Check-in enthält ungültige Kontoangaben.".into());
        }
        let device = belegung["geraete"]
            .as_array()
            .and_then(|a| a.iter().find(|d| d["pubkey"] == key.public_key()))
            .ok_or("Dieses Gerät steht nicht in der Belegung.")?;
        key.remember(identifier(
            device["geraet_id"].as_str().ok_or("Die Geräte-ID fehlt.")?,
        )?)?;
        self.anmelden(
            result["checkin_id"]
                .as_str()
                .ok_or("Die Check-in-ID fehlt.")?,
            key,
        )
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
    fn retry_only_connections_and_idempotent_calls_with_n2_backoff() {
        use std::cell::Cell;
        let calls = Cell::new(0);
        let mut delays = Vec::new();
        let result: Result<(), _> = retry_idempotent(
            true,
            || {
                calls.set(calls.get() + 1);
                Err(RequestError::Verbindung("offline".into()))
            },
            |d| delays.push(d.as_secs()),
        );
        assert_eq!(result.unwrap_err(), "offline");
        assert_eq!(calls.get(), 4); // Erstanfrage plus höchstens drei Wiederholungen.
        assert_eq!(delays, [1, 3, 9]);
        calls.set(0);
        let result: Result<(), _> = retry_idempotent(
            false,
            || {
                calls.set(calls.get() + 1);
                Err(RequestError::Verbindung("unklar".into()))
            },
            |_| panic!("Nicht idempotent"),
        );
        assert!(result.is_err());
        assert_eq!(calls.get(), 1);
        let result: Result<(), _> = retry_idempotent(
            true,
            || Err(RequestError::Dauerhaft("abgelehnt".into())),
            |_| panic!("Kein Transportfehler"),
        );
        assert!(result.is_err());
        for err in [
            ureq::Error::BodyExceedsLimit(1),
            ureq::Error::TooManyRedirects,
            ureq::Error::Tls("Test"),
        ] {
            assert!(matches!(
                request_error(err, "dauerhaft"),
                RequestError::Dauerhaft(_)
            ));
        }
        assert!(matches!(
            request_error(
                ureq::Error::Io(std::io::ErrorKind::UnexpectedEof.into()),
                "verloren"
            ),
            RequestError::Verbindung(_)
        ));
    }
    #[test]
    fn dropped_hallo_response_retries_but_checkin_never_does() {
        use std::io::Write;
        for route in ["hallo", "checkin", "checkout/id/erhalten"] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let c = Empfang::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
            let t = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut stream = std::io::BufReader::new(stream);
                read_mock_request(&mut stream);
                drop(stream); // Erstanfrage angenommen, Antwort verloren.
                if route == "hallo" {
                    let (stream, _) = listener.accept().unwrap();
                    let mut stream = std::io::BufReader::new(stream);
                    read_mock_request(&mut stream);
                    let mut stream = stream.into_inner();
                    let text = r#"{"protokoll":1,"typ":"empfang_willkommen"}"#;
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                        text.len()
                    )
                    .unwrap();
                }
            });
            let mut sleeps = Vec::new();
            let result =
                c.request_with_wait("POST", route, json!({}), |d| sleeps.push(d.as_secs()));
            assert_eq!(
                result.is_ok(),
                route == "hallo",
                "{route}: {result:?}; sleeps={sleeps:?}"
            );
            assert_eq!(sleeps, if route == "hallo" { vec![1] } else { vec![] });
            t.join().unwrap();
        }
    }
    fn read_mock_request(stream: &mut impl std::io::BufRead) {
        let mut length = 0;
        loop {
            let mut line = String::new();
            stream.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                if k.eq_ignore_ascii_case("content-length") {
                    length = v.trim().parse().unwrap();
                }
            }
        }
        let mut b = vec![0; length];
        stream.read_exact(&mut b).unwrap();
    }
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
    fn flat_empfang_matches_node_vector() {
        let example: Value = serde_json::from_str(include_str!(
            "../../../../shared/protocol/v1/examples/empfang_eingecheckt.json"
        ))
        .unwrap();
        let k = SigningKey::from_bytes(&[0; 32]);
        let pk = B64.encode(k.verifying_key().as_bytes());
        assert_eq!(
            verify_signed(&example["belegung"], &pk).unwrap()["checkin_id"],
            "test-checkin"
        );
        assert_eq!(
            verify_signed(&example["geraeteausweis"], &pk).unwrap()["rolle"],
            "teilnehmende"
        );
        let mut bad = example["belegung"].clone();
        bad["person"] = json!("unsigniert");
        assert!(verify_signed(&bad, &pk).is_err());
    }
    #[test]
    fn checkin_acceptance_binds_pin_device_role_path_and_expiry() {
        let dir = std::env::temp_dir().join(format!("cocopi-empfang-bind-{}", random_test_id()));
        let mut key = DeviceKey::at(&dir, "test").unwrap();
        let signer = SigningKey::from_bytes(&[0; 32]);
        let pk = B64.encode(signer.verifying_key().as_bytes());
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../shared/protocol/v1/examples/empfang_eingecheckt.json"
        ))
        .unwrap();
        let mut b = verify_signed(&fixture["belegung"], &pk).unwrap();
        let mut a = verify_signed(&fixture["geraeteausweis"], &pk).unwrap();
        b["geraete"][0]["pubkey"] = json!(key.public_key());
        a["geraet_pubkey"] = json!(key.public_key());
        let sign = |v: &Value| {
            let mut v = v.clone();
            v["signatur"] = json!(
                B64.encode(
                    signer
                        .sign(canonical_json(&v).unwrap().as_bytes())
                        .to_bytes()
                )
            );
            v
        };
        let mut packet = fixture.clone();
        packet["belegung"] = sign(&b);
        packet["geraeteausweis"] = sign(&a);
        let mut client = Empfang::new("https://example.test").unwrap();
        let now = 1899956800000;
        assert!(
            client
                .uebernehmen(
                    &packet,
                    &mut key,
                    &B64.encode(SigningKey::from_bytes(&[1; 32]).verifying_key().as_bytes()),
                    now
                )
                .is_err()
        );
        let mut bad = packet.clone();
        bad["hub_pfad"] = json!("/slot/account02");
        assert!(client.uebernehmen(&bad, &mut key, &pk, now).is_err());
        let mut bad = packet.clone();
        bad["rolle"] = json!("seminarleitung");
        assert!(client.uebernehmen(&bad, &mut key, &pk, now).is_err());
        assert!(
            client
                .uebernehmen(&packet, &mut key, &pk, 1900000000001)
                .is_err()
        );
        let mut bad_b = b.clone();
        bad_b["geraete"][0]["pubkey"] = json!(pk);
        let mut bad = packet.clone();
        bad["belegung"] = sign(&bad_b);
        assert!(client.uebernehmen(&bad, &mut key, &pk, now).is_err());
        client.uebernehmen(&packet, &mut key, &pk, now).unwrap();
        assert_eq!(key.geraet_id.as_deref(), Some("test-geraet"));
        assert!(client.auth.is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn random_test_id() -> String {
        let mut b = [0; 8];
        super::super::secure_random(&mut b).unwrap();
        B64.encode(b)
    }
    #[test]
    fn authenticated_request_uses_empfang_challenge_headers() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let expected_origin = origin.clone();
        let key = SigningKey::from_bytes(&[4; 32]);
        let verifying = key.verifying_key();
        let thread = std::thread::spawn(move || {
            for i in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut buf = [0; 8192];
                let n = stream.read(&mut buf).unwrap();
                let req = String::from_utf8_lossy(&buf[..n]);
                let text = if i == 0 {
                    assert!(req.starts_with("POST /v1/empfang/challenge "));
                    json!({"protokoll":1,"typ":"challenge","challenge_id":"test-cid","nonce":"test-nonce","gueltig_bis":1900000000000u64}).to_string()
                } else {
                    assert!(req.starts_with("GET /v1/empfang/status "));
                    let headers: std::collections::BTreeMap<_, _> = req
                        .lines()
                        .skip(1)
                        .filter_map(|l| {
                            l.split_once(':')
                                .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
                        })
                        .collect();
                    assert_eq!(headers["x-cocopi-checkin"], "test-checkin");
                    assert_eq!(headers["x-cocopi-challenge"], "test-cid");
                    let sig: [u8; 64] = B64
                        .decode(&headers["x-cocopi-signatur"])
                        .unwrap()
                        .try_into()
                        .unwrap();
                    verifying.verify_strict(format!("cocopi-v1-empfang\n{expected_origin}\ntest-checkin\ntest-cid\ntest-nonce").as_bytes(),&Signature::from_bytes(&sig)).unwrap();
                    json!({"protokoll":1,"typ":"empfang_status","belegung":[]}).to_string()
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                    text.len()
                )
                .unwrap();
            }
        });
        let c = Empfang {
            origin,
            agent: agent(false),
            auth: Some(("test-checkin".into(), key)),
        };
        assert_eq!(c.status().unwrap()["typ"], "empfang_status");
        thread.join().unwrap();
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
