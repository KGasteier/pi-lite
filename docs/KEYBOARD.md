# Tastaturnavigation

Grundprinzip: Tab/Shift+Tab wechseln Bedienelemente, Enter/Leertaste aktivieren native
Knöpfe. Pfeile navigieren in Listen und Knopfgruppen. Textfelder und Selects behalten
native Pfeilbedienung. Sichtbare Fokusmarkierung. F1 öffnet/schließt die Hilfe.

## Direkte Sprünge (außerhalb modaler Dialoge)

- Alt+1 Eingabe, Alt+2 aktuelle Seitenliste, Alt+3 Chat, Alt+4 Kopfzeile, Alt+5 Vorschau
- Alt+N neue Session, Alt+A abbrechen, Alt+F Fork
- Alt+M Modell, Alt+T Thinking (Modell, Workspace und Denken sind gleich bedienbare Dropdowns: ↑/↓ öffnet, Enter wählt, Esc schließt)
- Alt+− / Alt++ Ausgabeschrift (Alt+= ebenfalls vergrößern)
- Super+A markiert im Eingabefeld den gesamten Text (Strg+A funktioniert weiterhin)
- Strg/Cmd+B wechselt zur Sessionansicht und fokussiert die Liste

## Bereiche

- Sessions: ↑/↓, Home/End; Enter/Space öffnen; R/F2 umbenennen;
  Backspace löschen; M/Shift+F10 Menü. Menü: ↑/↓, Enter, Esc.
- Ressourcenlisten: ↑/↓, Home/End; Enter öffnet Markdown.
- Seitenliste: ← zur Ansichtsauswahl, → in die bereits sichtbare Vorschau.
  Die Vorschau erscheint nach 450 ms Fokus; Alt+5 ist ein alternativer Zugang.
- Modelle: ↑/↓ markieren ohne Modellwechsel; Home/End erreichen Anfang/Ende,
  PageUp/PageDown springen seitenweise. Enter/Klick bestätigt, Esc verwirft.
  Thinking behält die native Select-Bedienung.
- Kopfzeilen- und Vorschauknöpfe: Pfeile/Home/End wechseln Knöpfe; Tab ebenfalls.
- Chat selbst: ↑/↓, PageUp/PageDown, Home/End scrollen. Tab erreicht Links,
  Tool-Zusammenfassungen und andere native interaktive Elemente.
- Eingabe: ↑/↓ ausschließlich Cursorbewegung. Alt+↑/↓ lädt History;
  daneben zwei vertikal angeordnete, per Maus/Tab/Enter bedienbare Pfeilknöpfe.
  Enter sendet, Option/Alt+Enter fügt eine Zeile ein (auch anstelle einer Auswahl).
  Shift+Enter sendet ebenfalls; Option/Alt+Enter war zuvor nicht separat belegt.
- Editor: native Textnavigation, Strg/Cmd+S speichern, Esc schließen.
- Dialoge einschließlich Hilfe/Editor: Tab bleibt im Dialog. Pfeile navigieren
  Knöpfe, überschreiben aber keine Textnavigation. Esc schließt den obersten Dialog.
- Esc außerhalb von Dialogen schließt zuerst Kontextmenü/Vorschau, sonst zurück
  zur Eingabe. Neue Session/Datei öffnen schließt weiterhin die Vorschau.

Alt-Kürzel können auf macOS bzw. durch den Window Manager abgefangen werden.
Alle Funktionen bleiben über Tab und sichtbare Knöpfe erreichbar. Native Browser-
Bestätigungsdialoge (Cache löschen/Editor verwerfen) behalten ihre Systembedienung.
Kein vollständiges TUI-Fokusmodell: Tab bleibt der universelle Bereichswechsel.

## Prüfung

JavaScript-Syntax, History-/Vorschau-Logiktests, Rust-Tests und Release-Build geprüft.
Die neue reale Fokusreihenfolge, Tastenkombinationen, Selects und Dialognavigation
sind noch nicht live unter WebKit oder auf macOS/Windows geprüft.

## Aktualisierte History und Chataktionen

↑/↓ im fokussierten Eingabefeld rufen wieder History auf, wenn der Cursor in der
 ersten/letzten Textzeile steht und keine Auswahl aktiv ist. Innerhalb mehrzeiliger
 Texte bewegen sie den Cursor. Alt+↑/↓ und die Pfeilknöpfe bleiben verfügbar.
Tab folgt der DOM-Reihenfolge: Eingabe → oberer Historypfeil → unterer Historypfeil
→ Senden; die Pfeilknöpfe stehen visuell weiterhin links.

An der letzten eigenen Chatnachricht steht „Anfrage wiederholen“. Er sendet exakt
 deren Text erneut (keine Bilder/Anhänge), ohne den aktuellen Eingabeentwurf zu
verändern. Während pi arbeitet, wird Wiederholen mit Hinweis abgelehnt.
Am Ende des letzten abgeschlossenen Assistenten-Textblocks steht ⧉: Kopieren des
unveränderten Markdown-Quelltexts, ohne Thinking/Tools/HTML. Auch geladene Verläufe
haben diese Knöpfe. Tab und Enter/Space bedienen beide.

Syntax-/bestehende Tests und Release-Build erfolgreich; Zwischenablage, erneutes
Senden und echte WebKit-Tabreihenfolge noch nicht live geprüft.

## Korrektur: Anfrage übernehmen statt direkt wiederholen

Jede eigene Chatnachricht (auch im geladenen Verlauf) erhält unten rechts ein
kleines ↻. Tooltip/ARIA: „Anfrage in die Eingabe übernehmen“. Klick oder Enter
übernimmt nur den Text ins Eingabefeld und fokussiert es; keine automatische
Ausführung, kein History-Eintrag vor dem Senden. Ein vorhandener Eingabeentwurf
wird dabei ersetzt. Bestätigung erfolgt durch Enter/Senden. Anhänge werden nicht
übernommen. Syntax und bestehende Logiktests sowie Release-Build erfolgreich;
reale WebKit-Bedienung noch nicht live geprüft.

## Copy je Ausgabe und kompakte Anfrageaktion

Jeder abgeschlossene Assistenten-Textblock erhält ein eigenes ⧉, auch im geladenen
Verlauf. Es kopiert ausschließlich den Markdown-Quelltext dieses Blocks. Thinking
und Tools bleiben ausgeschlossen. ↻ liegt absolut im unteren rechten Rand jeder
Anfrage; keine zusätzliche Zeile, rechts reservierter Abstand zum Text. Funktion
bleibt: nur in die Eingabe übernehmen, nicht senden. Syntax-/bestehende Logiktests
und Release-Build erfolgreich; Darstellung und Clipboard noch nicht live geprüft.
