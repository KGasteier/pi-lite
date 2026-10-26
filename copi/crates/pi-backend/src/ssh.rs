//! SSH-Backend: Verbindung zu einem Server über das System-`ssh`. Beim ersten Verbinden wird der
//! pi-hub (`hub/server.mjs`, eingebettet) nach `~/.local/share/pi-hub` kopiert und nur an
//! `127.0.0.1` des Servers gestartet; ein `ssh -L`-Tunnel macht ihn lokal erreichbar. Das
//! Hub-Token wird auf dem Server erzeugt und nur im Speicher gehalten. Passwörter werden nie
//! abgefragt (BatchMode): Zugang nur per Schlüssel/ssh-agent. Tailscale-Namen funktionieren als Host.
use crate::backends::BackendCfg;
use crate::instances::*;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const HUB_JS: &str = include_str!("../hub/server.mjs");

#[derive(Clone, Debug, PartialEq)]
enum St {
    Setup,
    Ready,
    Failed(String),
}

pub struct SshInstances {
    inner: HubInstances,
    st: Arc<Mutex<St>>,
    stop: Arc<AtomicBool>,
    tunnel: Arc<Mutex<Option<Child>>>,
    cfg: BackendCfg,
}

impl Drop for SshInstances {
    /// Beendet den Tunnel sofort (nur diesen Kindprozess, per Handle) – sonst bliebe `ssh -L` als Waise.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(mut ch) = self.tunnel.lock().unwrap().take() {
            let _ = ch.kill();
            let _ = ch.wait();
        }
    }
}

fn ssh_bin() -> String {
    std::env::var("COPI_SSH").unwrap_or_else(|_| "ssh".into())
}

/// Einfache Anführungszeichen für die entfernte Shell.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Ziel `user@host` (oder nur `host`, z. B. ein Alias aus ~/.ssh/config).
pub fn target(c: &BackendCfg) -> String {
    if c.user.trim().is_empty() {
        c.host.trim().to_string()
    } else {
        format!("{}@{}", c.user.trim(), c.host.trim())
    }
}

fn base_args(c: &BackendCfg) -> Vec<String> {
    let mut a: Vec<String> = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=3"].map(String::from).to_vec();
    if !c.identity_file.trim().is_empty() {
        a.extend(["-i".into(), crate::tilde_expand(c.identity_file.trim()), "-o".into(), "IdentitiesOnly=yes".into()]);
    }
    if c.ssh_port != 0 {
        a.extend(["-p".into(), c.ssh_port.to_string()]);
    }
    a
}

pub fn hub_port(c: &BackendCfg) -> u16 {
    if c.hub_port == 0 { 8787 } else { c.hub_port }
}

/// Über `ssh host befehl` fehlt die Login-Umgebung: Versionsmanager (mise, asdf), npm-global und
/// ~/.local/bin stehen dann nicht im PATH, `pi` wäre für den Hub unsichtbar. Angehängt, damit ein
/// System-Node nicht überschattet wird.
pub const REMOTE_PATH: &str = r#"for d in "$HOME/.local/share/mise/shims" "$HOME/.asdf/shims" "$HOME/.local/bin" "$HOME/.npm-global/bin" "$HOME/.volta/bin" "$HOME/.bun/bin" "$HOME/.cargo/bin"; do [ -d "$d" ] && case ":$PATH:" in *":$d:"*) ;; *) PATH="$PATH:$d" ;; esac; done; export PATH"#;

/// Skript für die entfernte Shell: Hub installieren (stdin = server.mjs), starten, Token melden.
pub fn remote_script(port: u16) -> String {
    format!(
        r#"set -e
{path}
D="$HOME/.local/share/pi-hub"; mkdir -p "$D"
command -v node >/dev/null 2>&1 || {{ echo "ERR:node nicht gefunden" >&2; exit 3; }}
node -e 'process.exit(+process.versions.node.split(".")[0] < 20 ? 1 : 0)' || {{ echo "ERR:Node 20 oder neuer nötig" >&2; exit 3; }}
cat > "$D/server.mjs.new"
sd() {{ command -v systemctl >/dev/null 2>&1 && systemctl --user is-active -q pi-hub.service 2>/dev/null; }}
alive() {{ [ -f "$D/pid" ] && kill -0 "$(cat "$D/pid")" 2>/dev/null; }}
if [ -f "$D/server.mjs" ] && cmp -s "$D/server.mjs" "$D/server.mjs.new"; then rm -f "$D/server.mjs.new"; else
  mv "$D/server.mjs.new" "$D/server.mjs"
  if sd; then systemctl --user restart pi-hub.service; sleep 1
  elif alive; then kill "$(cat "$D/pid")" 2>/dev/null || true; sleep 1; fi
fi
if ! sd && ! alive; then
  HUB_HOST=127.0.0.1 HUB_PORT={port} nohup node "$D/server.mjs" >"$D/hub.log" 2>&1 </dev/null &
  echo $! > "$D/pid"
fi
T="${{XDG_CONFIG_HOME:-$HOME/.config}}/pi-hub/token"
i=0; while [ ! -s "$T" ] && [ "$i" -lt 50 ]; do sleep 0.2; i=$((i+1)); done
echo "TOKEN:$(cat "$T")"
"#,
        path = REMOTE_PATH
    )
}

/// Führt die Einrichtung aus und liefert das Hub-Token.
fn bootstrap(c: &BackendCfg) -> Result<String, String> {
    let mut cmd = Command::new(ssh_bin());
    cmd.args(base_args(c)).arg(target(c)).arg(format!("sh -c {}", sh_quote(&remote_script(hub_port(c)))));
    let mut ch = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("ssh nicht startbar: {e}"))?;
    let mut sin = ch.stdin.take().unwrap();
    let w = std::thread::spawn(move || {
        let _ = sin.write_all(HUB_JS.as_bytes());
    });
    let out = ch.wait_with_output().map_err(|e| e.to_string())?;
    let _ = w.join();
    parse_bootstrap(out.status.success(), &String::from_utf8_lossy(&out.stdout), &String::from_utf8_lossy(&out.stderr))
}

pub fn parse_bootstrap(ok: bool, stdout: &str, stderr: &str) -> Result<String, String> {
    if ok {
        if let Some(t) = stdout.lines().find_map(|l| l.strip_prefix("TOKEN:")) {
            let t = t.trim();
            if !t.is_empty() {
                return Ok(t.into());
            }
        }
        return Err("Server hat kein Hub-Token geliefert (hub.log auf dem Server prüfen).".into());
    }
    let e = stderr.trim();
    let hint = if e.contains("Permission denied") {
        " – Schlüssel-Login fehlt (ssh-copy-id im Terminal ausführen; Passwörter fragt copi nie ab)."
    } else {
        ""
    };
    let e = e.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" / ");
    Err(format!("SSH: {}{hint}", if e.is_empty() { "Verbindung fehlgeschlagen".to_string() } else { e }))
}

/// Ergebnis eines Abrufs (`fetch_dir`).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FetchReport {
    pub files: usize,
    /// Wegen falscher Architektur (x86-64 auf ARM64-Rechner) wieder entfernt.
    pub skipped_arch: Vec<String>,
    pub dest: String,
}

/// Verzeichnisse, die beim Abruf nie mitkommen (Build-Cache bleibt auf dem Server).
pub const FETCH_EXCLUDES: &[&str] = &["target/", "node_modules/", ".git/", "__pycache__/", ".cache/", "*.o", "*.a", "*.rlib", "*.so", "*.so.*", ".mise/"];

/// ELF-Architektur einer Datei (`e_machine`), sonst `None`.
pub fn elf_machine(path: &std::path::Path) -> Option<u16> {
    use std::io::Read;
    let mut b = [0u8; 20];
    std::fs::File::open(path).ok()?.read_exact(&mut b).ok()?;
    if &b[..4] != b"\x7fELF" {
        return None;
    }
    Some(if b[5] == 2 { u16::from_be_bytes([b[18], b[19]]) } else { u16::from_le_bytes([b[18], b[19]]) })
}

/// Maschinenkennung des laufenden Rechners (`e_machine`): 0xB7 = aarch64, 0x3E = x86-64.
pub fn local_machine() -> u16 {
    match std::env::consts::ARCH {
        "aarch64" => 0xB7,
        "x86_64" => 0x3E,
        "riscv64" => 0xF3,
        _ => 0,
    }
}

/// Entfernt ELF-Dateien fremder Architektur unterhalb von `dir`; Skripte und passende Binaries bleiben.
pub fn drop_foreign_elf(dir: &std::path::Path, machine: u16) -> (usize, Vec<String>) {
    let (mut kept, mut gone) = (0, Vec::new());
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                stack.push(e.path());
            } else if t.is_file() {
                match elf_machine(&e.path()) {
                    Some(m) if machine != 0 && m != machine => {
                        let _ = std::fs::remove_file(e.path());
                        gone.push(e.path().strip_prefix(dir).unwrap_or(&e.path()).to_string_lossy().into());
                    }
                    _ => kept += 1,
                }
            }
        }
    }
    gone.sort();
    (kept, gone)
}

/// Arbeitsordner (`remote_dir`) vom Server holen: `rsync` über dasselbe SSH (BatchMode, Schlüssel, Port),
/// ohne Build-Cache; danach werden x86-64-Binaries entfernt, weil sie hier nicht laufen.
/// Das Ziel liegt immer unter `dest_root` (kein Überschreiben fremder Ordner, kein `..`).
pub fn fetch_dir(c: &BackendCfg, remote_dir: &str, dest_root: &std::path::Path, machine: u16) -> Result<FetchReport, String> {
    if c.kind != "ssh" {
        return Err("Abruf gibt es nur für SSH-Server".into());
    }
    if !remote_dir.starts_with('/') || remote_dir.split('/').any(|p| p == "..") {
        return Err("ungültiger Serverpfad".into());
    }
    let name = std::path::Path::new(remote_dir).file_name().and_then(|n| n.to_str()).filter(|n| !n.is_empty()).unwrap_or("wurzel");
    let dest = dest_root.join(&c.id).join(name);
    std::fs::create_dir_all(&dest).map_err(|e| format!("Zielordner: {e}"))?;
    let rsh = std::iter::once(ssh_bin()).chain(base_args(c)).map(|a| sh_quote(&a)).collect::<Vec<_>>().join(" ");
    let mut cmd = Command::new(std::env::var("COPI_RSYNC").unwrap_or_else(|_| "rsync".into()));
    cmd.args(["-a", "-s", "--partial", "--no-owner", "--no-group", "-e", &rsh]);
    for x in FETCH_EXCLUDES {
        cmd.arg(format!("--exclude={x}"));
    }
    // rsync gibt den Pfad an die entfernte Shell weiter; mit `-s` (protect-args) werden Leer- und Sonderzeichen
    // dort NICHT ausgewertet – daher kein eigenes Quoting (das würde einen Ordner mit Anführungszeichen suchen).
    let src = format!("{}:{}/", target(c), remote_dir.trim_end_matches('/'));
    cmd.arg(src).arg(format!("{}/", dest.display()));
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { "rsync ist hier nicht installiert (z. B. `sudo pacman -S rsync`).".to_string() } else { e.to_string() })?;
    if !out.status.success() {
        let e = String::from_utf8_lossy(&out.stderr);
        return Err(format!("rsync: {}", e.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("fehlgeschlagen")));
    }
    let (files, skipped_arch) = drop_foreign_elf(&dest, machine);
    Ok(FetchReport { files, skipped_arch, dest: dest.to_string_lossy().into() })
}

/// Freien lokalen Port suchen.
fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0)).and_then(|l| l.local_addr()).map(|a| a.port()).unwrap_or(0)
}

fn tunnel_loop(c: BackendCfg, local: u16, stop: Arc<AtomicBool>, slot: Arc<Mutex<Option<Child>>>) {
    while !stop.load(Ordering::SeqCst) {
        let mut cmd = Command::new(ssh_bin());
        cmd.args(base_args(&c))
            .args(["-N", "-o", "ExitOnForwardFailure=yes", "-L"])
            .arg(format!("127.0.0.1:{local}:127.0.0.1:{}", hub_port(&c)))
            .arg(target(&c))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        {
            let mut g = slot.lock().unwrap();
            if stop.load(Ordering::SeqCst) {
                return;
            }
            if let Ok(ch) = cmd.spawn() {
                *g = Some(ch);
            }
        }
        wait_exit(&slot, &stop);
        for _ in 0..30 {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Wartet, bis der Tunnelprozess endet (oder `Drop` ihn entfernt hat).
fn wait_exit(slot: &Mutex<Option<Child>>, stop: &AtomicBool) {
    loop {
        {
            let mut g = slot.lock().unwrap();
            match g.as_mut().map(|c| c.try_wait()) {
                None | Some(Ok(Some(_))) | Some(Err(_)) => {
                    *g = None;
                    return;
                }
                Some(Ok(None)) => {}
            }
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

impl SshInstances {
    pub fn new(c: &BackendCfg) -> Result<Self, String> {
        if c.host.trim().is_empty() || c.host.contains(char::is_whitespace) || c.host.starts_with('-') {
            return Err(format!("{}: ungültiger Host", c.label));
        }
        if c.user.contains(char::is_whitespace) || c.user.starts_with('-') {
            return Err(format!("{}: ungültiger Benutzer", c.label));
        }
        let local = free_port();
        if local == 0 {
            return Err("kein freier lokaler Port".into());
        }
        let tok = Tok::default();
        let inner = HubInstances::with_tok(&c.id, &c.label, &format!("http://127.0.0.1:{local}"), tok.clone());
        let st = Arc::new(Mutex::new(St::Setup));
        let stop = Arc::new(AtomicBool::new(false));
        let tunnel: Arc<Mutex<Option<Child>>> = Default::default();
        let (c2, st2, stop2, slot) = (c.clone(), st.clone(), stop.clone(), tunnel.clone());
        std::thread::spawn(move || match bootstrap(&c2) {
            Ok(t) => {
                tok.set(&t);
                *st2.lock().unwrap() = St::Ready;
                tunnel_loop(c2, local, stop2, slot);
            }
            Err(e) => *st2.lock().unwrap() = St::Failed(e),
        });
        Ok(Self { inner, st, stop, tunnel, cfg: c.clone() })
    }

    /// Wartet auf die Einrichtung (höchstens 60 s) und prüft, dass der Tunnel antwortet.
    fn ready(&self) -> Result<(), String> {
        let t0 = Instant::now();
        loop {
            match self.st.lock().unwrap().clone() {
                St::Failed(e) => return Err(e),
                St::Ready => break,
                St::Setup => {}
            }
            if t0.elapsed() > Duration::from_secs(60) {
                return Err("SSH: Einrichtung dauert zu lange.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }
}

macro_rules! after_ready {
    ($s:ident, $e:expr) => {{
        $s.ready()?;
        $e
    }};
}

impl Instances for SshInstances {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn label(&self) -> String {
        self.inner.label()
    }
    fn is_remote(&self) -> bool {
        true
    }
    fn list(&self) -> Result<Vec<InstanceInfo>, String> {
        self.ready()?;
        // Der Tunnel braucht einen Moment nach dem Start.
        let mut last = Err(String::new());
        for _ in 0..40 {
            last = self.inner.list();
            if last.is_ok() || self.stop.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        last
    }
    fn create(&self, spec: &Spec) -> Result<InstanceInfo, String> {
        after_ready!(self, self.inner.create(spec))
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        after_ready!(self, self.inner.remove(id))
    }
    fn restart(&self, id: &str) -> Result<(), String> {
        after_ready!(self, self.inner.restart(id))
    }
    fn subscribe(&self, id: &str, sink: EvSink) -> Result<Subscription, String> {
        self.inner.subscribe(id, sink)
    }
    fn watch(&self, sink: EvSink) -> Result<Subscription, String> {
        self.inner.watch(sink)
    }
    fn harness(&self, id: &str) -> Result<Arc<dyn Harness>, String> {
        self.inner.harness(id)
    }
    fn files(&self) -> &dyn Files {
        self.inner.files()
    }
    fn fetch(&self, remote_dir: &str, dest_root: &std::path::Path, machine: u16) -> Result<FetchReport, String> {
        fetch_dir(&self.cfg, remote_dir, dest_root, machine)
    }
}

#[cfg(test)]
mod tests {
    /// Regression: `pi` aus mise/asdf/npm-global muss über die Nicht-Login-Shell von `ssh host cmd` auffindbar sein.
    #[test]
    fn remote_path_finds_pi_in_version_manager_shims() {
        use std::os::unix::fs::PermissionsExt;
        let home = std::env::temp_dir().join(format!("pi-path-{}", std::process::id()));
        let shims = home.join(".local/share/mise/shims");
        std::fs::create_dir_all(&shims).unwrap();
        let pi = shims.join("pi");
        std::fs::write(&pi, "#!/bin/sh\necho pi\n").unwrap();
        std::fs::set_permissions(&pi, std::fs::Permissions::from_mode(0o755)).unwrap();
        let run = |script: &str| {
            let o = std::process::Command::new("sh").arg("-c").arg(script).env("HOME", &home).env("PATH", "/usr/bin:/bin").output().unwrap();
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        };
        assert_eq!(run("command -v pi || echo FEHLT"), "FEHLT", "Ausgangslage: ohne Erweiterung unsichtbar");
        assert_eq!(run(&format!("set -e\n{REMOTE_PATH}\ncommand -v pi")), shims.join("pi").to_string_lossy());
        assert!(remote_script(1234).contains(REMOTE_PATH), "Bootstrap setzt den PATH vor dem Hub-Start");
        let _ = std::fs::remove_dir_all(&home);
    }

    use super::*;

    #[test]
    fn quoting_and_target() {
        assert_eq!(sh_quote("a'b"), r"'a'\''b'");
        let mut c = BackendCfg::ssh("s", "S", "194.95.203.142", "medienhaus");
        assert_eq!(target(&c), "medienhaus@194.95.203.142");
        c.user.clear();
        assert_eq!(target(&c), "194.95.203.142");
    }

    #[test]
    fn bootstrap_output_parsing() {
        assert_eq!(parse_bootstrap(true, "x\nTOKEN:abc\n", "").unwrap(), "abc");
        assert!(parse_bootstrap(true, "nix", "").is_err());
        assert!(parse_bootstrap(false, "", "x: Permission denied (publickey).").unwrap_err().contains("ssh-copy-id"));
        assert!(parse_bootstrap(false, "", "ERR:node nicht gefunden").unwrap_err().contains("node"));
    }

    #[test]
    fn rejects_option_injection_in_host_and_user() {
        let mut c = BackendCfg::ssh("s", "S", "-oProxyCommand=x", "u");
        assert!(SshInstances::new(&c).is_err());
        c.host = "h".into();
        c.user = "-oFoo".into();
        assert!(SshInstances::new(&c).is_err());
    }

    #[test]
    fn script_mentions_port_and_binds_loopback() {
        let s = remote_script(9911);
        assert!(s.contains("HUB_PORT=9911") && s.contains("HUB_HOST=127.0.0.1"));
    }
}
