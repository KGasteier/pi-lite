# Tokenzeile

Unten zeigt pi-lite die echten `get_session_stats`-Werte nach Zustandsaktualisierungen
(Start, Sessionwechsel, abgeschlossener Agentlauf). Kein kontinuierliches Streaming
oder Timer-Polling. Anzeige: ↑ Input, ↓ Output, R Cache-Read, CH Cachequote,
aktuelle Kontextbelegung/maximales Kontextfenster, (auto) bei aktivierter Kompaktierung.

Kumulative Zahlen umfassen die ganze Session einschließlich Tool-Usage und
Zusammenfassungen. CH wird als Cache-Read/(Input+Cache-Read+Cache-Write) berechnet.
Kontextwerte kommen ausdrücklich aus `contextUsage`, nicht aus kumulierten Tokens.
Unbekannte Kontextbelegung nach Kompaktierung erscheint als ?. Keine Kostenschätzung.
Das kompakte Layout ist pi-ähnlich; exakte Rundungs-/Cachequotengleichheit zur TUI
wurde noch nicht live verglichen.

ⓘ öffnet ein tastaturbedienbares Kontextfenster mit eingefrorenem Snapshot und
integrierter Zustimmung. Haiku erhält nur Anzeige, Tokenzahlen, Kontextgröße,
Modell-ID und Kompaktierungsstatus, niemals Sessionpfad oder Chatinhalte. Es läuft
isoliert, ohne Tools/Extensions/Context/Session, mit 60 Sekunden Timeout. Kein
automatischer KI-Aufruf, keine persistente Archivierung der Erklärung. Esc/× schließt.

Syntax, bestehende History-/Vorschau-/Rust-Tests und Release-Build erfolgreich.
Neue Tokenzeile, Erklärung und echter Modellaufruf noch nicht live geprüft.
