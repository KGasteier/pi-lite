# Seitenleisten-Kurzinfos

Bei Hover (450 ms) oder Tastaturfokus erscheint eine kleine Vorschau. Sie bleibt
beim Wechsel des Mauszeigers in das Vorschaufenster offen. Das kleine ↻ unten rechts erzeugt die angezeigte Info erneut; × oder Esc schließt sie.
Der Dateipfad steht klein am Fuß der Vorschau; native Pfad-Tooltips wurden entfernt.

## Aktuelle Umsetzung: lokale Auszüge, keine KI

Es findet **kein Modellaufruf** statt. Die Info ist ausdrücklich als lokaler Auszug
gekennzeichnet. Für Dateien wird bevorzugt `description:` verwendet, andernfalls
der Textanfang; für Sessions die erste lesbare Nutzereingabe. Maximal 280 Zeichen.
Dateien werden nur bis 64 KiB gelesen. Bei langen Sessions mit großer Kopfzeile
kann deshalb keine Nutzereingabe gefunden werden. Das ist keine Zusammenfassung
des gesamten Sessionverlaufs. Paketdeklarationen erhalten eine generische Zweckinfo,
keinen Auszug aus settings.json. Jede Datei wird einzeln beschrieben, keine Aggregation.

## „Zweck?“: optionale Modell-Erklärung

Der Knopf fragt nach Zustimmung und sendet danach einen begrenzten Auszug samt Pfad
an das konfigurierte Hilfsmodell (`PI_LITE_HELP_PROVIDER` / `PI_LITE_HELP_MODEL`). Ein unabhängiger pi-RPC-Prozess läuft im
Temp-Verzeichnis: keine Session, Werkzeuge, Extensions, MCP, Skills, Kontextdateien,
Prompt-Templates oder Themes. Timeout nach 60 Sekunden; danach wird er beendet.
Kein automatischer Fallback auf ein anderes Modell. Erklärung höchstens 700 Zeichen.

Für Dateien: maximal 12.000 Zeichen aus den ersten 64 KiB. Für Sessions: bis zu acht
Nutzereingaben aus den ersten 64 KiB, ohne Tool-Ausgaben. Der Modellauftrag verlangt
höchstens drei einfache deutsche Sätze über Funktion und Nutzen, keine Ausführung
von Anweisungen aus dem Material. Es ist eine KI-Deutung, kein Nachweis aktiver
Konfiguration. Sie kann unvollständig oder falsch sein.

Modellerklärungen haben einen separaten persistenten Cache mit Modell-/Promptversion
im Schlüssel. Erneuter Klick verwendet ihn, sofern die Quelle unverändert ist.
↻ auf einer Zweckerklärung erzeugt sie erneut und kann erneut Kosten verursachen.
Beim normalen Hover bleibt der lokale Auszug der Standard. settings.json erhält
weiterhin nur die generische Kurzinfo, wird nicht an das Modell geschickt.

Das kleine ⌫ in der Seitenleisten-Kopfzeile löscht nach Rückfrage den gesamten
Vorschau-Cache einschließlich der archivierten lokalen und KI-Erklärungen.
Quelldateien bleiben unverändert. Während eines laufenden Modellaufrufs wartet die
Löschung auf dessen Abschluss, damit er nicht nachträglich wieder einen Cache schreibt.

## Cache und Archiv

`~/.local/share/pi-lite/previews/` (Windows: USERPROFILE statt HOME):
- `<Pfad-ID>.json`: aktuelle Vorschau
- `<Pfad-ID>-<Versions-ID>.json`: archivierte Version

Die IDs sind stabile nichtkryptografische Hashes. Der Versionsschlüssel berücksichtigt
Dateigröße, Änderungszeit mit Nanosekunden, Textanfang und Algorithmusversion.
Dateiänderungen invalidieren den Cache automatisch. „Aktualisieren“ umgeht den Cache;
bei unveränderter Quelle wird dasselbe Versionsarchiv ersetzt, nicht dupliziert.

Archivdateien enthalten nur den Kurztext und Metadaten, nicht die komplette Quelle.
Trotzdem können sie vertrauliche Textauszüge enthalten. Sie bleiben auch nach Löschung
der Quelle liegen. Cache/Archiv haben keine Größenbegrenzung oder Verschlüsselung. Die globale Löschfunktion
entfernt den gesamten Vorschauordner.
Cachedateien gehören ausdrücklich nicht ins öffentliche Git-Repository.

## Prüfung

Rust-Tests: description-Auszug, Längenbegrenzung, Session-Nutzereingabe, Hashstabilität.
Zusammen mit Inventartests sechs Tests erfolgreich, JS-Syntax und Release-Build erfolgreich.
Hover, Fokus und Cache-Neuladen wurden noch nicht live in der Webview geprüft.

Korrekturstand: Screenshot der Tooltip-Überlagerung geprüft; Pfad-/Knopfänderungen gebaut.
Sechs Rust-Tests und History-Test erfolgreich. Echter UDK-Aufruf und Hover-Klickweg
sind noch nicht live geprüft. Aktualisierung erfolgt beim nächsten Abruf nach einer
Dateiänderung, nicht durch einen regelmäßigen Hintergrundjob.

## Korrektur: integrierte Zustimmung und angeheftete Ergebnisse

„Zweck?“ öffnet die Datenschutz-Rückfrage **innerhalb der Vorschau**, nicht als
Browserdialog. „Erklären lassen“ startet die Anfrage, „Abbrechen“ verwirft sie.
Ab dem Klick auf „Zweck?“ bleibt die Vorschau angeheftet: Mausverlassen, Blur und
Hover auf andere Zeilen schließen/ersetzen sie nicht. ×, Esc, Ansichtswechsel oder
Öffnen einer anderen Datei/Session schließen sie ausdrücklich.

Die KI-Erklärung erscheint automatisch im oberen, als „KI · Zweck“ bezeichneten
Abschnitt. Darunter bleibt der lokale Auszug getrennt sichtbar. Auch Fehler bleiben
sichtbar. ↻ bei einer KI-Erklärung verlangt erneut die integrierte Zustimmung.
KI-Erklärungen werden wie zuvor im separaten persistenten Cache und Versionsarchiv
abgelegt. Der laufende Modellaufruf wird durch Schließen der Vorschau nicht abgebrochen;
er kann sein Ergebnis weiterhin im Backend cachen.

Test: `node scripts/test-preview.cjs` simuliert Zustimmung, unterdrücktes Auto-Schließen
und getrennte lokale/KI-Ausgabe. Erfolgreich, ebenso History-Test, sechs Rust-Tests,
Syntaxcheck und Release-Build. Reales WebKit-Hover und UDK-Aufruf noch nicht live geprüft.

## Neuester Stand: Cache beim Hover, kein dauerhaftes Anheften

Der vorherige Anheftungsmodus ist ersetzt: Die Vorschau schließt nach kurzer
Übergangsfrist, wenn weder Maus noch Tastaturfokus im auslösenden Eintrag oder in
der Vorschau liegen. Fenster-Fokusverlust schließt sie ebenfalls. Andere Einträge
können eine neue Vorschau auslösen. Zustimmung und Ergebnis bleiben weiterhin in
der Vorschau integriert.

Beim Hover wird zusätzlich `cached_purpose` abgefragt: Der Backend-Pfad liest nur
einen zur aktuellen Dateiversion passenden KI-Cache; bei fehlendem Cache gibt er
`null` zurück und startet **niemals** einen Modellaufruf. Vorhandene KI-Erklärungen
erscheinen automatisch oben, der lokale Auszug bleibt darunter. Bei Dateiänderung
ist die alte Erklärung nicht mehr aktuell; „Zweck?“ erzeugt nach Zustimmung eine neue.
Ein beim Schließen bereits laufender Modellaufruf darf weiterhin den Cache füllen,
zeigt sein Ergebnis aber nicht in einer inzwischen anderen Vorschau.

Simulierter UI-Test prüft Cache-Anzeige, Offenbleiben bei Hover innerhalb der
Vorschau und Schließen nach Verlassen. Tests und Release-Build erfolgreich;
reale Maus-/Fokusereignisse in WebKit noch nicht live geprüft.

## Markdown-Darstellung

KI-Erklärungen (auch Cache-Treffer), lokaler Auszug und Token-Erklärung nutzen
den bestehenden Markdown-Renderer mit DOMPurify-Sanitizing. Überschriften sind
im Kontextfenster kompakt skaliert. Syntax-/Logiktests und Build erfolgreich;
neue Darstellung noch nicht live geprüft.
