//! Adapter für den von S3 besessenen Hub-v1-Client. Keine eigene HTTP-/Kryptologik.
use super::{
    Account, DesktopApi, Device, InviteError, PairCode, Reply, Request, Session, SessionState,
};
use pi_backend::hub_v1::{DeviceKey, HubV1};
use serde_json::Value;
use std::{
    sync::{mpsc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Connection {
    hub: HubV1,
    key: Option<DeviceKey>,
    name: String,
}
pub struct HubAdapter {
    connection: Mutex<Connection>,
}
impl HubAdapter {
    pub fn new(url: &str, name: &str) -> Result<Self, String> {
        Ok(Self {
            connection: Mutex::new(Connection {
                hub: HubV1::new(url)?,
                key: None,
                name: name.into(),
            }),
        })
    }
    fn invoke(&self, request: Request) -> Result<Reply, String> {
        let mut c = self
            .connection
            .lock()
            .map_err(|_| "Die Verbindung wurde unterbrochen. Melde dich erneut an.")?;
        match request {
            Request::Redeem(input) => {
                let mut key = c.key.take().map(Ok).unwrap_or_else(|| DeviceKey::load_or_create("seminar-desktop"))?;
                c.hub.hello("cocopi")?;
                let result = if key.geraet_id.is_some() && input.is_empty() { Ok(Value::Null) } else { c.hub.redeem(&mut key, &input, 'e', &c.name, "desktop") };
                let result = match result { Ok(v) => v, Err(e) => { c.key = Some(key); return Err(e); } };
                if result["typ"] == "koppeln_wartet" {
                    c.key = Some(key);
                    let confirmation = &result["bestaetigung"];
                    return Ok(Reply::PairLogin { id: string(&result, "anfrage_id"), pair: pair_code(confirmation)? });
                }
                let login = c.hub.login(&key);
                c.key = Some(key);
                let login = login?;
                Ok(Reply::SignedIn(Account { person: string(&login,"person"), slot: string(&login,"slot"), device_id: string(&login,"geraet_id"), device_name: c.name.clone() }))
            }
            Request::PairStatus(id) => {
                let mut key = c.key.take().ok_or("Die Geräteanmeldung ist nicht mehr vorhanden.")?;
                let result = c.hub.pairing_status(&mut key, &id);
                c.key = Some(key);
                if result?["typ"] == "koppeln_bestaetigt" {
                    let key = c.key.take().ok_or("Die Geräteanmeldung ist nicht mehr vorhanden.")?;
                    let login = c.hub.login(&key); c.key = Some(key);
                    let login = login?;
                    Ok(Reply::SignedIn(Account { person: string(&login,"person"), slot: string(&login,"slot"), device_id: string(&login,"geraet_id"), device_name: c.name.clone() }))
                } else { Ok(Reply::Ok) }
            }
            Request::Devices => Ok(devices(&c.hub.devices()?)),
            Request::Pair => { let v = c.hub.start_pairing()?; let until = v["gueltig_bis"].as_u64().ok_or("R2 hat keine Gültigkeitsdauer bestätigt.")?; Ok(Reply::Pair(PairCode { code: string(&v,"code"), url: string(&v,"url"), valid_for: Duration::from_millis(until.saturating_sub(now_ms())) })) }
            Request::PairDecision(id, allow) => { c.hub.approve_pairing(&id, allow)?; Ok(Reply::PairDone) }
            Request::Revoke(id) => { c.hub.revoke(&id)?; Ok(Reply::Revoked(id)) }
            Request::Rename(_) => Err("Der Gerätename kann auf R2 in I1 noch nicht geändert werden. Es wurde nichts umbenannt.".into()),
            Request::Sessions => Ok(sessions(&c.hub.sessions()?)),
            Request::Open(id) => {
                let opened = c.hub.open_session(if id.is_empty() { None } else { Some(id.as_str()) }, None, None)?;
                let id = string(&opened, "session_id"); c.hub.view(&id)?;
                let Reply::Sessions(list) = sessions(&c.hub.sessions()?) else { unreachable!() };
                Ok(Reply::Opened(id, list))
            }
            Request::Rest(id) => { c.hub.rest(&id)?; Ok(sessions(&c.hub.sessions()?)) }
            Request::Prompt(id, text) => { c.hub.send(&id, &text)?; Ok(Reply::Sent(id, text)) }
            Request::Stop(id) => { c.hub.stop(&id)?; Ok(sessions(&c.hub.sessions()?)) }
        }
    }
}
impl DesktopApi for HubAdapter {
    fn call(&self, request: Request) -> Reply {
        let invitation = matches!(request, Request::Redeem(_));
        self.invoke(request)
            .unwrap_or_else(|e| error(&e, invitation))
    }
    fn watch(&self, tx: mpsc::Sender<Reply>) {
        let stream = self
            .connection
            .lock()
            .map_err(|_| "Verbindung gesperrt".to_string())
            .and_then(|c| c.hub.events(None));
        match stream {
            Ok(mut stream) => loop {
                match stream.next_event() {
                    Ok(Some(ev)) => {
                        let _ = tx.send(Reply::Heartbeat);
                        if let Some(reply) = event(&ev.packet) {
                            if tx.send(reply).is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) | Err(_) => {
                        let _ = tx.send(Reply::Disconnected("Die Verbindung zu R2 wurde unterbrochen. Prüfe den letzten Auftrag, bevor du erneut sendest.".into()));
                        break;
                    }
                }
            },
            Err(e) => {
                let _ = tx.send(Reply::Disconnected(e));
            }
        }
    }
}
fn pair_code(v: &Value) -> Result<PairCode, String> {
    let until = v["gueltig_bis"]
        .as_u64()
        .ok_or("R2 hat keine Gültigkeitsdauer bestätigt.")?;
    let url = string(v, "url");
    if url.is_empty() {
        return Err("R2 hat keinen Bestätigungslink gesendet.".into());
    }
    Ok(PairCode {
        code: string(v, "code"),
        url,
        valid_for: Duration::from_millis(until.saturating_sub(now_ms())),
    })
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn string(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().into()
}
fn devices(v: &Value) -> Reply {
    Reply::Devices(
        v["geraete"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| !d["widerrufen"].as_bool().unwrap_or(false))
            .map(|d| Device {
                id: string(d, "geraet_id"),
                name: string(d, "geraetename"),
                kind: if d["art"] == "handy" {
                    "Handy".into()
                } else {
                    "Rechner".into()
                },
                last_seen: d["zuletzt_gesehen"]
                    .as_u64()
                    .map(|t| format!("vor {} min", now_ms().saturating_sub(t) / 60_000))
                    .unwrap_or_else(|| "unbekannt".into()),
            })
            .collect(),
    )
}
fn session(v: &Value) -> Session {
    Session {
        id: string(v, "session_id"),
        title: string(v, "titel"),
        state: match v["zustand"].as_str() {
            Some("arbeitet") => SessionState::Working,
            Some("wartet") => SessionState::NeedsYou,
            Some("fertig") => SessionState::Done,
            Some("bereit") => SessionState::Ready,
            Some("ruht") => SessionState::Resting,
            _ => SessionState::Error,
        },
        local: false,
        pinned: false,
    }
}
fn sessions(v: &Value) -> Reply {
    Reply::Sessions(
        v["sessions"]
            .as_array()
            .into_iter()
            .flatten()
            .map(session)
            .collect(),
    )
}
fn event(v: &Value) -> Option<Reply> {
    match v["typ"].as_str()? {
        "sessions_liste" => Some(sessions(v)),
        "session_zustand" => Some(Reply::SessionUpdated(session(v))),
        "session_ereignis" => Some(Reply::PiEvent(
            string(v, "session_id"),
            v["pi_event"].clone(),
        )),
        "koppeln_wartet" => Some(Reply::PairWaiting {
            id: string(v, "anfrage_id"),
            name: string(v, "geraetename"),
            words: v["pruefwort"].as_str().unwrap_or("").into(),
        }),
        "koppeln_bestaetigt" => Some(Reply::PairDone),
        "widerruf" => Some(Reply::Revoked(string(v, "geraet_id"))),
        "fehler" => Some(error(
            &format!(
                "[{}] {} {}",
                string(v, "code"),
                string(v, "was"),
                string(v, "tun")
            ),
            false,
        )),
        _ => None,
    }
}
fn error(text: &str, invitation: bool) -> Reply {
    if text.starts_with("[zu_viele_offen]") {
        return Reply::Limit;
    }
    let e = if text.starts_with("[nicht_angemeldet]") {
        InviteError::Revoked
    } else if invitation && text.starts_with("[abgelaufen]") {
        InviteError::Expired
    } else if invitation && text.starts_with("[verbraucht]") {
        InviteError::Used
    } else if invitation && text.starts_with("[unbekannt]") {
        InviteError::Unknown
    } else if text.starts_with("[zu_viele]") {
        InviteError::RateLimited
    } else {
        InviteError::Other(text.into())
    };
    Reply::Error(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn adapter_against_isolated_node_mock() {
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
        };
        let hub = std::path::Path::new("/home/medienhaus/Work/pi-hub-wt/s3");
        assert!(
            hub.join("server.mjs").is_file(),
            "S3-Mock fehlt; kein Live-Fallback"
        );
        let root = std::env::temp_dir().join(format!("s2-adapter-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let envs = [
            ("HUB_SLOT", "account01".to_string()),
            ("HUB_STATE", root.join("state").display().to_string()),
            ("HUB_TOKEN", "demo".into()),
            ("HUB_ROOTS", root.display().to_string()),
            ("HUB_PORT", "0".into()),
            ("HUB_HOST", "127.0.0.1".into()),
            ("HUB_NTFY_URL", "".into()),
            ("HUB_V1_HARNESS", "mock".into()),
            (
                "PI_SESSIONS_DIR",
                root.join("sessions").display().to_string(),
            ),
        ];
        let invitation = Command::new("node")
            .current_dir(hub)
            .args([
                "server.mjs",
                "admin",
                "einladung",
                "--slot",
                "account01",
                "--name",
                "Mira",
            ])
            .envs(envs.iter().map(|(k, v)| (*k, v)))
            .output()
            .unwrap();
        assert!(invitation.status.success());
        let code = pi_backend::hub_v1::parse_invitation(
            String::from_utf8(invitation.stdout).unwrap().trim(),
            'e',
        )
        .unwrap()
        .code;
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Child(
            Command::new("node")
                .current_dir(hub)
                .arg("server.mjs")
                .envs(envs.iter().map(|(k, v)| (*k, v)))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut line = String::new();
        BufReader::new(child.0.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let url = line.trim().strip_prefix("pi-hub läuft auf ").unwrap();
        let adapter = HubAdapter::new(url, "Testrechner").unwrap();
        adapter.connection.lock().unwrap().key =
            Some(DeviceKey::at(&root.join("devices"), "desktop").unwrap());
        let Reply::SignedIn(account) = adapter.call(Request::Redeem(code)) else {
            panic!("Anmeldung nicht bestätigt")
        };
        assert_eq!(account.slot, "account01");
        assert_eq!(account.person, "Mira");
        assert!(matches!(adapter.call(Request::Devices), Reply::Devices(d) if d.len() == 1));
        let Reply::Opened(id, sessions) = adapter.call(Request::Open(String::new())) else {
            panic!("Session nicht geöffnet")
        };
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].state, SessionState::Ready);
        assert!(
            matches!(adapter.call(Request::Rest(id)), Reply::Sessions(s) if s[0].state == SessionState::Resting)
        );
        assert!(
            matches!(adapter.call(Request::Pair), Reply::Pair(p) if p.url.contains("#1.k.") && p.valid_for.as_secs() <= 300)
        );
        assert!(matches!(
            adapter.call(Request::Revoke(account.device_id)),
            Reply::Revoked(_)
        ));
        assert!(matches!(adapter.call(Request::Sessions), Reply::Error(_)));
        drop(child);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v1_errors_map_without_guessing_ui_text() {
        assert!(matches!(
            error("[zu_viele_offen] text", false),
            Reply::Limit
        ));
        assert!(matches!(
            error("[abgelaufen] text", true),
            Reply::Error(InviteError::Expired)
        ));
        assert!(matches!(
            error("[verbraucht] text", true),
            Reply::Error(InviteError::Used)
        ));
        assert!(matches!(
            error("[nicht_angemeldet] text", false),
            Reply::Error(InviteError::Revoked)
        ));
    }
    #[test]
    fn sessions_come_from_hub_not_local_activity() {
        for (wire, expected) in [
            ("ruht", SessionState::Resting),
            ("bereit", SessionState::Ready),
            ("arbeitet", SessionState::Working),
            ("wartet", SessionState::NeedsYou),
            ("fertig", SessionState::Done),
            ("fehler", SessionState::Error),
        ] {
            let s = session(&json!({"session_id":"a","titel":"Entwurf","zustand":wire}));
            assert_eq!(s.state, expected);
            assert!(!s.local);
        }
    }
}
