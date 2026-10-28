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

/// Suchmetadaten, kein Beweis einer Peer-Rolle (dafür den Geräteausweis prüfen).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerzeichnisPerson {
    pub person: String,
    pub anzeige: String,
    pub rolle: String,
    pub net_pubkey: String,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verzeichnis {
    /// Unix-Sekunden, wie Ausweiszeiten (N4).
    pub ausgestellt: u64,
    pub personen: Vec<VerzeichnisPerson>,
}

pub struct Empfang {
    origin: String,
    agent: ureq::Agent,
    auth: Option<(String, ed25519_dalek::SigningKey)>,
    empfang_pubkey: Option<String>,
}
impl Empfang {
    pub fn new(origin: &str) -> Result<Self, String> {
        let origin = origin.trim_end_matches('/');
        validate_url(origin)?;
        Ok(Self {
            origin: origin.to_string(),
            agent: agent(false),
            auth: None,
            empfang_pubkey: None,
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
        let auth_headers = if !matches!(path, "hallo" | "checkin" | "challenge" | "nonce")
            && !path.starts_with("warten/")
        {
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
            .limit(if method == "GET" && path.starts_with("abholfach/") {
                // Empfang liefert das Chiffrat base64url im JSON (bis 2 GiB Archiv).
                // Nur die einzelne, ausdrücklich abgeholte Box darf so groß sein.
                (2 * 1024 * 1024 * 1024u64).div_ceil(3) * 4 + 65536
            } else {
                2 * 1024 * 1024
            })
            .read_to_string()
            .map_err(|e| {
                request_error(
                    e,
                    "Die Antwort des Empfangs ist zu groß oder unvollständig.",
                )
            })?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|_| "Der Empfang hat keine gültige Antwort gesendet.")?;
        // N9 ist eine signierte Protokollhülle ohne typ; Integritätsprüfung
        // erfolgt in seminar_verzeichnis, bevor irgendetwas daraus verwendet wird.
        if method == "GET"
            && path == "seminar"
            && (200..300).contains(&status)
            && value["typ"] != "fehler"
        {
            if value["protokoll"] != 1 {
                return Err(
                    "Das Seminarverzeichnis verwendet eine andere Protokollversion.".into(),
                );
            }
        } else {
            check_packet(&value)?;
        }
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
        // Ein neuer Auth-Kontext übernimmt keinen Pin einer früheren Belegung.
        self.empfang_pubkey = None;
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
        )?;
        self.empfang_pubkey = Some(pubkey.to_owned());
        Ok(())
    }
    /// Server-nonce für genau diesen Ed25519-Geräteschlüssel; vor Browserstart
    /// abrufen, danach an `oidc::Login::begin_with_nonce` weitergeben. Kein Retry.
    pub fn nonce(&self, geraet_pubkey: &str) -> Result<String, String> {
        let bytes: [u8; 32] = B64
            .decode(geraet_pubkey)
            .map_err(|_| "Der öffentliche Geräteschlüssel ist ungültig.")?
            .try_into()
            .map_err(|_| "Der öffentliche Geräteschlüssel ist ungültig.")?;
        VerifyingKey::from_bytes(&bytes)
            .map_err(|_| "Der öffentliche Geräteschlüssel ist ungültig.")?;
        let v = self.request("POST", "nonce", json!({"geraet_pubkey":geraet_pubkey}))?;
        let nonce = v["nonce"]
            .as_str()
            .ok_or("Der Empfang hat keine Anmeldeprüfung gesendet.")?;
        super::oidc::validate_nonce(nonce)?;
        Ok(nonce.to_owned())
    }
    /// Alle ≤ 60 s aus dem Worker aufrufen, solange Desktop ODER Handy am Hub
    /// verbunden sind; beim sauberen Trennen false. Keine automatische Wiederholung.
    pub fn aktiv(&self, verbunden: bool) -> Result<(), String> {
        self.require_auth()?;
        let v = self.request("POST", "aktiv", json!({"verbunden":verbunden}))?;
        if v["typ"] != "aktiv_bestaetigt" {
            return Err("Der Empfang hat die Verbindung nicht bestätigt.".into());
        }
        Ok(())
    }
    fn require_auth(&self) -> Result<(), String> {
        if self.auth.is_none() {
            return Err("Bitte checke zuerst am Empfang ein.".into());
        }
        Ok(())
    }
    /// Verzeichnis nur nach `uebernehmen` (bestätigter Pin). Signatur über das
    /// gesamte flache N9-Paket ohne signatur, einschließlich protokoll.
    /// Die Rollen sind Suchmetadaten: niemals Mitsteuerrechte daraus ableiten.
    pub fn seminar_verzeichnis(&self) -> Result<Verzeichnis, String> {
        self.require_auth()?;
        let pin = self
            .empfang_pubkey
            .as_deref()
            .ok_or("Bestätige zuerst den Empfangsschlüssel beim Check-in.")?;
        let packet = self.request("GET", "seminar", Value::Null)?;
        Self::pruefen_verzeichnis(&packet, pin)
    }
    fn pruefen_verzeichnis(packet: &Value, pin: &str) -> Result<Verzeichnis, String> {
        let signed = verify_signed(packet, pin)?;
        if signed["protokoll"] != 1 {
            return Err("Das Seminarverzeichnis verwendet eine andere Protokollversion.".into());
        }
        let v: Verzeichnis = serde_json::from_value(signed["verzeichnis"].clone())
            .map_err(|_| "Das Seminarverzeichnis ist unvollständig.")?;
        if v.personen.len() > 1000
            || v.ausgestellt == 0
            || v.personen.iter().any(|p| {
                p.person.is_empty()
                    || p.person.len() > 256
                    || p.person.chars().any(char::is_control)
                    || p.anzeige.trim().is_empty()
                    || p.anzeige.len() > 256
                    || p.anzeige.chars().any(char::is_control)
                    || !matches!(p.rolle.as_str(), "seminarleitung" | "teilnehmende" | "gast")
                    || p.net_pubkey.len() != 64
                    || !p
                        .net_pubkey
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err("Das Seminarverzeichnis enthält ungültige Kontakte.".into());
        }
        Ok(v)
    }
    pub fn checkout(&self, checkin_id: &str) -> Result<Value, String> {
        self.request(
            "POST",
            "checkout",
            json!({"checkin_id":identifier(checkin_id)?}),
        )
    }
    /// Wie checkout, mit bewusster Auswahl für den Dienstarchiv-Auftrag.
    /// Antwort: checkout_bereit oder direkt ausgecheckt bei nachgewiesen leerem Slot.
    pub fn checkout_mit_auswahl(&self, checkin_id: &str, auswahl: Value) -> Result<Value, String> {
        if !auswahl.is_object() {
            return Err("Die Dateiauswahl ist ungültig.".into());
        }
        self.request(
            "POST",
            "checkout",
            json!({"checkin_id":identifier(checkin_id)?,"auswahl":auswahl}),
        )
    }
    /// GET abholfach → abholfaecher {archive:[{checkin_id,ablauf,sha256}]}.
    /// Zugriff wie Checkout: aktuelle Geräte-Challenge, nur eigene Person.
    pub fn abholfach(&self) -> Result<Value, String> {
        self.request("GET", "abholfach", Value::Null)
    }
    /// GET abholfach/:checkin_id → abholfach {archiv: verschlüsselter Umschlag}.
    /// Keine Bestätigung vor Entschlüsselung, Hashprüfung und lokalem Rückspielen.
    pub fn abholfach_holen(&self, checkin_id: &str) -> Result<Value, String> {
        self.request(
            "GET",
            &format!("abholfach/{}", box_id(checkin_id)?),
            Value::Null,
        )
    }
    /// POST abholfach/:checkin_id/erhalten {sha256} → abholfach_erhalten.
    /// Löscht das Abholfach! Nicht automatisch wiederholt.
    pub fn abholfach_erhalten(&self, checkin_id: &str, sha256: &str) -> Result<Value, String> {
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Die Archiv-Prüfsumme ist ungültig.".into());
        }
        self.request(
            "POST",
            &format!("abholfach/{}/erhalten", box_id(checkin_id)?),
            json!({"sha256":sha256}),
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

// Genau die ID-Grammatik der Abholfach-Routen im Empfang (kein neuer Vertrag).
fn box_id(id: &str) -> Result<&str, String> {
    if id.len() != 36
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-')
    {
        return Err("Die Abholfach-ID ist ungültig.".into());
    }
    Ok(id)
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
    fn read_mock_request(stream: &mut impl std::io::BufRead) -> String {
        let mut request = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            assert_ne!(
                stream.read_line(&mut line).unwrap(),
                0,
                "HTTP-Header unvollständig"
            );
            request.push_str(&line);
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
        request.push_str(std::str::from_utf8(&b).unwrap());
        request
    }
    #[test]
    fn mock_request_reads_fragmented_headers_and_complete_body() {
        use std::io::{BufReader, Cursor};
        let wire =
            b"POST /v1/empfang/checkin HTTP/1.1\r\nContent-Length: 15\r\n\r\n{\"protokoll\":1}";
        // Ein Byte pro Pufferfüllung erzwingt Fragmentierung auch im Body.
        let mut reader = BufReader::with_capacity(1, Cursor::new(wire));
        assert_eq!(read_mock_request(&mut reader).as_bytes(), wire);
        let mut eof = BufReader::new(Cursor::new(b"GET / HTTP/1.1\r\n"));
        assert!(std::panic::catch_unwind(move || read_mock_request(&mut eof)).is_err());
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
        assert_eq!(client.empfang_pubkey.as_deref(), Some(pk.as_str()));
        client.anmelden("another", &key).unwrap();
        assert!(client.empfang_pubkey.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn random_test_id() -> String {
        let mut b = [0; 8];
        super::super::secure_random(&mut b).unwrap();
        B64.encode(b)
    }
    #[test]
    fn authenticated_request_uses_empfang_challenge_headers() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let expected_origin = origin.clone();
        let key = SigningKey::from_bytes(&[4; 32]);
        let verifying = key.verifying_key();
        let thread = std::thread::spawn(move || {
            for i in 0..2 {
                let (stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut stream = std::io::BufReader::new(stream);
                let req = read_mock_request(&mut stream);
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
                    stream.get_mut(),
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
            empfang_pubkey: None,
        };
        assert_eq!(c.status().unwrap()["typ"], "empfang_status");
        thread.join().unwrap();
    }
    #[test]
    fn selection_and_boxes_follow_routes_with_fresh_device_challenges() {
        use std::io::{BufRead, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let expected_origin = origin.clone();
        let key = SigningKey::from_bytes(&[7; 32]);
        let verifying = key.verifying_key();
        let id = "12345678-1234-1234-1234-123456789abc";
        let selection = json!({"zusaetzlich":["Projekte/Film/gross.mov"],"ausschliessen":["Ergebnisse/privat.md"],"gross_bestaetigt":true});
        let expected_selection = selection.clone();
        let server = std::thread::spawn(move || {
            for (i, (method, route, expected, packet, status)) in [
                ("POST", "checkout".to_owned(), json!({"checkin_id":"active","auswahl":expected_selection,"protokoll":1}), json!({"protokoll":1,"typ":"checkout_bereit"}), 200),
                ("GET", "abholfach".to_owned(), Value::Null, json!({"protokoll":1,"typ":"abholfaecher","archive":[{"checkin_id":id,"ablauf":1900000000000u64,"sha256":"a".repeat(64)}]}), 200),
                ("GET", format!("abholfach/{id}"), Value::Null, json!({"protokoll":1,"typ":"abholfach","archiv":{"checkin_id":id,"chiffrat":"fixture"}}), 200),
                ("POST", format!("abholfach/{id}/erhalten"), json!({"protokoll":1,"sha256":"a".repeat(64)}), json!({"protokoll":1,"typ":"abholfach_erhalten"}), 200),
                ("GET", format!("abholfach/{id}"), Value::Null, json!({"protokoll":1,"typ":"fehler","code":"unbekannt"}), 404),
            ].into_iter().enumerate() {
                for challenge in [true, false] {
                    let (stream, _) = listener.accept().unwrap();
                    stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
                    let mut r = std::io::BufReader::new(stream);
                    let mut start = String::new();
                    r.read_line(&mut start).unwrap();
                    let mut headers = std::collections::HashMap::new();
                    loop {
                        let mut line = String::new();
                        r.read_line(&mut line).unwrap();
                        if line == "\r\n" { break; }
                        let (k,v) = line.split_once(':').unwrap();
                        headers.insert(k.to_ascii_lowercase(), v.trim().to_owned());
                    }
                    let mut bytes = vec![0; headers.get("content-length").map(|n| n.parse().unwrap()).unwrap_or(0)];
                    r.read_exact(&mut bytes).unwrap();
                    let body = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap() };
                    let cid = format!("cid-{i}");
                    let (packet, status) = if challenge {
                        assert!(start.starts_with("POST /v1/empfang/challenge "));
                        assert_eq!(body, json!({"checkin_id":"active","protokoll":1}));
                        (json!({"protokoll":1,"typ":"challenge","challenge_id":cid,"nonce":"fixture-nonce"}), 200)
                    } else {
                        assert!(start.starts_with(&format!("{method} /v1/empfang/{route} ")));
                        assert_eq!(body, expected);
                        assert_eq!(headers["x-cocopi-checkin"], "active");
                        assert_eq!(headers["x-cocopi-challenge"], cid);
                        let sig: [u8; 64] = B64.decode(&headers["x-cocopi-signatur"]).unwrap().try_into().unwrap();
                        verifying.verify_strict(format!("cocopi-v1-empfang\n{expected_origin}\nactive\n{cid}\nfixture-nonce").as_bytes(), &Signature::from_bytes(&sig)).unwrap();
                        (packet.clone(), status)
                    };
                    let text = packet.to_string();
                    write!(r.get_mut(), "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len()).unwrap();
                }
            }
        });
        let c = Empfang {
            origin,
            agent: agent(false),
            auth: Some(("active".into(), key)),
            empfang_pubkey: None,
        };
        assert_eq!(
            c.checkout_mit_auswahl("active", selection).unwrap()["typ"],
            "checkout_bereit"
        );
        assert_eq!(c.abholfach().unwrap()["archive"][0]["checkin_id"], id);
        assert_eq!(
            c.abholfach_holen(id).unwrap()["archiv"]["chiffrat"],
            "fixture"
        );
        assert_eq!(
            c.abholfach_erhalten(id, &"a".repeat(64)).unwrap()["typ"],
            "abholfach_erhalten"
        );
        assert!(
            c.abholfach_holen(id)
                .unwrap_err()
                .starts_with("[unbekannt]")
        );
        assert!(c.checkout_mit_auswahl("active", Value::Null).is_err());
        assert!(c.abholfach_holen("../escape").is_err());
        assert!(c.abholfach_erhalten(id, "falsch").is_err());
        server.join().unwrap();
    }
    #[test]
    fn nonce_activity_and_signed_directory_use_fresh_challenges() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let signer = SigningKey::from_bytes(&[12; 32]);
        let pin = B64.encode(signer.verifying_key().as_bytes());
        let device = SigningKey::from_bytes(&[13; 32]);
        let pubkey = B64.encode(device.verifying_key().as_bytes());
        let expected_pubkey = pubkey.clone();
        let expected_origin = origin.clone();
        let verifying = device.verifying_key();
        let server = std::thread::spawn(move || {
            let mut directory = json!({"protokoll":1,"verzeichnis":{"ausgestellt":1793232000,"personen":[{"person":"pid-mira","anzeige":"Mira","rolle":"teilnehmende","net_pubkey":"a".repeat(64)}]}});
            directory["signatur"] = json!(
                B64.encode(
                    signer
                        .sign(canonical_json(&directory).unwrap().as_bytes())
                        .to_bytes()
                )
            );
            for (i, path) in ["nonce", "aktiv", "aktiv", "seminar", "seminar", "seminar"]
                .into_iter()
                .enumerate()
            {
                for challenge in if i == 0 {
                    vec![false]
                } else {
                    vec![true, false]
                } {
                    let (stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                        .unwrap();
                    let mut r = std::io::BufReader::new(stream);
                    let req = read_mock_request(&mut r);
                    let (head, body) = req.split_once("\r\n\r\n").unwrap();
                    let headers: std::collections::HashMap<_, _> = head
                        .lines()
                        .skip(1)
                        .filter_map(|l| {
                            l.split_once(':')
                                .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
                        })
                        .collect();
                    let body: Value = if body.is_empty() {
                        Value::Null
                    } else {
                        serde_json::from_str(body).unwrap()
                    };
                    let cid = format!("cid-{i}");
                    let response = if challenge {
                        assert!(head.starts_with("POST /v1/empfang/challenge "));
                        assert_eq!(body, json!({"protokoll":1,"checkin_id":"active"}));
                        json!({"protokoll":1,"typ":"challenge","challenge_id":cid,"nonce":"device-challenge"})
                    } else if i == 0 {
                        assert!(head.starts_with("POST /v1/empfang/nonce "));
                        assert_eq!(body, json!({"protokoll":1,"geraet_pubkey":expected_pubkey}));
                        assert!(!headers.contains_key("x-cocopi-checkin"));
                        json!({"protokoll":1,"typ":"nonce","nonce":"server-geraetegebundene-nonce-0123456789","gueltig_bis":1793232060000u64})
                    } else {
                        let method = if path == "seminar" { "GET" } else { "POST" };
                        assert!(head.starts_with(&format!("{method} /v1/empfang/{path} ")));
                        assert_eq!(headers["x-cocopi-checkin"], "active");
                        assert_eq!(headers["x-cocopi-challenge"], cid);
                        let sig: [u8; 64] = B64
                            .decode(&headers["x-cocopi-signatur"])
                            .unwrap()
                            .try_into()
                            .unwrap();
                        verifying.verify_strict(format!("cocopi-v1-empfang\n{expected_origin}\nactive\n{cid}\ndevice-challenge").as_bytes(), &Signature::from_bytes(&sig)).unwrap();
                        if path == "aktiv" {
                            assert_eq!(body, json!({"protokoll":1,"verbunden":i == 1}));
                            json!({"protokoll":1,"typ":"aktiv_bestaetigt"})
                        } else {
                            assert_eq!(body, Value::Null);
                            let mut response = directory.clone();
                            if i == 4 {
                                response["verzeichnis"]["personen"][0]["rolle"] =
                                    json!("seminarleitung");
                            }
                            if i == 5 {
                                response.as_object_mut().unwrap().remove("signatur");
                            }
                            response
                        }
                    };
                    let text = response.to_string();
                    write!(
                        r.get_mut(),
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                        text.len()
                    )
                    .unwrap();
                }
            }
        });
        let mut client = Empfang::new(&origin).unwrap();
        assert!(client.aktiv(true).is_err());
        assert!(client.seminar_verzeichnis().is_err());
        // nonce selbst mit altem Auth-Kontext unauthentisiert abrufen.
        client.auth = Some(("active".into(), device));
        assert_eq!(
            client.nonce(&pubkey).unwrap(),
            "server-geraetegebundene-nonce-0123456789"
        );
        assert!(client.nonce("falsch").is_err());
        assert!(client.seminar_verzeichnis().is_err()); // Pin fehlt, kein Netz.
        client.empfang_pubkey = Some(pin);
        client.aktiv(true).unwrap();
        client.aktiv(false).unwrap();
        let directory = client.seminar_verzeichnis().unwrap();
        assert_eq!(directory.ausgestellt, 1793232000);
        assert_eq!(directory.personen[0].person, "pid-mira");
        assert_eq!(directory.personen[0].anzeige, "Mira");
        assert_eq!(directory.personen[0].rolle, "teilnehmende");
        assert_eq!(directory.personen[0].net_pubkey, "a".repeat(64));
        assert!(client.seminar_verzeichnis().is_err());
        assert!(client.seminar_verzeichnis().is_err());
        server.join().unwrap();
    }
    #[test]
    fn directory_rejects_wrong_pin_invalid_contact_and_legacy_unsigned_wrapper() {
        let signer = SigningKey::from_bytes(&[11; 32]);
        let pin = B64.encode(signer.verifying_key().as_bytes());
        let sign = |mut v: Value| {
            v["signatur"] = json!(
                B64.encode(
                    signer
                        .sign(canonical_json(&v).unwrap().as_bytes())
                        .to_bytes()
                )
            );
            v
        };
        let packet = json!({"protokoll":1,"verzeichnis":{"ausgestellt":1793232000,"personen":[{"person":"mira","anzeige":"Mira","rolle":"teilnehmende","net_pubkey":"a".repeat(64)}]}});
        let signed = sign(packet.clone());
        assert!(Empfang::pruefen_verzeichnis(&signed, &pin).is_ok());
        let other = B64.encode(SigningKey::from_bytes(&[10; 32]).verifying_key().as_bytes());
        assert!(Empfang::pruefen_verzeichnis(&signed, &other).is_err());
        for (field, value) in [
            ("rolle", "root"),
            ("person", ""),
            ("anzeige", "\n"),
            ("net_pubkey", "a"),
            ("net_pubkey", &"A".repeat(64)),
        ] {
            let mut bad = packet.clone();
            bad["verzeichnis"]["personen"][0][field] = json!(value);
            assert!(
                Empfang::pruefen_verzeichnis(&sign(bad), &pin).is_err(),
                "{field}"
            );
        }
        let mut bad = signed.clone();
        bad["protokoll"] = json!(2);
        assert!(Empfang::pruefen_verzeichnis(&bad, &pin).is_err());
        // Alte N1-Hülle nur ohne unsignierte Extras lesbar.
        let nested = json!({"inhalt":packet,"signatur":signed["signatur"]});
        assert!(Empfang::pruefen_verzeichnis(&nested, &pin).is_ok());
        let mut bad = nested;
        bad["protokoll"] = json!(1);
        assert!(Empfang::pruefen_verzeichnis(&bad, &pin).is_err());
    }
    #[test]
    fn client_local_mock_201_202_checkout_and_limits() {
        use std::io::Write;
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
                let (stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut stream = std::io::BufReader::new(stream);
                let req = read_mock_request(&mut stream);
                assert!(req.contains(&format!("/v1/empfang/{route} ")));
                let text = packet.to_string();
                write!(stream.get_mut(), "HTTP/1.1 {status} Mock\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).unwrap();
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
