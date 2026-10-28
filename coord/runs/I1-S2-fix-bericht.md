# I1-S2-fix · Desktop-Nachbesserung

Stand: 28.10.2026, **abgeschlossen, HEAD `ed78a2b`** (ca. 20:29 CET,
Beginn 20:04:53; innerhalb 90 Minuten).
Worktree `/home/medienhaus/Work/pi-lite-wt/s2`, Branch `v3/s2`, sauber.
Ausgang `9c8857c`, eigener Checkpoint `dbc6267`, autorisierter S3-Merge
`9234ee8` (übernimmt `0f2716a`), HTTP-/Dialognachweis `ed78a2b`.

## Erledigt

- `main.rs`: gespeicherten Modus vor App-/Backend-Konstruktion initialisieren,
  einschließlich `--sync-states`; Demo/Screenshots lesen keinen echten Modus.
  Reihenfolgetest mit injiziertem Initialisierer ohne Konfigurationszugriff.
- `net.rs`: fehlerhaftes `backends.json` fällt durch `backends::parse("[]")`
  zurück; kein ungefilterter lokaler Eintrag. Regressionstest mit beschädigtem
  JSON im Temp-Verzeichnis und Vergleich mit zentralem Parser.
- `desktop.rs`: unabhängige Dialogaktionen in lesbaren Blöcken; ungenutzte
  Varianten/Felder entfernt. Pairing-Abbrechen per simuliertem Klick sowohl bei
  gültigem als auch bei abgelaufenem Code getestet. Einstellungen werden vor dem
  Pairing-QR geschlossen.
- Sprecher als eigene Daten und dezente Zeile; 14 px Innenabstand und Abstand
  zum Markdowntext. Sessiontitel/Zustand/Ellipsis auf einer Zeile, neutrale
  durchgehende Auswahl. Statt `udk2/…`: „Modell: Standard“.
- Ausschließlich dateibezogenes rustfmt für `desktop.rs` und `desktop/hub.rs`.
- Acht Szenen in `coord/shots/I1/` erneuert (1400×880), tatsächlich angesehen:
  `v3-start`, `v3-start-fehler`, `v3-main-remote`, `v3-modus`,
  `v3-hybrid-dialog`, `v3-geraete`, `v3-koppeln-qr`, `v3-grenze`.
  Zusätzlich Kontaktbogen `v3-fix-kontaktbogen.png`.

## Finale Tests / Nachweise nach S3-Merge

Mit `--offline --locked`, `CARGO_BUILD_JOBS=3` und
`CARGO_TARGET_DIR=/home/medienhaus/Work/pi-lite-wt/target-s2`:

| Prüfung | Ergebnis | Log |
|---|---|---|
| cocopi-Binarytests vollständig, einschließlich echter HTTP-/Node-Adaptertests | **125 ok, 2 ignoriert, 0 gefiltert** | `/tmp/I1-S2-fix-cocopi-final.log` |
| copi-Binarytests | **102 ok, 2 ignoriert** | `/tmp/I1-S2-fix-copi-final.log` |
| Debug-Build cocopi und acht Screenshots nach Merge | ok | `/tmp/I1-S2-fix-build-final.log` |
| Clippy cocopi einschließlich Tests | Exit 0, vorhandene Warnungen | `/tmp/I1-S2-fix-clippy-final.jsonl` |
| Clippy-Baseline `eb534bb`, aus `git archive` in Temp-Verzeichnis | **keine neuen S2-Warnungsarten** | `/tmp/I1-S2-fix-baseline-clippy-tests.jsonl` |
| dateibezogenes `rustfmt --edition 2021 --check` und `git diff --check` | ok | abschließender Toollauf |

Clippy-Differenz über Datei/Code/Meldung verglichen: `desktop.rs`,
`desktop/hub.rs`, `main.rs` ohne Warnungen; die Warnung `if_same_then_else` in
`net.rs` ist bereits in `eb534bb` vorhanden. Beim Test-Clippy ist auch
`items_after_test_module` in `main.rs` schon in der Baseline vorhanden (neue
Funktionen ergänzen nur ihre sekundären Spans). Vergleich dedupliziert nach
Datei/Code/Meldung: keine neue Warnungsart. Alte Warnungen wurden nicht als durch
diesen Auftrag behoben ausgegeben.

## S3-Integration / echte HTTP-Regression

Nach überprüftem Heartbeat **„FIX fertig 0f2716a e2eabfa“** wurde ausschließlich
der erlaubte Merge `git merge --no-edit v3/s3` ausgeführt. Die API blieb
`Result<…, String>` statt eines neuen öffentlichen Fehlertyps; `[code]`-Präfixe
sind nun der maschinenlesbare Vertrag. Der bestehende Adapter versteht sie.

- Der neue echte Loopback-HTTP-A3-Test reproduzierte **vor** dem Merge den Fehler
  (Server-Werbetext statt lokalem Text). **Nach** dem Merge prüft er erfolgreich
  `abgelaufen`, `verbraucht`, `unbekannt`, `zu_viele` und exakte A3-Texte, obwohl
  die HTTP-Antwort absichtlich unpassende `was`/`tun`-Texte enthält.
- Der Node-Adaptertest läuft gegen Hub `e2eabfa` mit temporärem privatem State,
  Mock-Harness und Temp-Schlüssel. Er öffnet zwei Sessions; der dritte echte
  HTTP-Versuch liefert `Reply::Limit`. Der Test wendet die Antwort auf den Desktop
  an und prüft die tatsächlich gerenderte Überschrift „Zwei Sessions sind offen“.
  Außerdem Einlösen/Login mit Kontextsignatur, Geräte, Ruhen, Paircode, Widerruf.
- `COCOPI_TEST_HUB_DIR` für den isolierten Adaptertest ergänzt. Keine eigene
  Produktions-HTTP-/Kryptologik implementiert; nur die Tests besitzen HTTP-Fixtures.

Die anfangs ausgesparte Integration und die zwischenzeitlichen roten neuen
Regressionstests sind im finalen vollständigen grünen Lauf enthalten, nicht
weggefiltert. Screenshots nach dem finalen Merge/Build erneut erzeugt; Hauptchat
nochmals angesehen, übrige Szenen vor Merge vollständig visuell geprüft.

## Unveränderte Grenzen

Keine Release-/Installations-, Netzwerk-/System- oder Kontenänderung. Kein echtes
Modell, keine reale Konfiguration und kein R2-Zugriff. Nur isolierte Loopback-Mocks
und temporäre Geräteschlüssel. Headless-Szenen sind keine macOS-/Windows- oder
Handy-/Kameraabnahme. Lokaler Hybrid, Modellpicker, Dateitransfer, Vorlesen und
vollständige Rückfragen bleiben die schon dokumentierten I1-Lücken.

**Neue wichtige S3-Grenze:** der SSE-Reader hat ein 45-s-Gesamtbodybudget, keinen
gleitenden Idle-Timer. Der Desktop meldet das Ende als Unterbrechung und sendet
niemals automatisch erneut. Automatischer Reconnect mit Last-Event-ID und
Deduplizierung ist noch nicht implementiert und vor Pilotbetrieb separat nötig.
Schlüsselspeichernamen noch nicht je Ursprung/Slot getrennt; Kontextsignatur
verhindert nun die originübergreifende Signaturweiterleitung. Der tatsächliche
45-s-Ablauf, WAN, Mac/Windows und Hardware bleiben ungeprüft. Diese Grenzen sind
auch in `copi/app/DESKTOP_V3.md` dokumentiert. Keine vollständige Pilotfreigabe.
