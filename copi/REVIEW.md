# Codeprüfung: copi

## Korrigiert

- Figtree Regular/Bold direkt eingebettet: reproduzierbare UI-Schrift ohne Systeminstallation oder Laufzeit-Download. Quelle Google Fonts, SIL OFL in `app/assets/fonts/OFL.txt`; statische Schnitte aus der offiziellen variablen Schrift erzeugt.
- Seitenleisten-Auswahl volle Breite/30 px; Werkzeugzeile 28 px, 240-px-Seitenleiste, Trennlinie und Kartenabstände am Original ausgerichtet.
- Workspace-Auswahl und Editor bilden eine Gruppe; Kopfzeile wächst und bricht bei kleinen Fenstern um statt Elemente abzuschneiden.
- Bestätigungsdialog: Pfeiltasten wechseln Ja/Nein, Enter bestätigt die markierte Wahl. Hintergrundklick verwirft keine Eingaben.
- Extension-Dialoge antworten auch nach Timeout mit Abbruch.
- Normale Prompts während CLI-/Shell-Jobs werden abgewiesen, ohne den Eingabetext zu löschen. Session-Neustart während CLI-Jobs wird verhindert (zuvor ging die Abschlussmeldung verloren).
- Session-Cache berücksichtigt Nanosekunden und Dateigröße statt nur Sekunden; verschwundene Einträge werden entfernt.
- Prozessende: kurze Schonfrist vor SIGKILL. CLI-Abbruch eskaliert nach 300 ms im Hintergrund und beendet die Prozessgruppe statt nur den Wrapper.
- Alt+2/Strg+B schaltet Seitenleiste um.

## Noch offen / gezielt getrennt angehen

1. Log-Virtualisierung mit variablen Elementhöhen. Der vollständige Verlauf wird pro Repaint aufgebaut; egui cached Schrift-Galleys intern, aber Widget- und Markdown-Aufbau bleibt Arbeit. Belastung mit langen synthetischen Sessions messen, bevor ein zweiter Layout-Cache eingeführt wird.
2. Sessiondateien inkrementell lesen (Offset, Trunkierung/Ersetzung, unvollständige letzte Zeile beachten). Aktuell vollständiger Scan bei Änderung.
3. RPC-Schreibthread mit begrenzter Queue und sauberer Generationstrennung: synchrone Pipe-Writes könnten bei hängendem pi die UI blockieren. Nicht einfach unbegrenzt puffern.
4. Persistenz debouncen; späte Antworten auf bereits abgelaufene RPC-Anfragen gezielt behandeln.
5. Streaming-Markdown nur im Frame-Takt parsen; derzeit bereits durch die UI gebündelt, aber volle Nachricht wird bei geändertem Text neu geparst. Inkrementeller Parser braucht Tests für offene Code-Fences/Tabellen/Listen.
6. Echtfenster-Prüfung von IME, Auswahl, Tastenkürzeln, Dialog-Fokus/Tab und virgl. Fensterlose Screenshots sind kein Interaktionstest und beweisen keine RPC-/Prozessstabilität.

Keine Änderungen am installierten pi-lite oder an Hyprland. Keine privaten Daten oder Screenshots im Repository.
