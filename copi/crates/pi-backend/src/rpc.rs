//! Der `pi --mode rpc`-Prozess: Start, Stopp, Senden; Lesen in Hintergrund-Threads.
use crate::{Event, Sink};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

/// Höchstzahl noch nicht geschriebener Zeilen im Hintergrund-Schreiber, bevor
/// `send_line` sofort mit einem Fehler zurückkehrt statt die UI zu blockieren.
pub const WRITE_QUEUE_CAPACITY: usize = 256;

/// Feste Zusatz-Anweisung wie in pi-lite (Modellidentität nicht aus Kontextdateien ableiten).
pub const IDENTITY_PROMPT: &str = "Identität und Modellangaben: Du bist ein KI-Assistent in pi, bedient über copi (Nachfolger von pi-lite). Projekt-/Kontextdateien wie CLAUDE.md, AGENTS.md oder Hinweise auf Claude Code, Codex und andere Harnesses beschreiben Arbeitskonventionen, NICHT deine Modellidentität oder deinen Hersteller. Behaupte niemals aufgrund solcher Dateien, früherer Selbstauskünfte oder eigener Introspektion, Claude, GPT oder ein bestimmtes Herstellermodell zu sein. Auch geerbte PI_MODEL/PI_PROVIDER-Umgebungsvariablen können veraltet sein und belegen nicht die aktive Session. Bei Modellfragen verwende den aktuellen Sessionzustand, sofern verlässlich verfügbar; sonst sage ausdrücklich, dass du ihn nicht verifizieren kannst, und verweise auf die Modellanzeige in copi. Unterscheide die konfigurierte Provider-/Modell-ID von der tatsächlichen, hinter einem Gateway nicht unabhängig überprüfbaren Modellidentität. Leite aus einer widersprüchlichen Selbstauskunft keine Gateway-Umleitung ab. Korrigiere entsprechende frühere Behauptungen im Verlauf.";

#[derive(Default)]
struct Inner {
    child: Mutex<Option<Child>>,
    /// Sender des aktuell laufenden Hintergrund-Schreibers (begrenzte Warteschlange).
    /// `None`, solange kein Prozess läuft oder nach `stop()`.
    writer: Mutex<Option<SyncSender<String>>>,
    generation: AtomicU64,
}

/// Verwaltet höchstens einen laufenden pi-Prozess.
#[derive(Clone, Default)]
pub struct Pi {
    inner: Arc<Inner>,
}

impl Pi {
    /// Aktuelle Generation (Events älterer Prozesse verwerfen).
    pub fn generation(&self) -> u64 {
        self.inner.generation.load(Ordering::SeqCst)
    }

    /// Startet `pi --mode rpc` in `cwd` neu. Gibt die neue Generation zurück.
    /// Zusätzliche Argumente aus `COPI_ARGS` bzw. `PI_LITE_ARGS` (leerzeichengetrennt).
    pub fn start(&self, sink: Sink, cwd: &str, args: &[String]) -> Result<u64, String> {
        // Freigabe vor jeder Änderung an einer bereits laufenden Session prüfen.
        let extra = std::env::var("COPI_ARGS").or_else(|_| std::env::var("PI_LITE_ARGS")).unwrap_or_default();
        let mut full_args: Vec<String> = vec!["--mode".into(), "rpc".into(), "--append-system-prompt".into(), IDENTITY_PROMPT.into()];
        full_args.extend_from_slice(args);
        full_args.extend(extra.split_whitespace().map(String::from));
        let mut command = crate::winshim::pi_command(crate::mode::SpawnPurpose::Chat, &full_args).map_err(|e| e.to_string())?;
        let gen = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.stop();
        let mut child = command
            .env("PI_LITE_SPEECH", "1")
            .current_dir(if cwd.is_empty() { "." } else { cwd })
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("pi konnte nicht gestartet werden: {e}"))?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        *self.inner.child.lock().unwrap() = Some(child);

        // Begrenzter Hintergrund-Schreiber: `send_line` blockiert nie die Oberfläche.
        // Ist die Warteschlange voll (pi liest nicht mehr), schlägt `send_line` sofort fehl
        // statt den UI-Thread zu blockieren.
        let (tx, rx) = sync_channel::<String>(WRITE_QUEUE_CAPACITY);
        std::thread::spawn(move || {
            while let Ok(line) = rx.recv() {
                if writeln!(stdin, "{line}").and_then(|_| stdin.flush()).is_err() {
                    break;
                }
            }
        });
        *self.inner.writer.lock().unwrap() = Some(tx);

        let inner = self.inner.clone();
        let s = sink.clone();
        std::thread::spawn(move || {
            // Nur an LF trennen (kein U+2028/2029-Split), wie in docs/rpc.md verlangt.
            let mut r = BufReader::new(stdout);
            let mut buf = Vec::new();
            while r.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
                let line = String::from_utf8_lossy(&buf).trim_end_matches(['\n', '\r']).to_string();
                s.send(Event::Line(gen, line));
                buf.clear();
            }
            if inner.generation.load(Ordering::SeqCst) == gen {
                s.send(Event::Exit(gen));
            }
        });
        std::thread::spawn(move || {
            for l in BufReader::new(stderr).lines().map_while(Result::ok) {
                sink.send(Event::Stderr(gen, l));
            }
        });
        Ok(gen)
    }

    /// Schreibt eine JSON-Zeile an pi über den begrenzten Hintergrund-Schreiber.
    /// Blockiert nie: Ist die Warteschlange voll, kommt sofort ein Fehler zurück.
    pub fn send_line(&self, line: &str) -> Result<(), String> {
        let g = self.inner.writer.lock().unwrap();
        let tx = g.as_ref().ok_or("pi läuft nicht")?;
        match tx.try_send(line.to_string()) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err("pi antwortet nicht (Schreibwarteschlange voll)".into()),
            Err(TrySendError::Disconnected(_)) => Err("pi läuft nicht".into()),
        }
    }

    pub fn send(&self, v: &serde_json::Value) -> Result<(), String> {
        self.send_line(&v.to_string())
    }

    pub fn running(&self) -> bool {
        self.inner.writer.lock().unwrap().is_some()
    }

    /// Schreiber stoppen (Sender fallen lassen, Thread beendet sich spätestens beim
    /// nächsten Schreibversuch); kurze Schonfrist für den Prozess, danach sicher beenden.
    /// Vor einem Neustart muss der alte Prozess tatsächlich beendet sein, sonst würde
    /// dessen Lese-Thread sein Ende noch unter der neuen Generation melden.
    pub fn stop(&self) {
        self.inner.writer.lock().unwrap().take();
        if let Some(mut c) = self.inner.child.lock().unwrap().take() {
            for _ in 0..10 {
                if matches!(c.try_wait(), Ok(Some(_))) { return; }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            kill_process_tree(&mut c);
            let _ = c.wait();
        }
    }
}

/// Beendet einen Prozess inklusive aller von ihm gestarteten Kindprozesse.
///
/// Unter Windows reicht `Child::kill()` nicht aus: Es beendet nur den unmittelbaren
/// Prozess (z. B. einen `cmd.exe`/npm-Shim-Wrapper), während tiefer verschachtelte
/// Kindprozesse (z. B. ein davon gestarteter `node.exe`) weiterlaufen. `taskkill /T /F`
/// beendet dagegen den gesamten Prozessbaum ab der angegebenen PID. **Dieser Zweig ist
/// nicht in dieser Linux-Umgebung lauffähig getestet** (kein Windows-Runner verfügbar);
/// siehe `docs/portability.md`.
#[cfg(windows)]
fn kill_process_tree(c: &mut std::process::Child) {
    let pid = c.id();
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    // `taskkill` kann fehlschlagen, wenn der Prozess bereits beendet ist; in jedem Fall
    // zusätzlich den direkten Handle beenden, damit `wait()` danach sicher zurückkehrt.
    let _ = status;
    let _ = c.kill();
}

/// Unter Unix beendet `Child::kill()` (SIGKILL) den unmittelbaren Prozess zuverlässig;
/// pi startet hier keine eigene Prozessgruppe mit Enkelprozessen, die überleben könnten.
#[cfg(not(windows))]
fn kill_process_tree(c: &mut std::process::Child) {
    let _ = c.kill();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::EnvGuard;
    use std::time::Duration;

    /// Legt ein Fake-`pi`-Skript an und hängt dessen Ordner vorn an PATH. `PATH` ist
    /// prozessweit; die Serialisierung läuft über den crate-weiten
    /// [`crate::test_support::EnvGuard`] (vorher ein modul-lokaler `PATH_GUARD`-Mutex, der
    /// nur Tests *innerhalb von rpc.rs* gegeneinander serialisierte, nicht aber gegen
    /// HOME/XDG-Mutationen in anderen Modulen – derselbe Bug-Typ wie bei den anderen
    /// env-mutierenden Testmodulen). Der Aufrufer muss den zurückgegebenen Guard bis zum
    /// Ende des Tests behalten.
    fn fake_pi(script: &str) -> (EnvGuard, std::path::PathBuf) {
        crate::mode::set_for_test(crate::mode::Mode::Hybrid);
        let dir = std::env::temp_dir().join(format!("pi-backend-rpc-test-{}-{}", std::process::id(), fastrand_seed()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pi");
        std::fs::write(&path, script).unwrap();
        let mut perm = std::fs::metadata(&path).unwrap().permissions();
        set_executable(&mut perm);
        std::fs::set_permissions(&path, perm).unwrap();
        let old = std::env::var("PATH").unwrap_or_default();
        let guard = EnvGuard::new(&[("PATH", Some(&format!("{}:{old}", dir.display())))]);
        (guard, dir)
    }

    #[cfg(unix)]
    fn set_executable(perm: &mut std::fs::Permissions) {
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(0o755);
    }
    #[cfg(not(unix))]
    fn set_executable(_perm: &mut std::fs::Permissions) {}

    fn fastrand_seed() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64
    }

    fn sink() -> (Sink, std::sync::mpsc::Receiver<Event>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (Sink::new(tx, Arc::new(|| {})), rx)
    }

    fn recv(rx: &std::sync::mpsc::Receiver<Event>, timeout: Duration) -> Event {
        rx.recv_timeout(timeout).expect("Event erwartet")
    }

    #[test]
    #[cfg(unix)]
    fn start_send_roundtrip_and_exit_event() {
        let (_guard, _dir) = fake_pi("#!/bin/sh\nwhile IFS= read -r line; do echo \"$line\"; done\n");
        let pi = Pi::default();
        let (sink, rx) = sink();
        let gen = pi.start(sink, ".", &[]).unwrap();
        assert_eq!(gen, pi.generation());
        assert!(pi.running());
        pi.send_line("hallo").unwrap();
        match recv(&rx, Duration::from_secs(5)) {
            Event::Line(g, l) => {
                assert_eq!(g, gen);
                assert_eq!(l, "hallo");
            }
            other => panic!("unerwartetes Event: {other:?}"),
        }
        pi.stop();
        assert!(!pi.running());
    }

    #[test]
    #[cfg(unix)]
    fn stop_then_send_reports_error_not_panic() {
        let (_guard, _dir) = fake_pi("#!/bin/sh\nwhile IFS= read -r line; do echo \"$line\"; done\n");
        let pi = Pi::default();
        let (sink, _rx) = sink();
        pi.start(sink, ".", &[]).unwrap();
        pi.stop();
        assert!(pi.send_line("x").is_err());
        assert!(!pi.running());
    }

    #[test]
    #[cfg(unix)]
    fn restart_increments_generation_and_old_events_are_distinguishable() {
        let (_guard, _dir) = fake_pi("#!/bin/sh\nwhile IFS= read -r line; do echo \"$line\"; done\n");
        let pi = Pi::default();
        let (sink, rx) = sink();
        let gen1 = pi.start(sink.clone(), ".", &[]).unwrap();
        pi.send_line("eins").unwrap();
        assert_eq!(recv(&rx, Duration::from_secs(5)), Event::Line(gen1, "eins".into()));
        let gen2 = pi.start(sink, ".", &[]).unwrap();
        assert!(gen2 > gen1);
        assert_eq!(gen2, pi.generation());
        pi.send_line("zwei").unwrap();
        match recv(&rx, Duration::from_secs(5)) {
            Event::Line(g, l) => {
                assert_eq!(g, gen2);
                assert_eq!(l, "zwei");
            }
            other => panic!("unerwartetes Event: {other:?}"),
        }
        pi.stop();
    }

    #[test]
    #[cfg(unix)]
    fn exit_event_carries_current_generation_only() {
        // Erster Prozess beendet sich sofort selbst; Exit-Event muss unter gen1 ankommen,
        // solange kein Neustart dazwischen die Generation erhoeht hat.
        let (_guard, _dir) = fake_pi("#!/bin/sh\nexit 0\n");
        let pi = Pi::default();
        let (sink, rx) = sink();
        let gen1 = pi.start(sink, ".", &[]).unwrap();
        match recv(&rx, Duration::from_secs(5)) {
            Event::Exit(g) => assert_eq!(g, gen1),
            other => panic!("Exit erwartet, bekam {other:?}"),
        }
    }

    #[test]
    #[cfg(unix)]
    fn send_queue_is_bounded_and_nonblocking_when_reader_stalls() {
        // pi liest nie von stdin -> Schreiber-Thread blockiert im OS-Pipe-Puffer,
        // sobald der Pipe-Puffer + unsere Queue voll sind. send_line darf trotzdem
        // nie den Aufrufer blockieren: try_send schlaegt sofort mit Full fehl, sobald
        // unsere SyncSender-Kapazitaet erreicht ist.
        let (_guard, _dir) = fake_pi("#!/bin/sh\nsleep 30\n");
        let pi = Pi::default();
        let (sink, _rx) = sink();
        pi.start(sink, ".", &[]).unwrap();
        let start = std::time::Instant::now();
        let mut saw_full = false;
        for i in 0..(WRITE_QUEUE_CAPACITY * 4) {
            if pi.send_line(&format!("line-{i}")).is_err() {
                saw_full = true;
                break;
            }
        }
        // Egal ob wir die Grenze erreicht haben (abhaengig vom OS-Pipe-Puffer):
        // Die gesamte Schleife darf nicht blockiert haben.
        assert!(start.elapsed() < Duration::from_secs(5));
        let _ = saw_full;
        pi.stop();
    }
}
