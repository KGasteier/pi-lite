# Session-Bookmarks und Sortierung

Jede Session-Karte zeigt unten rechts dauerhaft ein kleines Bookmark-Symbol:
Umriss = nicht markiert, gefüllt und in Kategorienfarbe = markiert. Klick toggelt
die Markierung, ohne die Session zu öffnen. Per Tab erreichbar, Enter/Leertaste
aktivieren den Button; `aria-pressed` bildet den Zustand ab. Das Kontextmenü der
Session bietet die gleiche Aktion.

Die Kategorieauswahl bildet den gemeinsamen Seitenleistenkopf; darunter eine
kompakte Aktionszeile mit Neuer Session, Sortierung und Bookmark-Filter. Keine
doppelte sichtbare Kategorieüberschrift. Skills/Extensions/Agents/Harnesses nutzen
denselben Kopf mit kurzem Hinweis, ohne Sessionfunktionen.

Die kompakte Auswahl in der Sidebar-Überschrift sortiert nach **Zeit ↓** (neueste
Änderung zuerst) oder **Bookmarks** (markierte zuerst, innerhalb beider Gruppen
neueste zuerst). Der Lesezeichen-Toggle rechts daneben blendet alle nicht
markierten Sessions aus; erneuter Klick zeigt wieder alle (`aria-pressed`, Zustand
lokal gespeichert). Ohne Bookmarks erscheint ein kurzer Hinweis statt leerer Liste.

Auf Hover/Fokus zeigt die Karte links neben dem Bookmark ein dezentes SVG-Symbol:
Papierkorb (Löschen, rot beim Hover) bzw. Stift (Umbenennen) bei der aktiven Session.
Der Sortierknopf ist nur in der Sessions-Ansicht sichtbar. Beim Toggle bleibt
Fokus auf dem Bookmark-Button der betroffenen Session, auch nach Umsortieren.

Markierungen, Sortiermodus und Filter werden lokal in WebKit-localStorage gespeichert,
nicht in Sessiondateien oder beim Anbieter. Zuordnung über den Session-Dateipfad:
Umbenennen des Sessiontitels behält den Bookmark; Verschieben/Dateiumbenennen oder
Forken übernimmt ihn nicht. Keine Synchronisation zwischen Geräten. Beim Löschen
der WebKit-Appdaten gehen diese UI-Metadaten verloren.

Tests: `node scripts/test-session-bookmarks.cjs` prüft Sortierung, Toggle,
Persistenz, beschädigten Speicher und Fehler-Rollback. JS-Syntax, bestehende
History-/Preview-/Footer-Tests, Rust-Tests und Release-Build geprüft. Kein echter
WebKit-Klicktest durchgeführt; Symbolgröße/Platzierung nach Neustart live prüfen.

## Bookmark-Farben und Umbenennen

Jedes Bookmark kann eine von acht festen Farben haben (Blau/Grün/Gelb/Violett/Orange/Rot/
Türkis/Pink; Blau ist die Vorgabe). Die Farbe wird wie die Markierung selbst lokal
gespeichert (pi-lite: `localStorage`-Schlüssel `pi-lite.session-bookmark-colors.v1` in
`ui/session-bookmarks.js#colorOf/setColor`; copi: `Store::bookmark_colors` in `state.json`
via `Store::bookmark_color/set_bookmark_color` in `copi/app/src/store.rs`), bleibt bei
Umbenennen der Session erhalten und geht beim Entfernen des Bookmarks verloren (kein
verwaister Zustand). Die acht Hex-Codes/Namen sind zentral in `ui/bookmark-palette.js`
bzw. `copi/app/src/bookmark_palette.rs` definiert (bewusst identisch, mit Paritätstest).

Neben markierten Bookmarks öffnet der kleine Farbkreis die acht Standardfarben.
Der sichtbare Kopfbutton „Session umbenennen“ erlaubt einen neuen, merkbaren Titel
für die aktuelle Session. Andere Sessions können über das Kontextmenü bzw. R/F2
umbenannt werden. Die Funktionen sind in beiden Oberflächen eingebunden; reale
Bedienprüfung bleibt offen.

Tests: `node scripts/test-bookmark-colors.cjs` prüft die Palette (acht eindeutige Farben,
Index-Normalisierung, Parität zwischen `ui/bookmark-palette.js` und
`copi/app/src/bookmark_palette.rs`) sowie Setzen/Lesen/Aufräumen der Farbe in
`session-bookmarks.js` (Persistenz über einen Neustart, Entfernen eines Bookmarks räumt
seine Farbe auf, andere Bookmarks bleiben unberührt, beschädigter Farbspeicher zerstört
nicht die Bookmarks selbst). `cargo test -p copi --locked` prüft die Rust-Seite
(`Store`/`bookmark_palette`) analog.
