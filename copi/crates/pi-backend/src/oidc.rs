//! OIDC Authorization Code + PKCE; System-Browser nur durch App-Callback.
//! Keine JWT-Vertrauensentscheidung hier: der Empfang prüft id_token/JWKS und Rolle.
//! SHA-256 wird als Callback einer geprüften Kryptobibliothek injiziert (keine Eigenkrypto).
use super::{agent, secure_random, validate_url};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

pub struct Discovery {
    pub issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
}
fn endpoint(url: &str) -> Result<(), String> {
    let uri: ureq::http::Uri = url
        .parse()
        .map_err(|_| "Die Anmeldeadresse ist ungültig.")?;
    let origin = format!(
        "{}://{}",
        uri.scheme_str()
            .ok_or("Die Anmeldeadresse ist unvollständig.")?,
        uri.authority()
            .ok_or("Die Anmeldeadresse ist unvollständig.")?
    );
    validate_url(&origin)?;
    if url.contains('#') || url.contains('\\') || url.chars().any(char::is_control) {
        return Err("Die Anmeldeadresse ist ungültig.".into());
    }
    Ok(())
}
fn response_json(mut response: ureq::http::Response<ureq::Body>) -> Result<Value, String> {
    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_string()
        .map_err(|_| "Die Antwort der Anmeldung ist zu groß oder unvollständig.")?;
    if !(200..300).contains(&status) {
        return Err("Die Anmeldung wurde abgewiesen. Versuche es erneut.".into());
    }
    serde_json::from_str(&text)
        .map_err(|_| "Die Anmeldung hat keine gültige Antwort gesendet.".into())
}
impl Discovery {
    pub fn load(issuer: &str) -> Result<Self, String> {
        endpoint(issuer)?;
        if issuer.contains('?') {
            return Err("Die Anbieteradresse darf keine Suchparameter enthalten.".into());
        }
        let v = response_json(
            agent(false)
                .get(format!(
                    "{}/.well-known/openid-configuration",
                    issuer.trim_end_matches('/')
                ))
                .call()
                .map_err(|_| "Der Anmeldedienst ist nicht erreichbar.")?,
        )?;
        if v["issuer"] != issuer {
            return Err("Der Anmeldedienst hat eine andere Identität gemeldet.".into());
        }
        let authorization_endpoint = v["authorization_endpoint"]
            .as_str()
            .ok_or("Die Browser-Anmeldeadresse fehlt.")?
            .to_string();
        let token_endpoint = v["token_endpoint"]
            .as_str()
            .ok_or("Die Bestätigungsadresse fehlt.")?
            .to_string();
        endpoint(&authorization_endpoint)?;
        endpoint(&token_endpoint)?;
        if issuer.starts_with("https:")
            && (!authorization_endpoint.starts_with("https:")
                || !token_endpoint.starts_with("https:"))
        {
            return Err("Der Anmeldedienst bietet keine sichere Verbindung an.".into());
        }
        if !v["code_challenge_methods_supported"]
            .as_array()
            .is_some_and(|a| a.iter().any(|m| m == "S256"))
        {
            return Err(
                "Der Anmeldedienst unterstützt keine sichere Geräteanmeldung (PKCE S256).".into(),
            );
        }
        Ok(Self {
            issuer: issuer.into(),
            authorization_endpoint,
            token_endpoint,
        })
    }
}
pub fn valid_verifier(verifier: &str) -> bool {
    (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
}
/// `sha256` muss SHA-256 aus einer geprüften Bibliothek sein, niemals ein eigener Hash.
pub fn pkce_challenge(
    verifier: &str,
    sha256: impl FnOnce(&[u8]) -> [u8; 32],
) -> Result<String, String> {
    if !valid_verifier(verifier) {
        return Err("Der Anmeldeschlüssel hat ein ungültiges Format.".into());
    }
    Ok(B64.encode(sha256(verifier.as_bytes())))
}
fn random() -> Result<String, String> {
    let mut bytes = [0; 32];
    secure_random(&mut bytes)?;
    Ok(B64.encode(bytes))
}
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn decode(value: &str) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut rest = value.as_bytes();
    while let Some((&b, tail)) = rest.split_first() {
        if b == b'%' {
            if rest.len() < 3 {
                return Err("Die Anmeldeantwort ist beschädigt.".into());
            }
            let hex = std::str::from_utf8(&rest[1..3])
                .map_err(|_| "Die Anmeldeantwort ist beschädigt.")?;
            bytes.push(
                u8::from_str_radix(hex, 16).map_err(|_| "Die Anmeldeantwort ist beschädigt.")?,
            );
            rest = &rest[3..];
        } else {
            bytes.push(if b == b'+' { b' ' } else { b });
            rest = tail;
        }
    }
    let text = String::from_utf8(bytes).map_err(|_| "Die Anmeldeantwort ist beschädigt.")?;
    if text.chars().any(char::is_control) {
        return Err("Die Anmeldeantwort ist beschädigt.".into());
    }
    Ok(text)
}
fn parameters(query: &str) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut result = std::collections::BTreeMap::new();
    for pair in query.split('&') {
        let (key, value) = pair
            .split_once('=')
            .ok_or("Die Anmeldeantwort ist unvollständig.")?;
        if result.insert(decode(key)?, decode(value)?).is_some() {
            return Err("Die Anmeldeantwort enthält doppelte Angaben.".into());
        }
    }
    Ok(result)
}
fn redirect(
    stream: &mut TcpStream,
    state: &str,
    expected_host: &str,
    timeout: Duration,
) -> Result<String, String> {
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|_| "Die Anmeldeantwort konnte nicht gelesen werden.")?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|_| "Die Anmeldeantwort konnte nicht bestätigt werden.")?;
    let deadline = Instant::now() + timeout;
    let mut bytes = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Err("Die Anmeldung hat zu lange gedauert.".into());
        }
        stream
            .set_read_timeout(Some(deadline.saturating_duration_since(Instant::now())))
            .map_err(|_| "Die Anmeldung hat zu lange gedauert.")?;
        let mut b = [0];
        if stream
            .read(&mut b)
            .map_err(|_| "Die Anmeldung hat zu lange gedauert.")?
            == 0
        {
            return Err("Die Anmeldeantwort wurde unterbrochen.".into());
        }
        bytes.push(b[0]);
        if bytes.len() > 8192 {
            return Err("Die Anmeldeantwort ist zu groß.".into());
        }
        if bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let request = std::str::from_utf8(&bytes).map_err(|_| "Die Anmeldeantwort ist beschädigt.")?;
    let mut lines = request.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut first = first.split(' ');
    if first.next() != Some("GET") {
        return Err("Die Anmeldeantwort verwendet eine falsche Methode.".into());
    }
    let target = first
        .next()
        .ok_or("Die Anmeldeantwort ist unvollständig.")?;
    if !matches!(first.next(), Some("HTTP/1.1" | "HTTP/1.0")) || first.next().is_some() {
        return Err("Die Anmeldeantwort ist beschädigt.".into());
    }
    let hosts: Vec<_> = lines
        .filter_map(|line| {
            line.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
                .map(|(_, v)| v.trim())
        })
        .collect();
    if hosts != [expected_host] {
        return Err("Die Anmeldeantwort gehört zu einem anderen Gerät.".into());
    }
    let query = target
        .strip_prefix("/oidc/callback?")
        .ok_or("Die Anmeldeantwort verwendet einen falschen Rückweg.")?;
    let p = parameters(query)?;
    if p.get("state").map(String::as_str) != Some(state) {
        return Err("Die Anmeldeantwort gehört nicht zu dieser Anmeldung.".into());
    }
    if p.contains_key("error") {
        return Err("Die Anmeldung wurde abgebrochen. Du kannst sie erneut starten.".into());
    }
    p.get("code")
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .cloned()
        .ok_or("Die Anmeldung enthält keine Bestätigung.".into())
}

pub(crate) fn validate_nonce(nonce: &str) -> Result<(), String> {
    if !(16..=256).contains(&nonce.len())
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
    {
        return Err(
            "Die Anmeldeprüfung des Empfangs ist ungültig. Starte die Anmeldung erneut.".into(),
        );
    }
    Ok(())
}

pub struct Login {
    discovery: Discovery,
    client_id: String,
    listener: TcpListener,
    verifier: String,
    pub nonce: String,
    state: String,
    redirect_uri: String,
    authorization_url: String,
}
/// Nicht Debug/Clone: enthält Verifier und Nonce. Kein Öffnen eines Browsers im Backend.
impl Login {
    /// Kompatibilität für ältere Aufrufer. Für Empfang-Check-in ausschließlich
    /// begin_with_nonce verwenden; eine lokale nonce ist nicht gerätegebunden.
    pub fn begin(
        discovery: Discovery,
        client_id: &str,
        scopes: &[&str],
        sha256: impl FnOnce(&[u8]) -> [u8; 32],
    ) -> Result<Self, String> {
        Self::begin_with_nonce(discovery, client_id, scopes, &random()?, sha256)
    }
    /// Die frische, gerätegebundene nonce von `Empfang::nonce` unverändert in
    /// Auth-URL und späteren Check-in-Nachweis übernehmen. Kein Browser-Autostart.
    pub fn begin_with_nonce(
        discovery: Discovery,
        client_id: &str,
        scopes: &[&str],
        server_nonce: &str,
        sha256: impl FnOnce(&[u8]) -> [u8; 32],
    ) -> Result<Self, String> {
        validate_nonce(server_nonce)?;
        if client_id.is_empty() || scopes.is_empty() || !scopes.contains(&"openid") {
            return Err("Für die Anmeldung fehlen Anbieterangaben.".into());
        }
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|_| "Der lokale Rückweg für die Anmeldung konnte nicht geöffnet werden.")?;
        listener
            .set_nonblocking(true)
            .map_err(|_| "Der Anmelderückweg ist nicht bereit.")?;
        let redirect_uri = format!(
            "http://{}/oidc/callback",
            listener
                .local_addr()
                .map_err(|_| "Der Anmelderückweg ist nicht bereit.")?
        );
        let verifier = random()?;
        let state = random()?;
        let nonce = server_nonce.to_owned();
        let challenge = pkce_challenge(&verifier, sha256)?;
        let query = [
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", &redirect_uri),
            ("scope", &scopes.join(" ")),
            ("code_challenge_method", "S256"),
            ("code_challenge", &challenge),
            ("state", &state),
            ("nonce", &nonce),
        ]
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect::<Vec<_>>()
        .join("&");
        let separator = if discovery.authorization_endpoint.contains('?') {
            '&'
        } else {
            '?'
        };
        let authorization_url = format!("{}{separator}{query}", discovery.authorization_endpoint);
        Ok(Self {
            discovery,
            client_id: client_id.into(),
            listener,
            verifier,
            nonce,
            state,
            redirect_uri,
            authorization_url,
        })
    }
    /// Die App öffnet erst nach bewusstem Klick den System-Browser.
    pub fn open_browser(
        &self,
        open: impl FnOnce(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        open(&self.authorization_url)
    }
    /// Nimmt genau einen HTTP-Rückruf an. Falscher State beendet den Versuch ebenfalls.
    /// `timeout` begrenzt Listener + Lesen; kein unbegrenztes Warten im UI.
    pub fn finish(self, timeout: Duration) -> Result<Value, String> {
        if timeout.is_zero() || timeout > Duration::from_secs(600) {
            return Err("Die Wartezeit für die Anmeldung ist ungültig.".into());
        }
        let deadline = Instant::now() + timeout;
        let (mut stream, peer) = loop {
            if Instant::now() >= deadline {
                return Err("Die Anmeldung hat zu lange gedauert. Starte sie erneut.".into());
            }
            match self.listener.accept() {
                Ok(v) => break v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => return Err("Die Anmeldeantwort konnte nicht empfangen werden.".into()),
            }
        };
        if !peer.ip().is_loopback() {
            return Err("Die Anmeldeantwort stammt nicht von diesem Gerät.".into());
        }
        drop(self.listener);
        let host = self
            .redirect_uri
            .strip_prefix("http://")
            .and_then(|s| s.split('/').next())
            .ok_or("Der Rückweg ist ungültig.")?;
        let code = redirect(
            &mut stream,
            &self.state,
            host,
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(5)),
        );
        let html = if code.is_ok() {
            "<!doctype html><html lang=\"de\"><meta charset=\"utf-8\"><title>cocopi</title><p>Du kannst zu cocopi zurückkehren.</p></html>"
        } else {
            "<!doctype html><html lang=\"de\"><meta charset=\"utf-8\"><title>cocopi</title><p>Die Anmeldung konnte nicht bestätigt werden. Starte sie in cocopi erneut.</p></html>"
        };
        let status = if code.is_ok() {
            "200 OK"
        } else {
            "400 Bad Request"
        };
        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{html}",
            html.len()
        );
        drop(stream);
        let code = code?;
        let body = [
            ("grant_type", "authorization_code"),
            ("client_id", &self.client_id),
            ("code", &code),
            ("redirect_uri", &self.redirect_uri),
            ("code_verifier", &self.verifier),
        ]
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect::<Vec<_>>()
        .join("&");
        let token = response_json(
            agent(false)
                .post(&self.discovery.token_endpoint)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .send(body.as_bytes())
                .map_err(|_| "Der Anmeldedienst konnte die Anmeldung nicht bestätigen.")?,
        )?;
        let id_token = token["id_token"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Der Anmeldedienst hat keinen Identitätsnachweis gesendet.")?;
        Ok(json!({"art":"oidc","id_token":id_token,"nonce":self.nonce}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_rfc7636_vector_and_verifier_bounds() {
        // SHA-256-Ausgabe aus RFC 7636, Anhang B. Produktiver Hash kommt per Callback.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let digest: [u8; 32] = B64
            .decode("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM")
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            pkce_challenge(verifier, |v| {
                assert_eq!(v, verifier.as_bytes());
                digest
            })
            .unwrap(),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert!(!valid_verifier(&"a".repeat(42)));
        assert!(valid_verifier(&"a".repeat(128)));
        assert!(!valid_verifier(&"a".repeat(129)));
        assert!(!valid_verifier(&"!".repeat(43)));
        assert_eq!(decode(&encode("a +ä&=")).unwrap(), "a +ä&=");
        assert!(parameters("state=a&state=b").is_err());
        for bad in ["", "kurz", "angriff&nonce=ersatz", "mit\nsteuerzeichen"] {
            assert!(validate_nonce(bad).is_err());
        }
        assert!(validate_nonce(&"a".repeat(257)).is_err());
        assert!(validate_nonce("server-geraetegebundene-nonce-0123456789").is_ok());
    }
    #[test]
    fn local_discovery_browser_callback_token_exchange() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let issuer = format!("http://{}", server.local_addr().unwrap());
        let origin = issuer.clone();
        let mock = std::thread::spawn(move || {
            for i in 0..2 {
                let (mut s, _) = server.accept().unwrap();
                s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut bytes = Vec::new();
                let mut b = [0; 1];
                while !bytes.ends_with(b"\r\n\r\n") {
                    s.read_exact(&mut b).unwrap();
                    bytes.push(b[0]);
                }
                let headers = String::from_utf8(bytes).unwrap();
                let text = if i == 0 {
                    assert!(headers.starts_with("GET /.well-known/openid-configuration "));
                    json!({"issuer":origin,"authorization_endpoint":format!("{origin}/authorize"),"token_endpoint":format!("{origin}/token"),"code_challenge_methods_supported":["S256"]}).to_string()
                } else {
                    assert!(headers.starts_with("POST /token "));
                    let len: usize = headers
                        .lines()
                        .find_map(|s| {
                            s.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|s| s.trim().parse().unwrap())
                        })
                        .unwrap();
                    let mut body = vec![0; len];
                    s.read_exact(&mut body).unwrap();
                    let p = parameters(std::str::from_utf8(&body).unwrap()).unwrap();
                    assert_eq!(p["code"], "test-code");
                    assert_eq!(p["client_id"], "cocopi");
                    assert!(valid_verifier(&p["code_verifier"]));
                    assert!(!p.contains_key("client_secret"));
                    json!({"id_token":"mock-nicht-produktiv"}).to_string()
                };
                write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                    text.len()
                )
                .unwrap();
            }
        });
        let d = Discovery::load(&issuer).unwrap();
        let login = Login::begin_with_nonce(
            d,
            "cocopi",
            &["openid", "profile"],
            "server-geraetegebundene-nonce-0123456789",
            |_| [7; 32],
        )
        .unwrap();
        let expected_nonce = "server-geraetegebundene-nonce-0123456789".to_owned();
        assert_eq!(login.nonce, expected_nonce);
        let mut url = String::new();
        login
            .open_browser(|u| {
                url = u.into();
                Ok(())
            })
            .unwrap();
        let query = parameters(url.split_once('?').unwrap().1).unwrap();
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["nonce"], expected_nonce);
        let callback = std::thread::spawn(move || {
            let uri = query["redirect_uri"].strip_prefix("http://").unwrap();
            let (host, path) = uri.split_once('/').unwrap();
            let mut s = TcpStream::connect(host).unwrap();
            write!(
                s,
                "GET /{path}?code=test-code&state={} HTTP/1.1\r\nHost: {host}\r\n\r\n",
                query["state"]
            )
            .unwrap();
            let mut html = String::new();
            s.read_to_string(&mut html).unwrap();
            assert!(html.contains("Du kannst zu cocopi zurückkehren"));
        });
        let v = login.finish(Duration::from_secs(3)).unwrap();
        assert_eq!(v["nonce"], expected_nonce);
        assert_eq!(v["id_token"], "mock-nicht-produktiv");
        callback.join().unwrap();
        mock.join().unwrap();
    }
    #[test]
    fn redirect_wrong_state_consumes_attempt_and_timeout() {
        let d = || Discovery {
            issuer: "http://127.0.0.1".into(),
            authorization_endpoint: "http://127.0.0.1/authorize".into(),
            token_endpoint: "http://127.0.0.1/token".into(),
        };
        let login = Login::begin(d(), "cocopi", &["openid"], |_| [0; 32]).unwrap();
        let host = login.listener.local_addr().unwrap();
        let t = std::thread::spawn(move || {
            let mut s = TcpStream::connect(host).unwrap();
            write!(
                s,
                "GET /oidc/callback?code=x&state=falsch HTTP/1.1\r\nHost: {host}\r\n\r\n"
            )
            .unwrap();
            let mut text = String::new();
            s.read_to_string(&mut text).unwrap();
            assert!(text.starts_with("HTTP/1.1 400"));
        });
        assert!(
            login
                .finish(Duration::from_secs(2))
                .unwrap_err()
                .contains("nicht zu dieser")
        );
        t.join().unwrap();
        assert!(TcpStream::connect_timeout(&host, Duration::from_millis(30)).is_err());
        assert!(
            Login::begin(d(), "cocopi", &["openid"], |_| [0; 32])
                .unwrap()
                .finish(Duration::from_millis(20))
                .is_err()
        );
        assert!(endpoint("http://evil.example/a").is_err());
        assert!(endpoint("https://user:pass@example.test/a").is_err());
    }
}
