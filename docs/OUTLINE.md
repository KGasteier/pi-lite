# Gemeinsame Gliederung in copi, cocopi-v2 und pi-lite

## Bedienung

Links in der Ansichtsauswahl **Gliederung** wählen. Mit **+ Kategorie** lassen sich
benennbare Oberkategorien anlegen; das Kontextmenü bietet Unterkategorien an.
Sessions und Markdown-Dateien aus dem vorhandenen Ressourceninventar stehen zunächst
in den Gruppen **Sessions ohne Einordnung** / **md-Dateien ohne Einordnung**.
Diese Gruppen sind anfangs zugeklappt.

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
Ansicht bleibt unverändert verfügbar. Keine Migration bestehender Bookmarks nötig.

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

- Native Tests: copi 118 bestanden / 2 ignoriert; cocopi 122 bestanden / 2 ignoriert.
  Enthalten sind Modell-/Dateikonflikttests sowie Zeilen-, Tastatur-, Dropziel- und
  Dialogregressionen. Keine echten privaten Sessiondateien werden verändert.
- `node scripts/test-outline.cjs` und JS-Syntaxchecks bestanden. Bestehende JS-Suite:
  nur `test-statusline-net.cjs` scheitert am bereits in HEAD entfernten historischen
  `NET_PLACEHOLDER`; kein neuer Gliederungsfehler. Dieser Alt-Test ist offen.
- pi-lite `cargo check --offline --locked` bestanden. Dafür alten API-Bruch in
  `src-tauri/src/net.rs` korrigiert: `Spec.args` wird als leere Liste initialisiert.
- Native Headless-Demo `COPI_SCENE=outline` angesehen; isolierte Chromium-Seitenleiste
  mit Testdaten angesehen. Beides **ohne pi**, keine echte WebKit-/Backend-Abnahme.
- Release/Installation: siehe aktuellen Nachtrag in `HANDOFF.md`.
- Offen: reale Maus-/Tastaturbedienung, langes Ziehen mit Scrollen, gleichzeitige
  Mehrfensterbedienung, WebKit-Livetest und echte Remote-Sessions. Automatisierte
  Tests ersetzen diese Prüfungen nicht. Keine v3- oder R2-Übernahme erfolgt.
