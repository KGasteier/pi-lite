# Funktionsabgleich pi-lite / copi

Stand: 13. Oktober 2026. Quellcode- und Testabgleich, **keine Behauptung vollständiger Live-Parität**. Unterschiedliche Darstellung/Toolkit ist zulässig; Verhalten, Zustimmung und Datenhaltung müssen gleichwertig sein.

| Funktionsbereich | pi-lite | copi | Status / Grenzen |
|---|---|---|---|
| pi-RPC, Streaming, Abbruch | Tauri/JSONL | Rust/JSONL | vorhanden; reale Abbruch-/Fehlerfälle weiter prüfen |
| Markdown, Thinking, Tool-/Shellblöcke | DOM/Markdown | md-core/md-egui | vorhanden; Layout/Textauswahl toolkitbedingt verschieden |
| Lange Sitzungen | DOM, Streaming-Debounce | variable Höhenvirtualisierung | funktional gleich, Performance nicht gleich; virgl-Hänger offen |
| Neue/öffnen/löschen/umbenennen Session | vorhanden | vorhanden | sichtbarer Umbenennen-Button und F2/R; neuer Titel persistiert via pi |
| Bookmark, Filter, Sortierung | localStorage | state.json | acht identische Farben; Speicherformate verschieden, kein automatischer Austausch |
| Workspace/Folders/VSCodium | vorhanden | vorhanden | Materialbrowser separat, Referenzen Datei:Zeile vorhanden |
| Modell/Thinking/Fork/Clone | vorhanden | vorhanden | verfügbare Modelle von pi; dynamische Extension-/RPC-Parität extern offen |
| Slash-Befehle | Autocomplete + Kopfmenü | Autocomplete + Kopfmenü | alphabetisch; Kopfmenü übernimmt ohne Ausführung |
| History/CLI/Bash | vorhanden | vorhanden | Tool-/CLI-Abbruch auf Zielplattform live prüfen |
| Inventar (Sessions/Extensions/Skills/Agents/Harnesses) | vorhanden | vorhanden | Navigation und Konfiguration lokal |
| Lokale Kurzinfo / KI-Zweck | vorhanden | vorhanden | explizite Zustimmung; Helferauswahl in copi, pi-lite bisher Umgebungs-Konfiguration: **Lücke** |
| Interner Editor / Schreibschutz / Konflikte | vorhanden | vorhanden | explizites Speichern, kein stiller Schreibzugriff |
| Material (Dateien/URLs) | vorhanden | vorhanden | Standard Originalreferenz; Auto-Modus optional; keine Auswahlkopien |
| Materialbrowser | gemeinsames Backend + DOM | gemeinsames Backend + egui | DL_Omarchy-Erkennung, Name/Datum, Mehrfachauswahl |
| Bild-/Text-/Markdown-/PDF-Vorschau | lokal | lokal | PNG direkt; JPEG/WebP via ImageMagick; PDF erste Seite via Poppler; optionale externe Werkzeuge |
| Medienanalyse | konfiguriert, Zustimmung | konfiguriert, Zustimmung | Adapter unterschiedlich; live Netzwerkübertragung nicht geprüft |
| Tokeninfos / KI-Erklärung | vorhanden | vorhanden | Zustimmung, nur Nutzungsstatistik; Helferauswahl-Lücke siehe oben |
| Sprache | vorlesen-Extension | vorlesen-Extension | gleiche lokale Extension; Linux/PipeWire; Audio live prüfen |
| Extension-Dialoge/Status/Widgets | vorhanden | vorhanden | RPC-API abgebildet, beliebige Extension-Kompatibilität nicht garantiert |
| Schriftgröße / Hilfe / Sidebar-Toggle | vorhanden | vorhanden | UI-Inhalte und Shortcuts weiter live vergleichen |
| Laufzeitstatus | invertierte Laufzeit | invertierte Laufzeit | keine fiktiven Gesamtprozente |
| Portabilität | Omarchy-Installer | Linux ARM64 verifiziert | macOS/Windows nicht ausgeführt, Release-Downloadinstallation geplant |
| Endpunkt-Assistent | Extension + lokale RPC-Adaption | Extension + lokale RPC-Adaption | keine autonome Provider-Migration; pi-udk-Kompatibilität offen |
| Tastatur, IME, Auswahl, Clipboard | vorhanden | vorhanden | keine vollständige reale Gleichwertigkeit nachgewiesen |

## Erkannte tatsächliche Lücken / nächste Aufgaben

1. Hilfsmodellwahl in pi-lite als UI-Funktion ergänzen (copi kann bereits Provider/Modell auswählen und speichern; pi-lite nutzt bisher env.sh).
2. Gemeinsamen Diagnosecheck für pi-Version/Agent-Verzeichnis/Extension-Modelle implementieren.
3. Gegenüberstellung mit identischen synthetischen Szenen und anschließende Nutzer-Liveprüfung (keine privaten Sessions in Testartefakte).
4. DOM-Langlog-Performance und native Render-/GPU-Stabilität getrennt untersuchen.
5. Textauswahl und Tastatur-/IME-Parität auf dem echten Zielsystem verifizieren.

## Untersuchung Kopierweg

Suche in Materialvorbereitung, Browser und Vorschau beider Frontends: kein `fs::copy`, `copy_file`, `copyfile` oder `shutil.copy` zur Materialablage gefunden. Die Auswahl gibt Originalpfade zurück; Referenzvorbereitung kanonisiert sie. Vorschau JPEG/WebP/PDF schreibt ausschließlich in einen privaten temporären Ordner und entfernt diesen danach. PNG/Text werden gelesen. Auswahl und Vorschau kopieren nichts nach DL_Omarchy.

Materialtext enthält jetzt die ausdrückliche Anweisung, Originaldateien nicht zu kopieren/verschieben und Änderungen nur auf Auftrag vorzunehmen. Agenten behalten ihre bestehenden Dateitools: **keine Sandbox-Garantie**. Ohne konkreten Tool-Aufruf/Log der beobachteten Kopie lässt sich deren Verursacher nicht belegen. Keine privaten Sessions ungefragt durchsucht.
