# Korrekturen und Parität – 12. Oktober 2026

## Verbindliche Regel

Jede neue Funktion/Änderung wird für copi und pi-lite umgesetzt und getrennt geprüft. Plattformbedingt verschiedene Implementierungen sind zulässig, keine stillschweigend unterschiedlichen Funktionen. Bereits bestehende Unterschiede müssen inventarisiert werden; volle Parität ist noch nicht nachgewiesen.

## Auftrag / offene Prüfungen

- [x] Quellcode beider UIs: Akzentgefüllte invertierte Laufzeitanzeige statt fiktivem 95%-Plateau. Keine belastbare Gesamtfortschrittsmessung aus pi-RPC verfügbar.
- [x] Quellcode beider UIs: alphabetisches Slash-Menü oben; Auswahl übernimmt in Eingabe, führt nicht ungefragt aus.
- [x] Quellcode beider UIs: Seitenleisten-Symbol statt Hamburger.
- [x] Quellcode beider UIs: Material-Plus unter die Eingabe-Historienpfeile.
- [x] Bookmark-Farbbutton mit acht Standardfarben in beiden UIs und Persistenz.
- [x] Expliziter Button „Session umbenennen“ in beiden UIs.
- [x] Originaldateien standardmäßig referenzieren, keine Auswahlkopien nach DL_Omarchy. Agentenanweisung ergänzt; Agentenaktionen sind nicht technisch gesperrt.
- [x] Lokale Materialvorschau: PNG, JPEG/WebP (ImageMagick), Text/Markdown und erste PDF-Seite (Poppler), ohne automatische KI-Übertragung.
- [x] Materialbrowser: existierenden DL_Omarchy erkennen, Datum/Name-Sortierung.
- [x] Quellcode-Abgleich als docs/FEATURE_PARITY.md; tatsächliche verbleibende Lücken klar aufgeführt.
- [x] Erste Tranche geprüft: copi 120 Tests, pi-lite 12 Rust-Tests, bestehende JS-Tests, neuer Paritäts-Strukturtest und Media-Test erfolgreich. Beide Release-Builds erfolgreich; atomar in die tatsächlichen Launcher-Binaries installiert, Hashgleichheit geprüft. CPU-Headless-Langlog-Screenshot erfolgreich. Liveprüfung ausstehend.
- [x] Zweite Tranche (13.10.): Rust-/JS-Tests und Release-Builds erfolgreich, beide Launcher-Binaries atomar installiert, Hashgleichheit bestätigt. Lokale Vorschauen und Farben noch Nutzer-Liveprüfung ausstehend.

## Hängerdiagnose (vorläufig)

Screenshots zeigen „Application Not Responding“, nicht den Beweis eines Prozessabsturzes. Im verfügbaren Journal am 12.10., 15–21 Uhr keine passenden OOM-/Segfault-Einträge gefunden; coredumpctl ab 12.10. meldet keine Dumps. Fehlende Logs schließen Fehler nicht aus. Aktuell ca. 5,3 GiB RAM verfügbar; Root ca. 5,8 GiB frei; Shared Folder ca. 17 GiB frei, Anzeige 100% gerundet. Diese momentanen Werte beweisen nicht den Zustand beim Hänger. Keine Dateien gelöscht, keine Systemkonfiguration verändert.

Konkreter Codefund: Erzwungenes Scrollen ans Ende renderte potentiell die gesamte verbleibende Sitzung in einem Frame. Das kann bei langen Markdown-/Tool-Logs den UI-Thread blockieren. Quellcode geändert: nur virtualisierten Endbereich mounten. Kein Nachweis, dass dies sämtliche gemeldeten Hänger behebt. Grafik-/virgl-/Wayland-Probleme und andere Rust-Apps separat untersuchen. Keine laufenden Nutzerprozesse beenden.
