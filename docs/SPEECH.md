# Sprachausgabe

pi-lite nutzt die vorhandene lokale pi-Extension `vorlesen`, deren Piper-Stimme,
Regeln und Aktivierungszustand. Keine zweite TTS-Implementierung. 🔊/🔇 rechts unten
in der Tokenleiste schaltet Vorlesen um; ■ stoppt die laufende Ausgabe. Beide Knöpfe
sind per Tab/Enter erreichbar. Die zusätzliche Extension-Statuszeile „🔊 vorlesen“
wird ausgeblendet, da sie denselben Zustand ohne zusätzliche Funktion anzeigt.
Andere Extension-Statusmeldungen bleiben sichtbar. Ohne registrierten
`vorlesen`-Befehl bleiben sie deaktiviert.

Der Installer adaptiert die bekannte vorhandene Extension minimal: Sie darf neben
TUI auch RPC sprechen, ausschließlich wenn `PI_LITE_SPEECH=1` gesetzt ist. Dieses
Merkmal setzt pi-lite nur beim Hauptprozess, nicht bei Vorschau-Hilfsprozessen.
Die ursprüngliche Extension wird vorher gesichert. Unbekannte Versionen werden
nicht verändert; erneute Installation ist idempotent. Opt-out:
`PI_LITE_SETUP_SPEECH=0 ./scripts/install.sh`.

Es werden keine Audio-Pakete/Stimmen heruntergeladen und keine pi-Extension neu
installiert. Voraussetzung ist das bekannte lokale Setup in
`~/.pi/agent/extensions/vorlesen/index.ts`, Piper unter `~/.local/lib/piper/piper`,
ONNX-Stimmen unter `~/.local/share/piper/voices` und `pw-play` (Linux/PipeWire).
`PI_CODING_AGENT_DIR` wird vom Einrichtungshelfer beachtet; die vorhandene
Extension selbst verwendet weiterhin ihre bestehenden Home-Pfade.

Die Extension spricht kurze Antworten direkt, lange Antworten werden wie in pi
über das konfigurierte Zusammenfassungsmodell zusammengefasst (zusätzlicher
Modellaufruf/Kosten). Regeln und `state.json` bleiben unverändert. An/Aus über die
GUI verändert denselben persistenten Zustand wie `/vorlesen an|aus` in pi.

Geprüft: isolierter RPC-Start meldet `🔊 vorlesen` und registriert den Vorlesen-
Befehl; Syntax-, bestehende Logik-/Rust-Tests und Release-Build erfolgreich.
Tatsächlicher Audio-Output und GUI-Knöpfe noch nicht live geprüft. Keine Mac-/
Windows-Audio-Unterstützung durch dieses Linux-Setup.
