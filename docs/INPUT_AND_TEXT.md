# Eingabe-History und Ausgabeschrift

## Eingabe-History

Gesendete Eingaben (auch `steer`) werden im Speicher der laufenden Anwendung gehalten,
maximal 100. Direkt aufeinanderfolgende Duplikate werden zusammengefasst. Keine Speicherung
auf Platte: Eingaben können vertraulich sein. Die History gilt über Sessionwechsel hinweg.

- ↑/↓ bewegen nur den Cursor; sie rufen keine History mehr auf.
- Alt+↑: ältere Eingabe holen.
- Alt+↓: neuere Eingabe holen.
- Alternativ die vertikalen ↑/↓-Knöpfe neben dem Eingabefeld per Maus oder Tab/Enter.
- Nach der neuesten Eingabe wird der zuvor eingegebene Entwurf wiederhergestellt.
- Innerhalb mehrzeiliger Texte bleibt die normale Pfeilnavigation erhalten.
- Bei markiertem Text bzw. mit Zusatztasten bleibt die normale Textbedienung erhalten.
- Manuelles Bearbeiten beginnt einen neuen History-Durchlauf mit dem bearbeiteten Entwurf.
- Zurückgeholte Eingaben werden erst durch Enter/Senden erneut gesendet.

## Ausgabeschrift

A− und A+ in der Kopfzeile ändern die Schrift im Chatbereich zwischen 10 und 30 px.
Klick auf die Größenanzeige setzt auf 15 px zurück. Auch Code, Thinking und Tool-Ausgaben
skalieren mit; Eingabefeld, Seitenleiste und Datei-Editor bleiben unverändert.
Die Größe wird im lokalen Webview-Speicher für den nächsten Start behalten.

## Tests

`node scripts/test-history.cjs`: Reihenfolge, Grenzen, Duplikate, Entwurfsrückkehr und Limit.
JS-Syntaxcheck, drei Backend-Tests und Release-Build erfolgreich.
Die tatsächliche Pfeilbedienung in WebKit und die Größenknöpfe sind noch nicht live geprüft.

## Aktualisierte History und Chataktionen

↑/↓ im fokussierten Eingabefeld rufen wieder History auf, wenn der Cursor in der
 ersten/letzten Textzeile steht und keine Auswahl aktiv ist. Innerhalb mehrzeiliger
 Texte bewegen sie den Cursor. Alt+↑/↓ und die Pfeilknöpfe bleiben verfügbar.
Links neben der Eingabe stehen drei gleich große SVG-Werkzeugknöpfe: Material +,
ältere und neuere History. Rechts entsprechen ihnen Editor, Senden, Abbrechen in
gleicher Dreierteilung. Tab-Reihenfolge: Material → History ↑/↓ → Eingabe → rechts.

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
