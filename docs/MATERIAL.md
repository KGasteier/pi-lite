# Material für die nächste Anfrage (erster Ausbau)

`+` neben der Eingabe öffnet Dateien-Mehrfachauswahl (Desktopportal) oder einen
Dialog für HTTP(S)-URLs, eine pro Zeile. Maximal 12 Materialien; Duplikate werden
vermieden. Chips zeigen die Auswahl; Klick entfernt einen Eintrag, Tooltip den
vollständigen Pfad/URL. Die Auswahl allein überträgt nichts und ruft keine URL ab.

Eine normale Frage und **Senden** bereiten Material lokal vor und übergeben es
mit der Anfrage an pi. Slash-Befehle mit Material und Material während eines
laufenden Agenten werden abgelehnt. Bei erfolgreicher Annahme werden Chips
geleert; Fehler behalten Material und stellen den Prompt wieder her, sofern
zwischenzeitlich kein neuer Text eingegeben wurde. Material bleibt beim
Sessionwechsel als expliziter Entwurf erhalten, nicht nach App-Neustart.

## Automatisch / manuell

Unter **Material · Details / manuelle Auswahl**:

- **Automatisch:** PNG/JPEG/WebP als tatsächliche RPC-Bildinhalte; PDF als lokaler
  Textauszug; andere Dateien als absolute Dateiverweise. URLs als Verweise.
- **Nur Dateiverweise:** keine Bildübertragung/PDF-Vorverarbeitung. Der Agent kann
  Dateien anschließend trotzdem lesen und Inhalte zum Provider schicken. Dies
  ist ausdrücklich **kein Lokal-only-Modus**.
- **Modell:** aktuelles Modell behalten oder eines der tatsächlich verfügbaren
  Modelle selbst wählen. Bildfähigkeit stammt aus pi-Metadaten `input`, nicht
  aus erratenen Modellnamen.

Ist das aktuelle Modell nicht bildfähig, wird ein bildfähiges Modell desselben
Anbieters angeboten und vor dem Wechsel gefragt. Kein automatischer Anbieter-
wechsel. Eine manuelle Auswahl wird nie automatisch ersetzt. Der Modellwechsel
bleibt aktiv und sichtbar in der Kopfzeile; kein temporärer Spezialagent.

## Grenzen und Datenschutz

Bilder maximal 8 MiB einzeln / 20 MiB zusammen; Dateiformat-Signatur geprüft.
PDF maximal 100 MiB; `pdftotext` extrahiert höchstens die ersten 30 Seiten und
60000 Bytes. Die Begrenzung steht im Prompt. Scans benötigen später OCR;
kein PDF-Vollständigkeitsversprechen. Auswahlpfade und Auszüge erscheinen im
Sessionverlauf. Für URLs kein Abruf/SSRF-Risiko durch diesen Vorbereitungsschritt;
Agentenwerkzeuge können sie später auf Nutzerauftrag abrufen.

Materialinhalt ist als Untersuchungsdaten gekennzeichnet, aber diese Kennzeichnung
ist keine technische Prompt-Injection-Sandbox. Kein separater Kostenbudget-Manager.
Senden kann Material an den aktuellen/gewählten Modellprovider übertragen.

Audio/Video werden im automatischen Modus mit verständlichem Hinweis abgelehnt;
als Dateiverweis möglich. ffmpeg/ffprobe sind hier vorhanden, aber kein verifizierter
Transkriptions-/Videodienst eingerichtet. **Audio-/Video-Automatik, parallele
Spezialjobs, Job-Abbruch/Timeouts, Kostenlimits und echte Lokal-only-Verarbeitung
sind noch nicht umgesetzt.** Diese Funktionen dürfen nicht als vorhanden gelten.

## Teststand

Rust: fehlende Dateien, Anzahlgrenze, Bildsignatur/Base64-Übergabe, Referenzmodus,
Audio-Ablehnung im automatischen Modus. JS-Syntax und bestehende Regressionstests,
Rust-Tests und Release-Build. Native Dateiauswahl, Bilder beim Provider, echte
PDF-Extraktion im Chat und WebKit-Klicks noch nicht live geprüft. Keine privaten
Dateien oder Credentials für Tests verwendet.
