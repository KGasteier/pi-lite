# Gemeinsamer Teil für alle v3-Subagenten

Lies **vollständig** `/home/medienhaus/.pi/agent/skills/copidev/cocopi-v3-Entwicklungsprompt.md`
(Abschnitte 1–4 gelten für dich; deine Rolle steht in 5 und 8). Feste Entscheidungen E1–E6
nicht neu verhandeln. Gestaltungsregeln §3 gelten für jede Zeile.

Ortsangaben auf R2 (abweichend vom Prompt):
- Tutorial: `/home/medienhaus/cocopi-docs/remote-only-tutorial/cocopi-Remote-only-Mac-R2-Tutorial.md` (nur lesen).
- Orca read-only: `/tmp/orca-review` @ `05aab3a` (nichts installieren, nichts ausführen).
- pi-hub: `/home/medienhaus/Work/pi-hub` (nur lesen); S3 schreibt Hub-Code nur in `/home/medienhaus/Work/pi-hub-wt/s3`.
- Modellprovider heißt hier `udk2/…`.

Regeln:
- Du arbeitest in deinem eigenen Git-Worktree (cwd), Branch `v3/<rolle>`. **Nur Dateien deiner Rolle**
  (Tabelle §5). Fremde Dateien: Änderungsbitte in deinem Bericht.
- Umgebung: `export PATH=$HOME/.cargo/bin:$HOME/.local/share/mise/shims:$PATH`;
  eigenes Build-Verzeichnis `export CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-<rolle> CARGO_BUILD_JOBS=3`;
  nur `cargo … --offline`, keine Release-Builds, keine neuen Abhängigkeiten ohne Bitte.
- **Heartbeat:** mindestens alle 20 Minuten eine Zeile anhängen:
  `echo "$(date -Is) <auftrag-id> <was gerade>" >> /home/medienhaus/Work/pi-lite/coord/heartbeat/<rolle>`
- **Checkpoint:** nach jedem grünen Teilschritt im Worktree committen, nur eigene Pfade (`git add <pfade>`, nie `-A`),
  Autor = vorhandene git-Konfiguration, Nachricht Deutsch `cocopi v3 <auftrag-id>: …`. Kein Push, kein Merge,
  kein `reset --hard`, kein `stash`, keine Löschung fremder/ungetrackter Dateien. Nichts unter `docs/concepts/` stagen.
- Keine Prozesse anderer anfassen (pi, pi-hub-Dienst, copi/cocopi-Fenster); kein Zugriff auf `~/.config`-Echtdaten
  in Tests (Temp-Verzeichnisse). Keine Änderungen am System, an R2-Konten, systemd oder Netzwerk. Keine
  Internetzugriffe. Keine echten Modellaufrufe in Tests (nur Mocks).
- Idempotent: erst Zustand prüfen (git log/status im Worktree, vorhandene Dateien), dann handeln.
- **Abschlussbericht** als Datei `/home/medienhaus/Work/pi-lite/coord/runs/<auftrag-id>-bericht.md` (Deutsch, knapp):
  Commits, Dateien, Ergebnisse, Nachweis (Testname/Befehl/Screenshotpfad), geprüft/ungeprüft, offene Fragen,
  Änderungsbitten an andere Rollen. Nichts als fertig melden, was nicht nachgewiesen ist.
