# Slash-Befehle

Im Eingabefeld `/` tippen: über dem Feld erscheint eine Liste der tatsächlich
vom laufenden pi per `get_commands` angebotenen Extensions, Prompt-Vorlagen und
Skills. Namen und Beschreibungen werden gefiltert; Namenspräfixe erscheinen zuerst.

- Tippen filtert, ↑/↓ navigieren mit Umlauf, Hover markiert.
- Tab, Enter oder Klick übernimmt `/name ` in die Eingabe, ohne Ausführung.
- Ein weiteres Enter oder Senden führt aus; davor können Argumente ergänzt werden.
- Esc schließt, ohne den Text zu löschen. Beim nächsten Tippen öffnet die Liste
  wieder. Ohne offene Liste bleiben die Eingabe-History-Pfeile unverändert.
- Nur ein einzelnes `/wort` am Eingabeanfang und Cursor am Ende öffnet die Liste;
  normale Nachrichten, Argumente und mehrzeilige Texte nicht.

Die Liste wird nach Sessionstart und abgeschlossenen Slash-Aufrufen aktualisiert.
Ein Wizard darf länger als 15 Sekunden dauern. Extension-Kommandos werden auch
während eines laufenden Agenten über `prompt` gesendet (nicht `steer`). Normale
Nachrichten erhalten währenddessen `streamingBehavior: steer`.

## Grenzen

TUI-interne Befehle wie `/settings` und `/hotkeys` sind nicht Teil von
`get_commands` und funktionieren nicht als RPC-Prompt. Sie werden hier nicht als
funktionierende Befehle angeboten. Modell/Thinking/Session-Funktionen bleiben über
die vorhandenen pi-lite-Bedienelemente zugänglich.

Extensions mit `ctx.ui.custom()` brauchen eine eigene RPC-Alternative. Für die
hier installierte pi-custom-endpoint-Version wurde diese lokal ergänzt:

```bash
python scripts/adapt-endpoint-rpc.py
```

Opt-in, idempotent, mit datiertem Backup der Extension-Datei; unbekannte Versionen
werden nicht verändert. Standard-`select` statt TUI-Auswahl, Mehrfachauswahl durch
wiederholtes An-/Abwählen und „Confirm selection“. Kein Lesen/Ändern von Credentials
oder models.json durch das Adaptionsskript. Die bestehenden Wizard-Aktionen können
wie in der TUI Konfiguration ändern und erfordern bewusste Auswahl/Bestätigung.
Nach pi-/Extension-Neustart verfügbar. npm-Updates können die Anpassung überschreiben;
dann erneut prüfen/anwenden. Keine automatische Installer-Anpassung.

## Wizard-Auswahl

Wizard-Dialoge haben einen begrenzten Scrollbereich mit Scrollbar und Positions-
zähler. Hover und Tastaturfokus markieren denselben Eintrag; ↑↓, Home/End und
PageUp/PageDown navigieren und scrollen den aktiven Eintrag ins Bild. Enter/Klick
bestätigt, Tab erreicht auch Abbrechen, Esc bricht ab. Regressionstest:
`node scripts/test-dialog-selection.cjs` mit 25 synthetischen Einträgen.
„New models“ enthält laut Extension nur noch nicht konfigurierte Modelle; die
Zahl kann kleiner als die Gesamtzahl des Providers sein. Der Zähler zählt alle
vom RPC-Dialog gelieferten Optionen, bei Mehrfachwahl inklusive Confirm-Aktion.
CSS-/DOM-Stubprüfung bestanden; echte WebKit-Bedienung bleibt manuell zu prüfen.

## Tests

`node scripts/test-slash.cjs`: Filter, Präfixpriorität, Pfeilumlauf, Hover,
Übernahme ohne Ausführen, Escape und History-Abgrenzung (DOM-Stubs).
Separater echter pi-RPC-Test ohne Session: endpoint-custom entdeckt, erster
Wizard-select empfangen, abgebrochen, erfolgreiche Antwort. Keine Konfiguration
verändert. Syntax, bestehende JS-Checks, Rust-Tests und Release-Build geprüft.
Echte WebKit-Maus-/Tastaturinteraktion und vollständiger Endpoint-Wizard noch
nicht live geprüft. Die laufende Nutzersession wurde nicht gesteuert/beendet.
