# Datenschutz beim cocopi-Pool

**Information in einfacher Sprache · Entwurf vor dem Pilotbetrieb.**
Verantwortlich: `[Einrichtung, Anschrift]` · Betrieb: `[Name/Funktion, E-Mail]` ·
Datenschutzkontakt: `[E-Mail/Telefon]`. Diese Angaben müssen vor Nutzung ergänzt
und die tatsächlichen Abläufe gemeinsam geprüft werden.

## Kein persönlicher Speicher auf dem Server

cocopi hat zehn gemeinsame Arbeitsplätze. Ein Platz gehört Ihnen nur während
Ihres Check-ins. Es gibt kein dauerhaftes persönliches Serverkonto und keinen
Anspruch auf Aufbewahrung. Bewahren Sie Ihre Arbeit auf Ihrem eigenen Gerät auf.
Vor dem Check-in wählen Sie ausdrücklich aus, welche Dateien hochgeladen werden.
Versteckte `.sync`-Daten dienen dem Abgleich, nicht als zusätzliche Sicherung.

Der Server verarbeitet während der Nutzung ausgewählte Dateien, Ergebnisse,
Agentensitzungen und die für Anmeldung/Geräteprüfung nötigen Angaben. Ein
Arbeitsplatz ist durch ein eigenes Unixkonto von den anderen getrennt. Die
Serveradministration hat technisch weitreichenden Zugriff; das ist **keine
Ende-zu-Ende-Verschlüsselung Ihrer laufenden Serverarbeit**.

## Wann wird gelöscht?

Beim normalen Auschecken erhalten Sie ein persönliches Ergebnisarchiv zurück.
Erst nach bestätigtem Empfang wird die Verbindung freigegeben. Der Server beendet
Prozesse, leert Home- und Datenverzeichnis und spielt nur das leere Grundgerüst
wieder ein. Vor Wiedervergabe müssen Leerzustand und Quoten geprüft sein. Dazu
gehören auch Agentensitzungen, temporäre Dateien und Hub-Geräteregister. Ein
Fehler sperrt den Platz; er wird nicht als „leer“ an die nächste Person gegeben.
Es gibt **keine dauerhafte Sicherung der persönlichen Slotdaten**.

Bei erzwungenem Auschecken, beispielsweise am festgelegten Tagesende oder nach
acht Stunden ohne Aktivität, darf nur ein verschlüsseltes Abholarchiv entstehen.
Es ist an die beim Check-in gelieferte Verschlüsselungsidentität gebunden. Es
wird nach bestätigter Abholung, spätestens nach **72 Stunden**, gelöscht. Danach
ist keine Wiederherstellung vorgesehen. Ohne passenden Schlüssel ist das Archiv
nicht lesbar. Für Gäste ist ein sicherer Wiederabholweg vor dem Pilot gesondert
zu klären; nicht ungeprüft darauf vertrauen.

Serverausfall oder Löschfehler können den Ablauf verzögern. Bis zur Behebung
bleibt der Arbeitsplatz gesperrt; die Administration klärt den Vorfall, statt
eine erfolgreiche Löschung vorzutäuschen. Gelöschte Dateien werden logisch aus
dem Dateisystem entfernt; dies ist keine Garantie physischer Überschreibung
jedes SSD-Blocks. Verschlüsselung der Datenträger und ausgeschlossene
Snapshots/Backups sind getrennte Betriebsvoraussetzungen.

## Anmeldung und Protokolle

authentik bestätigt die Anmeldung und die Rolle „Teilnahme“ oder „Seminarleitung“.
Der Empfang nutzt dafür einen pseudonymen Hash und stellt einen höchstens zwölf
Stunden gültigen Geräteausweis aus. Namen werden nur für die notwendige Anzeige
und laufende Zuordnung verarbeitet; nicht als Protokollinhalt übernommen.

Betriebsprotokolle dürfen nur Ereignisart, Zeitpunkt und pseudonymen Hash
enthalten. **Keine Namen, Dateinamen, Inhalte, Prompts, Tokens oder kompletten
URLs/Queries.** Sie werden spätestens nach sieben Tagen entfernt. Der öffentliche
Proxy schreibt in dieser Vorlage keine Access-Logs. Die System-Units unterdrücken
ungefilterte Programmausgaben. authentik, Tailscale, KI-Anbieter und vorhandene
Systemprotokolle haben eigene Verantwortlichkeiten und Fristen, die vor Nutzung
geprüft und hier ergänzt werden müssen: `[Betreiber/Frist/Link]`.

Ein aktiver Dienst zeigt keine Anwesenheit und keine Arbeitsleistung. Die
Betriebsübersicht ist keine Anwesenheitskontrolle. KI-Aufrufe sind ein eigener
Datenfluss zum bewusst gewählten Anbieter; Anmeldung bei authentik ist keine
Einwilligung zur Übermittlung privater Inhalte an einen KI-Anbieter.

## Fragen und Rechte

Bei Problemen nicht mit echten Inhalten/Schlüsseln in öffentliche Fehlerberichte
gehen. Wenden Sie sich an `[Betriebskontakt]`. Auskunft, Berichtigung, Löschung
und weitere Datenschutzrechte klären Sie mit `[Datenschutzkontakt]`.
Rechtsgrundlage, zuständige Aufsicht, freiwillige Alternative und mögliche
Anbieterübermittlungen ergänzt die verantwortliche Einrichtung vor dem Pilot:
`[Angaben/Link]`. Solange diese Angaben oder die technische Löschabnahme fehlen,
ist dies keine Freigabe für vertrauliche Arbeiten.
