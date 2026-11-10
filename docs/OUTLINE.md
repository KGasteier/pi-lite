# Gemeinsame Gliederung in copi, cocopi-v2 und pi-lite

## Bedienung

Im Kontextmanager **Gliederung** einschalten. **Detail** ist die Standardansicht:
vollständige Sessionkarten mit Aktionen und Vorschau. **Kompakt** zeigt schmale Zeilen.
Beide Ansichten sind gleichermaßen bearbeitbar: Ziehen, Strukturtasten, Umbenennen
und Gliederungs-Kontextmenü stehen in beiden zur Verfügung. Die frühere Trennung
**Edit/View** bezeichnet jetzt nur noch die Darstellung, nicht die Bearbeitbarkeit.
In Detail sind ganze Karten um 9 px je Ebene eingerückt;
Sessionkarten mit Kindern tragen den Auf-/Zuklapppfeil direkt links im Titelbereich
(keine separate „Teilbaum“-Zeile). Pfeilklick klappt nur die Untereinträge, die
restliche Karte öffnet weiterhin die Session.
Farboberkategorien bleiben links bündig. Die Karten werden entsprechend schmaler,
nicht inhaltlich reduziert. Ab Ebene 5 bleibt die Einrückung auf 45 px begrenzt,
damit auch tiefe Gliederungen bedienbare Karten behalten (native und Web-Ansicht).
Alle Bookmark-Farben sind automatisch Oberkategorien, zusätzlich **Unmarkiert**.
Farbige Trennstreifen klappen die jeweilige Gruppe auf/zu; Unterkategorien bleiben
möglich. **+ Farbe** fragt Name und Hex-Farbe (#RRGGBB) ab und hängt einen neuen
Palette-Eintrag an, ohne bestehende Indizes zu verändern (max. 255 Farben).

Vorhandene Sessions und Inventar-Markdown-Dateien werden automatisch einsortiert.
Bestehende manuelle Kategorien bleiben unter Unmarkiert erhalten; extern geänderte
Bookmark-Zuordnungen werden beim Refresh übernommen. Einträge oder Teilbäume in eine
Farbkategorie verschieben setzt deren Bookmark-Farbe; Unmarkiert entfernt die
Markierung. Farboberkategorien sind geschützt gegen Entfernen oder Verschieben.
Bearbeitung der Namen per F2/Kontextmenü. Unterbäume behalten manuelle Reihenfolge.
Neue, noch nicht übernommene Inventare können kurz in den bisherigen losen Gruppen
erscheinen, bis der periodische Abgleich erfolgt.

- Kleine Dreiecke klappen Teilbäume auf/zu; Punkte markieren Sessions, kleine
  Rechtecke Markdown-Dateien. Die gesamte Zeile kann gezogen werden.
- Ziehen sortiert Teilbäume um; waagrechte Position bestimmt die Einrückung.
  Die Einfügelinie zeigt die Zielebene. Escape bricht ab.
- ↑/↓, Home/End wählen; ←/→ klappen zu/auf bzw. gehen zum Elternteil/ersten Kind.
- Tab / Umschalt+Tab rücken ein/aus; Tab übernimmt auch einen losen Eintrag.
- Alt+↑/↓ verschieben unter Geschwistern einschließlich ihrer Nachfahren.
- N legt eine Kategorie danach an, Umschalt+N eine Unterkategorie.
- F2/R benennt Kategorien bzw. Sessions um; Enter öffnet einen Eintrag.
- Entf/Backspace entfernt nur den Verweis. Kategorien benötigen eine Bestätigung;
  ihre Kinder rücken eine Ebene auf. Dateien und Sessions werden **nicht gelöscht**.
- Kontextmenü: dieselben Strukturaktionen; bei losen Einträgen zusätzlich
  „In Kategorie“ zur direkten Zuordnung ohne Ziehen.

In copi/cocopi gelten Baumtasten nur ohne Textfokus und ohne modalen Dialog/Popup.
In pi-lite muss eine Baumzeile fokussiert sein. Markdown öffnet in beiden Apps den
vorhandenen Editor; copi bietet zusätzlich die bestehende Vorschau bei Einzelklick.
Keine freie Dateisystemsuche: nur Markdown aus dem Ressourceninventar, keine beliebigen
Dateien außerhalb dessen Zugriffsregeln. Die Session-/Bookmarkfilter der alten
Sessions-Ansicht schränken die Gliederung nicht ein. Remote-Sessions erscheinen,
soweit die Backend-Verbindungen sie bereits geliefert haben.

## Architektur und gemeinsame Ablage

`shared/outline.rs` ist Modell und Änderungslogik für beide Frontends. Flache Liste
mit Ebenen (AUGMOST-artige Teilbäume), stabile Kategorie-IDs, Session-Schlüssel wie
bei gemeinsamen Bookmarks (`remote:<backend>:<pfad>` für Remote-Sessions).

Datei: `<config>/pi-frontends/outline.json`, neben `bookmarks.json`.
Kein Verschieben echter Dateien; keine neue Server-API. Die bisherige Sessions-
Ansicht bleibt verfügbar. Zusätzliche Palette: `extra_colors` in `bookmarks.json`;
zehn Standardfarben bleiben unverändert. Ältere Binaries kennen Zusatzindizes nicht:
für gemeinsame Bearbeitung die aktualisierten Frontends verwenden.

Speichern: Dateisperre, Revision vergleichen, temporär schreiben, fsync und atomar
umbenennen. Fremde Änderungen lösen einen Konflikthinweis samt Neuladen aus, statt
fremde Sortierungen still zu überschreiben. Auch Dialoge behalten die Revision ihres
Ausgangsstands. Unlesbare Dateien werden nicht überschrieben. Aktive Ansichten lesen
fremde Änderungen periodisch ein. Es gibt noch kein Struktur-Undo/Redo.

- Native Oberfläche: `copi/app/src/outline_ui.rs`.
- Web-Oberfläche: `ui/outline.js`, verwendet `outline_read` / `outline_apply` in Tauri.
- Operationen und Dateipersistenz werden in Rust getestet; JS enthält nur Zeilen-
  ableitung, Darstellung und Interaktion, keine zweite Strukturänderungslogik.

## Backends-Schalter

Der aktive Backends-Schalter von cocopi zeigt statt „×“ eine gemalte Sprechblase mit
Rückpfeil. Es bedeutet **zurück zu Chats/Kontakten/Räumen/Agenten**, nicht schließen.
Der pi-lite-Rasterschalter zeigte bereits ein Raster statt „×“ und bleibt erhalten;
„×“ beim Entfernen einzelner Rasterkacheln hat weiterhin seine andere Bedeutung.

## Prüfstand

**Aktuell (Kompakt/Detail):** cocopi 135 Tests bestanden / 2 ignoriert;
Strukturtasten explizit in beiden Darstellungen, Detailkarte auf Ziehbarkeit geprüft.
24 JS-Skripte bestanden, bekannter Altfehler `test-statusline-net.cjs` unverändert.
Native und Web-Detailscreenshots mit Demodaten angesehen. cocopi und pi-lite auf
ARM offline/locked/release gebaut, atomar installiert und vollständig hashgeprüft.
Details und Backups in `HANDOFF.md`. Kein echter Maus-/WebKit-/Remote-Livetest.
Die folgenden älteren Testzahlen beschreiben die vorherigen Ausbaustufen.

- Neuer DOM-Interaktionstest `node scripts/test-outline-browser.cjs`: Detail-Standard,
  beide Ansichten mit Fokus, Strukturtasten, Umbenennen, Kontextmenü und Drag-and-drop;
  kein Ebenensprung beim Anziehen und kein versehentliches Öffnen nach dem Drop.
  Chromium mit isolierten Demodaten, keine echte Tauri-/WebKit-Abnahme.
- Historischer Prüfstand vor Kompakt/Detail: copi 123 bestanden / 2 ignoriert; cocopi 127 bestanden / 2 ignoriert.
  Enthalten sind Modell-/Dateikonflikttests sowie Zeilen-, Tastatur-, Dropziel- und
  Dialogregressionen. Keine echten privaten Sessiondateien werden verändert.
- `node scripts/test-outline.cjs` und JS-Syntaxchecks bestanden. Bestehende JS-Suite:
  nur `test-statusline-net.cjs` scheitert am bereits in HEAD entfernten historischen
  `NET_PLACEHOLDER`; kein neuer Gliederungsfehler. Dieser Alt-Test ist offen.
- pi-lite `cargo check --offline --locked` bestanden. Dafür alten API-Bruch in
  `src-tauri/src/net.rs` korrigiert: `Spec.args` wird als leere Liste initialisiert.
- Native Headless-Demos `COPI_SCENE=outline-edit` und `outline-view` angesehen.
  Web-Code syntax-/compile-geprüft; neue Edit/View-Webdarstellung noch nicht live angesehen.
  Frühere isolierte Chromium-Seitenleiste bezieht sich auf die Vorgängerversion.
  Kein echter WebKit-/Backend-Test.
- Release/Installation: siehe aktuellen Nachtrag in `HANDOFF.md`.
- Offen: reale Maus-/Tastaturbedienung, langes Ziehen mit Scrollen, gleichzeitige
  Mehrfensterbedienung, WebKit-Livetest und echte Remote-Sessions. Automatisierte
  Tests ersetzen diese Prüfungen nicht. Keine v3- oder R2-Übernahme erfolgt.
