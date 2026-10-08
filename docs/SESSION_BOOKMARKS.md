# Session-Bookmarks und Sortierung

Jede Session-Karte zeigt unten rechts dauerhaft ein kleines Bookmark-Symbol:
Umriss = nicht markiert, gefüllt und in Kategorienfarbe = markiert. Klick toggelt
die Markierung, ohne die Session zu öffnen. Per Tab erreichbar, Enter/Leertaste
aktivieren den Button; `aria-pressed` bildet den Zustand ab. Das Kontextmenü der
Session bietet die gleiche Aktion.

Die kompakte Auswahl in der Sidebar-Überschrift sortiert nach **Zeit ↓** (neueste
Änderung zuerst) oder **Bookmarks** (markierte zuerst, innerhalb beider Gruppen
neueste zuerst). Keine Filterung: Nicht markierte Sessions bleiben sichtbar.
Der Sortierknopf ist nur in der Sessions-Ansicht sichtbar. Beim Toggle bleibt
Fokus auf dem Bookmark-Button der betroffenen Session, auch nach Umsortieren.

Markierungen und Sortiermodus werden lokal in WebKit-localStorage gespeichert,
nicht in Sessiondateien oder beim Anbieter. Zuordnung über den Session-Dateipfad:
Umbenennen des Sessiontitels behält den Bookmark; Verschieben/Dateiumbenennen oder
Forken übernimmt ihn nicht. Keine Synchronisation zwischen Geräten. Beim Löschen
der WebKit-Appdaten gehen diese UI-Metadaten verloren.

Tests: `node scripts/test-session-bookmarks.cjs` prüft Sortierung, Toggle,
Persistenz, beschädigten Speicher und Fehler-Rollback. JS-Syntax, bestehende
History-/Preview-/Footer-Tests, Rust-Tests und Release-Build geprüft. Kein echter
WebKit-Klicktest durchgeführt; Symbolgröße/Platzierung nach Neustart live prüfen.
