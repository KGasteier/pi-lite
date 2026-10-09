//! Der `pi --mode rpc`-Prozess: Start, Stopp, Senden; Lesen in Hintergrund-Threads.
use crate::{Event, Sink};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Feste Zusatz-Anweisung wie in pi-lite (Modellidentität nicht aus Kontextdateien ableiten).
pub const IDENTITY_PROMPT: &str = "Identität und Modellangaben: Du bist ein KI-Assistent in pi, bedient über copi (Nachfolger von pi-lite). Projekt-/Kontextdateien wie CLAUDE.md, AGENTS.md oder Hinweise auf Claude Code, Codex und andere Harnesses beschreiben Arbeitskonventionen, NICHT deine Modellidentität oder deinen Hersteller. Behaupte niemals aufgrund solcher Dateien, früherer Selbstauskünfte oder eigener Introspektion, Claude, GPT oder ein bestimmtes Herstellermodell zu sein. Auch geerbte PI_MODEL/PI_PROVIDER-Umgebungsvariablen können veraltet sein und belegen nicht die aktive Session. Bei Modellfragen verwende den aktuellen Sessionzustand, sofern verlässlich verfügbar; sonst sage ausdrücklich, dass du ihn nicht verifizieren kannst, und verweise auf die Modellanzeige in copi. Unterscheide die konfigurierte Provider-/Modell-ID von der tatsächlichen, hinter einem Gateway nicht unabhängig überprüfbaren Modellidentität. Leite aus einer widersprüchlichen Selbstauskunft keine Gateway-Umleitung ab. Korrigiere entsprechende frühere Behauptungen im Verlauf.";

#[derive(Default)]
struct Inner {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
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
        // Erst Generation erhöhen, dann alten Prozess beenden – sonst meldet dessen
        // Lese-Thread sein Ende noch unter der aktuellen Nummer.
        let gen = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.stop();
        let extra = std::env::var("COPI_ARGS").or_else(|_| std::env::var("PI_LITE_ARGS")).unwrap_or_default();
        let mut child = Command::new("pi")
            .args(["--mode", "rpc"])
            .env("PI_LITE_SPEECH", "1")
            .args(["--append-system-prompt", IDENTITY_PROMPT])
            .args(args)
            .args(extra.split_whitespace())
            .current_dir(if cwd.is_empty() { "." } else { cwd })
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("pi konnte nicht gestartet werden: {e}"))?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        *self.inner.stdin.lock().unwrap() = child.stdin.take();
        *self.inner.child.lock().unwrap() = Some(child);

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

    /// Schreibt eine JSON-Zeile an pi.
    pub fn send_line(&self, line: &str) -> Result<(), String> {
        let mut g = self.inner.stdin.lock().unwrap();
        let w = g.as_mut().ok_or("pi läuft nicht")?;
        writeln!(w, "{line}").and_then(|_| w.flush()).map_err(|e| e.to_string())
    }

    pub fn send(&self, v: &serde_json::Value) -> Result<(), String> {
        self.send_line(&v.to_string())
    }

    pub fn running(&self) -> bool {
        self.inner.stdin.lock().unwrap().is_some()
    }

    /// stdin schließen; kurze Schonfrist, danach sicher beenden und aufräumen.
    /// Vor einem Neustart muss der alte Prozess tatsächlich beendet sein.
    pub fn stop(&self) {
        self.inner.stdin.lock().unwrap().take();
        if let Some(mut c) = self.inner.child.lock().unwrap().take() {
            for _ in 0..10 {
                if matches!(c.try_wait(), Ok(Some(_))) { return; }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
