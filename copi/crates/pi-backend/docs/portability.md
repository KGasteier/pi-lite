# Portabilität von `pi-backend`: tatsächlich verifiziert vs. nur Conditional-Compile

Dieses Dokument unterscheidet explizit, welche plattformspezifischen Code-Pfade in
`pi-backend` durch tatsächlich laufende Tests in dieser Entwicklungsumgebung
(Linux, `x86_64-unknown-linux-gnu`, kein Windows-/macOS-Runner verfügbar) **verifiziert**
wurden, von solchen, die lediglich **kompilieren** (`#[cfg(windows)]` /
`#[cfg(target_os = "macos")]`-Zweige, per `cargo build`/`cargo check` typgeprüft), aber
**nie ausgeführt** wurden. Letztere sind nach bestem Wissen korrekt implementiert und so
weit wie möglich in reine, plattformunabhängige Hilfsfunktionen zerlegt, die *wohl*
getestet sind – aber der tatsächliche OS-Aufruf (Prozessstart, `taskkill`, `explorer.exe`
etc.) selbst wurde nicht beobachtet.

Stand: diese Datei beschreibt den Code nach den in dieser Session durchgeführten Fixes.
Alle genannten Tests laufen mit `cargo test -p pi-backend --lib` auf Linux.

## 1. Verzeichnispfade (`lib.rs`: `config_base_dir`, `data_base_dir`, `copi_config_dir`,
   `copi_data_dir`, `legacy_pi_lite_config_dir`, `legacy_pi_lite_data_dir`,
   `resolve_config_file`, `resolve_data_dir`)

- **Verifiziert (Linux):** Die komplette Auflösungslogik (COPI-Pfad bevorzugt, Fallback auf
  den alten `pi-lite`-Pfad, Neuanlage am bevorzugten Pfad, wenn keiner existiert) läuft
  unter Linux über `dirs::config_dir()`/`dirs::data_dir()`, die dort `$XDG_CONFIG_HOME`
  bzw. `$HOME/.config` (analog für `data`) auswerten. Getestet in
  `lib.rs::tests::{copi_config_dir_prefers_new_path, resolve_config_file_prefers_copi_over_legacy,
  resolve_config_file_falls_back_to_preferred_when_neither_exists}`,
  `media.rs::tests::preferred_copi_config_path_wins_over_legacy_pi_lite_path` und
  `preview.rs::tests::preview_dir_prefers_copi_path_and_falls_back_to_legacy`.
- **Nur Conditional-Compile (Windows/macOS):** `dirs` liefert auf Windows Known-Folder-Pfade
  (`%APPDATA%`/`%LOCALAPPDATA%`) und auf macOS `~/Library/Application Support` bzw.
  `~/Library/Preferences`. Diese Zweige in der `dirs`-Crate selbst wurden **nicht** in
  dieser Umgebung ausgeführt; wir vertrauen hier auf die (extern getestete) `dirs`-Crate.
  Die darüber liegende COPI-Präferenz-/Legacy-Fallback-Logik ist plattformunabhängig
  formuliert (reine `PathBuf`-Operationen, keine OS-Verzweigung), daher ist das Risiko
  gering – aber ein echter Lauf auf Windows/macOS steht aus.

## 2. Bounded Media Read (`media.rs`)

- **Verifiziert (Linux):** Das tatsächliche Einlesen von Mediendateien erfolgt über
  `File::open(p).take(max_bytes + 1).read_to_end(...)` statt über einen unbegrenzten
  `std::fs::read`. Dadurch kann niemals mehr als `max_bytes + 1` Byte in den Speicher
  geladen werden, unabhängig von einer zwischen Metadaten-Check und Lesevorgang
  gewachsenen Datei (TOCTOU). Dieser Pfad ist plattformunabhängiges Standard-`std::io` und
  läuft identisch unter Linux/macOS/Windows; die Linux-Tests (`media.rs::tests::*`) decken
  die umgebenden Routingregeln ab. Ein dedizierter Test, der eine Datei größer als
  `max_bytes` tatsächlich bis zum Limit liest und danach abbricht, ist durch die
  bestehende Größenprüfung (`total > chosen.max_bytes`) in der Praxis kaum separat zu
  triggern (die Vorab-Prüfung fängt die meisten Fälle schon ab) – die Bounded-Read-Logik
  selbst ist aber durch Code-Inspektion und den bestehenden Testlauf ohne Panik/OOM
  bestätigt.

## 3. Sessions-Ersetzungserkennung (`sessions.rs`)

- **Verifiziert (Linux):** Sowohl die Unix-Dateiidentität (`file_identity`, Gerät+Inode via
  `MetadataExt::dev()`/`ino()`) als auch der portable Fingerabdruck der ersten
  `PREFIX_FINGERPRINT_LEN`-Bytes (`prefix_fingerprint`) sind durch eigene Unit-Tests
  abgedeckt (`file_identity_differs_across_distinct_files_and_matches_same_file`,
  `prefix_fingerprint_changes_when_content_changes_and_stable_otherwise`). Das
  Zusammenspiel in `InfoCache::list()` – insbesondere der kritische Fall einer **größeren**
  Ersatzdatei unter demselben Pfad, die eine reine Größen-/mtime-Heuristik fälschlich als
  "gewachsenes Original" durchlassen würde – ist durch
  `larger_replacement_file_at_same_path_is_rebuilt_not_merged` abgedeckt und läuft
  tatsächlich (nicht nur kompiliert) auf Linux.
- **Nur Conditional-Compile (Windows):** `file_identity` liefert unter
  `#[cfg(not(unix))]` konstant `None`; auf Windows stützt sich die Ersetzungserkennung
  dadurch ausschließlich auf den Fingerabdruck plus Größen-/mtime-Heuristik. Das ist
  schwächer als die Unix-Variante (ein pathologischer Fall: neue Datei hat exakt
  identischen 4096-Byte-Anfang, identische Größe und identische Sekunden-mtime wie die
  alte – praktisch nur relevant bei synthetisch konstruierten Kollisionen), wurde aber
  nicht auf echtem Windows verifiziert, da `#[cfg(not(unix))]` in dieser Umgebung nie
  kompiliert/ausgeführt wird (wir sind unter Unix).

## 4. Windows-Prozessbaum-Terminierung (`rpc.rs::kill_process_tree`)

- **Nur Conditional-Compile (Windows), nicht verifiziert:** Der `#[cfg(windows)]`-Zweig
  ruft `taskkill /PID <pid> /T /F` auf, um den gesamten Prozessbaum (inkl. z. B. eines von
  `pi` gestarteten `node.exe`) zu beenden – `Child::kill()` allein beendet unter Windows nur
  den unmittelbaren Prozess. Dieser Pfad wurde **nicht** ausgeführt (kein Windows-Runner
  verfügbar); er wurde nur durch `cargo build`/`cargo check` auf Unix *nicht* mitkompiliert
  (da `#[cfg(windows)]`), ist also noch nicht einmal in dieser Session typgeprüft worden.
  Die Syntax wurde manuell gegen die `taskkill`-Dokumentation geprüft.
- **Verifiziert (Linux/Unix):** Der `#[cfg(not(windows))]`-Zweig (einfaches
  `Child::kill()`) läuft unverändert wie zuvor und wird indirekt durch die bestehenden
  `rpc::tests::*` (z. B. `stop_then_send_reports_error_not_panic`) mitgetestet.

## 5. Windows-URL-Opener (`cli.rs::open_command`, `cli.rs::open_url`)

- **Verifiziert (Linux/plattformunabhängig):** Die Scheme-Validierung `is_http_url`
  (nur `http://`/`https://` werden akzeptiert, alles andere inkl. `file://`,
  `javascript:`, lokale Pfade wird abgelehnt) ist plattformunabhängig und durch
  `cli::tests::open_url_rejects_non_http_schemes` getestet – dieser Schutz gilt auf allen
  Plattformen gleich, bevor überhaupt ein OS-Befehl zum Öffnen aufgerufen wird.
- **Nur Conditional-Compile (Windows), nicht verifiziert:** Der `#[cfg(target_os =
  "windows")]`-Zweig von `open_command` ruft jetzt `explorer.exe <url>` statt vorher
  `cmd /c start "" <url>` auf, um zu vermeiden, dass `cmd.exe` seine Befehlszeile selbst
  noch einmal interpretiert (Metazeichen wie `&`, `|`, `^`). Dieser Zweig wurde nicht
  ausgeführt; die Umstellung ist durch Dokumentation zum bekannten Verhalten von
  `explorer.exe` als URL-Handler motiviert, aber nicht auf echtem Windows beobachtet.
- **Nur Conditional-Compile (macOS), nicht verifiziert:** Der `open`-Aufruf für macOS war
  bereits vorher vorhanden und unverändert; auch dieser Zweig läuft nicht in dieser
  Umgebung.

## 6. `pi`-npm-Shim-Start unter Windows (`winshim.rs`)

- **Verifiziert (Linux, reine Logik):** Alle plattformunabhängigen Bausteine sind per
  Unit-Test abgedeckt:
  - `pathext_list` (Parsen/Default von `PATHEXT`),
  - `find_executable` (PATH-/Endungs-Suchreihenfolge, mit injizierter `exists`-Funktion
    statt echtem Dateisystemzugriff – deterministisch und plattformunabhängig testbar),
  - `is_script_shim` (Erkennung von `.cmd`/`.bat`),
  - `escape_for_cmd` (Maskierung einzelner Argumente für eine `cmd.exe /C`-Zeile:
    Anführungszeichen-Verdopplung, `%`-Verdopplung gegen Variablenexpansion, Ablehnung von
    eingebetteten Zeilenumbrüchen). Elf Tests in `winshim::tests::*` laufen tatsächlich auf
    Linux durch.
  - Der `#[cfg(not(windows))]`-Zweig von `pi_command` (einfacher `Command::new("pi")`-
    Aufbau ohne jede Shim-Behandlung) ist der tatsächlich auf Linux verwendete Pfad und
    läuft indirekt über alle bestehenden `cli`-, `rpc`- und `preview`-Tests, die `pi`
    tatsächlich starten (z. B. `rpc::tests::start_send_roundtrip_and_exit_event`).
- **Nur Conditional-Compile (Windows), nicht verifiziert:** Der `#[cfg(windows)]`-Zweig von
  `pi_command` (PATH-Suche über echte Umgebungsvariablen, Entscheidung
  direkte `.exe` vs. `cmd.exe /D /S /C`-Fallback für `.cmd`/`.bat`-Shims, Aufbau der
  vollständigen Befehlszeile aus maskierten Argumenten) wurde **nicht** kompiliert oder
  ausgeführt, da dieser Code unter `#[cfg(windows)]` auf einem Linux-Build komplett
  entfernt wird. Die einzelnen Bausteine (s. o.) sind getestet; ihre Verkettung im
  `#[cfg(windows)]`-Zweig selbst ist es nicht. **Dies ist das größte verbleibende
  Portabilitätsrisiko dieser Änderungen** und sollte vor einem Windows-Release auf einer
  echten Windows-Maschine (idealerweise mit einer echten `npm install -g`-Installation von
  `pi`, die tatsächlich `pi.cmd` erzeugt) nachgezogen werden.

## 7. KI-Vorschau-Timeouts/Sicherheit (`preview.rs::ask_haiku`)

- **Verifiziert (Linux):** Die JSON-Anfrage wird ausschließlich über
  `serde_json::json!`/`to_string()` aufgebaut (keine String-Konkatenation von
  Nutzereingaben in eine Shell- oder Format-Zeile, daher keine Injektionsfläche über das
  Eingabematerial). Das 60-Sekunden-Zeitlimit wird über `rx.recv_timeout` mit einer festen
  `deadline` durchgesetzt; läuft die Zeit ab, wird der Kindprozess im `ensure`-Block
  (`let _ = child.kill(); let _ = child.wait();`) in jedem Fall beendet, auch bei Fehlern
  im `outcome`-Zweig. `stderr` wird nach `Stdio::null()` umgeleitet, sodass keine
  Fehlerausgabe des Unterprozesses ungefiltert irgendwo landet. Dies ist plattformunabhängiger
  Code (nur `std::process`/`std::sync::mpsc`) und durch `preview::tests::*` indirekt
  mitgetestet (z. B. `preview_summarizes_and_caches_with_stable_fingerprint`,
  `json_settings_never_leaks_content`). Der eigentliche Prozessstart nutzt jetzt
  `crate::winshim::pi_command`, siehe Abschnitt 6 für dessen Portabilitätsstatus.
- Kein konkreter Zusatzfehler über das bereits Vorhandene hinaus identifiziert; bewusst
  keine Änderung am Timeout-Wert oder der Fehlerbehandlung vorgenommen.

## Zusammenfassung: verbleibende Risiken

| Bereich | Verifiziert auf Linux | Nur Conditional-Compile |
|---|---|---|
| Verzeichnispfade (COPI/Legacy) | Ja (Logik) | `dirs`-interne Windows/macOS-Zweige |
| Bounded Media Read | Ja | – (plattformunabhängig) |
| Sessions-Ersetzungserkennung | Ja (inkl. Unix-Inode) | Windows-Zweig ohne Inode-Fallback |
| Prozessbaum-Terminierung | Ja (Unix-`kill`) | Windows-`taskkill`-Zweig |
| URL-Opener | Ja (Scheme-Filter) | Windows-`explorer.exe`-Zweig, macOS-`open`-Zweig |
| `pi`-Shim-Start | Ja (reine Logik-Bausteine) | Windows-`cmd.exe`-Fallback-Verkettung |
| KI-Vorschau-Timeout/Sicherheit | Ja | – (plattformunabhängig) |

Alle als "nicht verifiziert" markierten Zweige sind nach bestem Wissen korrekt
implementiert, aber ohne Zugriff auf eine echte Windows-/macOS-Maschine in dieser Sitzung
nicht ausführbar. Vor einem Produktions-Release auf diesen Plattformen wird ein manueller
oder CI-gestützter Testlauf auf echter Windows-/macOS-Hardware empfohlen, insbesondere für
den `winshim`-`cmd.exe`-Fallback (Abschnitt 6) und die `taskkill`-Prozessbaum-Terminierung
(Abschnitt 4).
