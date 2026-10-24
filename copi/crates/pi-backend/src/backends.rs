//! Konfiguration der Backends (lokal und Hub-Server), gemeinsam für copi, pi-lite und cocopi.
//! Datei: `<config>/copi/backends.json`. Das Token steht im Klartext in der Datei (Modus 0600);
//! alternativ `token_env` mit dem Namen einer Umgebungsvariable.
use crate::instances::{HubInstances, Instances, LocalInstances};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackendCfg {
    pub id: String,
    pub label: String,
    /// "local" oder "hub"
    pub kind: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub token_env: String,
    /// Beim Start verbinden und anzeigen.
    #[serde(default = "yes")]
    pub enabled: bool,
}
fn yes() -> bool {
    true
}

impl BackendCfg {
    pub fn local() -> Self {
        Self { id: "lokal".into(), label: "Dieser Rechner".into(), kind: "local".into(), url: String::new(), token: String::new(), token_env: String::new(), enabled: true }
    }
    pub fn hub(id: &str, label: &str, url: &str, token: &str) -> Self {
        Self { id: id.into(), label: label.into(), kind: "hub".into(), url: url.into(), token: token.into(), token_env: String::new(), enabled: true }
    }
    /// Baut das Backend; Hub-Konfiguration ohne URL ist ein Fehler.
    pub fn build(&self) -> Result<Arc<dyn Instances>, String> {
        match self.kind.as_str() {
            "local" => Ok(Arc::new(LocalInstances::new(&self.id))),
            "hub" => {
                if !(self.url.starts_with("http://") || self.url.starts_with("https://")) {
                    return Err(format!("{}: URL muss mit http:// oder https:// beginnen", self.label));
                }
                let token = if self.token_env.is_empty() { self.token.clone() } else { std::env::var(&self.token_env).unwrap_or_default() };
                Ok(Arc::new(HubInstances::new(&self.id, &self.label, &self.url, &token)))
            }
            k => Err(format!("unbekannte Backend-Art: {k}")),
        }
    }
}

pub fn config_path() -> Option<std::path::PathBuf> {
    crate::copi_config_dir().map(|d| d.join("backends.json"))
}

/// Liest die Liste; fehlt die Datei, gibt es genau das lokale Backend. Ein lokales Backend ist
/// immer enthalten (wird vorangestellt, falls die Datei es nicht nennt).
pub fn parse(text: &str) -> Result<Vec<BackendCfg>, String> {
    let mut v: Vec<BackendCfg> = serde_json::from_str(text).map_err(|e| format!("backends.json ungültig: {e}"))?;
    let mut seen = std::collections::HashSet::new();
    for b in &v {
        if !seen.insert(b.id.clone()) {
            return Err(format!("backends.json: doppelte id \"{}\"", b.id));
        }
    }
    if !v.iter().any(|b| b.kind == "local") {
        v.insert(0, BackendCfg::local());
    }
    Ok(v)
}

pub fn load() -> Result<Vec<BackendCfg>, String> {
    let Some(p) = config_path() else { return Ok(vec![BackendCfg::local()]) };
    match std::fs::read_to_string(&p) {
        Ok(t) => parse(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![BackendCfg::local()]),
        Err(e) => Err(e.to_string()),
    }
}

/// Atomar schreiben, nur für den Besitzer lesbar (enthält Tokens).
pub fn save(list: &[BackendCfg]) -> Result<(), String> {
    let p = config_path().ok_or("kein Konfigurationsordner")?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("json.new");
    std::fs::write(&tmp, serde_json::to_string_pretty(list).unwrap() + "\n").map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_is_always_present_and_ids_unique() {
        let v = parse(r#"[{"id":"s","label":"Server","kind":"hub","url":"http://h:1","token":"x"}]"#).unwrap();
        assert_eq!(v[0].kind, "local");
        assert_eq!(v[1].id, "s");
        assert!(v[1].enabled);
        assert!(parse(r#"[{"id":"a","label":"","kind":"local"},{"id":"a","label":"","kind":"local"}]"#).unwrap_err().contains("doppelte"));
    }

    #[test]
    fn build_rejects_bad_url_and_kind() {
        assert!(BackendCfg::hub("h", "H", "server:8787", "t").build().is_err());
        let mut c = BackendCfg::local();
        c.kind = "ssh".into();
        assert!(c.build().is_err());
        assert!(BackendCfg::hub("h", "H", "http://server:8787/", "t").build().unwrap().is_remote());
    }

    #[test]
    fn token_env_is_read_at_build_time() {
        let mut c = BackendCfg::hub("h", "H", "http://x:1", "");
        c.token_env = "PI_BACKEND_TEST_TOKEN_UNSET".into();
        assert!(c.build().is_ok());
    }

    #[test]
    fn roundtrip_save_load() {
        let home = std::env::temp_dir().join(format!("pi-backends-{}", std::process::id()));
        let _g = crate::test_support::EnvGuard::temp_home(&home);
        let l = vec![BackendCfg::local(), BackendCfg::hub("s", "S", "http://a:1", "tok")];
        save(&l).unwrap();
        assert_eq!(load().unwrap(), l);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(config_path().unwrap()).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&home);
    }
}
