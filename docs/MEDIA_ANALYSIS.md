# Isolierte Audio-/Videoanalyse

## Implementiert

Material im Automatikmodus kann Audio (wav/mp3/flac/ogg/m4a) und Video
(mp4/webm/mov) an einen separaten OpenRouter-Chat-Completions-Aufruf übergeben.
Vor jedem Upload fragt die Oberfläche nach Zustimmung und weist auf Zusatzkosten
hin. Analyse läuft in einem separaten Python-Prozess, schreibt keine pi-Session,
hat keine Tools und verändert das Hauptmodell nicht. Textbefund samt Modell-ID und
Dateipfaden wird der normalen Anfrage an den Hauptagenten hinzugefügt. Dieser
Befund ist nicht unabhängig verifiziert. Dateien werden nicht über eine dauerhafte
öffentliche URL bereitgestellt, sondern base64-kodiert in der API-Anfrage gesendet.

## Einrichtung (Power-User)

1. Exakte API-ID und Audio-/Video-Unterstützung beim Anbieter prüfen. Tabellen-
   oder Marketingnamen sind nicht ausreichend. Die PDF-Modellnamen wurden NICHT
   ungeprüft als Routen übernommen.
2. `examples/media-routes.json` nach `~/.config/pi-lite/media-routes.json` kopieren.
3. Platzhalter ersetzen, Modalitäten korrekt setzen, erst dann `verified: true`.
4. Den in `keyEnv` benannten API-Schlüssel sicher in der App-Umgebung bereitstellen
   (z.B. lokales `~/.config/pi-lite/env.sh` mit restriktiven Dateirechten).
   Kein Schlüssel in routes.json oder Git; keine automatische Extraktion aus
   bestehenden pi-Auth-/Providerdateien.
5. pi-lite neu starten. Details → Medienanalyse zeigt bestätigte Routen; ohne
   Schlüssel sind sie deaktiviert. Automatik wählt nach priority (kleiner zuerst),
   dann Modell-ID, nur wenn alle Dateimodalitäten abgedeckt sind. Eine ausdrücklich
   gewählte Route wird nie automatisch durch eine andere ersetzt.

Vor dem Upload prüft der Worker im aktuellen OpenRouter-Katalog, ob die genaue
Modell-ID und benötigten `architecture.input_modalities` weiterhin existieren.
Diese Prüfung bestätigt deklarierte Fähigkeiten, nicht tatsächliche Qualität,
Berechtigung oder Kontingent. Falls die gewählte Route ausfällt, erfolgt KEIN
kostenpflichtiger automatischer Wiederholungsversuch bei einem anderen Modell.

## Grenzen / Kontrolle

Max. 4 Medien pro Analyse, zusammen max. 20 MiB (konfigurierbar nur nach unten),
max. 4096 Ausgabetokens. HTTP-Zeitlimit: Katalog 20s, Analyse 120s. Kein Streaming,
keine Fortschrittsprozentzahl, keine Video-Zerlegung/Transkription-Fallbacks.
Routen bestätigen Audio/Video allgemein; konkrete Codec-/Dauerlimits müssen
zusätzlich beim Anbieter geprüft werden. Kein harter Geldbetrag als Kostenlimit.
„Nur Dateiverweise“ umgeht den separaten Medienaufruf; kein Lokal-only-Schutz.

Der normale Abbrechen-Knopf stoppt den Hauptagenten, nicht den Medienworker.
Ein eigener Jobmanager mit unmittelbarem Abbruch, persistentem Befundcache und
Parallelisierung mehrerer Aufträge fehlt noch. Schlägt die nachfolgende Haupt-
Anfrage fehl, kann ein erneutes Senden erneut Medienanalyse-Kosten verursachen.
Material bleibt bei Fehlern erhalten. **Keine automatische Einrichtung unbekannter
Anbieter, keine bereits nachgewiesene reale Audio-/Videoanalyse auf dieser Maschine.**

## Quellen / Tests

OpenRouter-Dokumentation geprüft: Audio `input_audio` mit data/format; Video
`video_url` mit base64 data URL über `/api/v1/chat/completions`.
https://openrouter.ai/docs/guides/overview/multimodal/audio
https://openrouter.ai/docs/guides/overview/multimodal/videos

`python scripts/test-media.py`: unbestätigte Routen nicht angeboten, Schlüssel
nicht ausgegeben, inkompatible Medien und fehlende manuelle Route abgelehnt;
keine Netzaufrufe oder echten Medien. JS-Syntax/bestehende Tests, Rust-Tests und
Release-Build geprüft. Native WebKit-Bedienung und realer API-Medienaufruf offen.
