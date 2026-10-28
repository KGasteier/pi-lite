# I2-Gate und Angriffsszenarien (S6)

## Aufruf (ohne Installation)

```sh
node --test test/i2/*.test.mjs
I2_HUB_DIR=/pfad/pi-hub-worktree node --test test/i2/*.test.mjs
I2_SEMINAR_DIR=/pfad/pi-lite-s8 node --test test/i2/consent.test.mjs
bash test/v3-gate.sh "$PWD" /pfad/hub-s3 /pfad/hub-s7
V3_GATE_HUBS="/pfad/hub-s3 /pfad/hub-s7" bash test/v3-gate.sh "$PWD"
python3 test/self-test.py
```

Standard-Hub für direkte Node-Aufrufe: `/home/medienhaus/Work/pi-hub-wt/s7`.
Fehlende Module ergeben **SKIP**, keine grüne Funktionsbehauptung. `i2-szenarien`
prüft die I2-Basis einschließlich Seminar-Automat. Die neuen N2/N3/N6- und
Recovery-Verträge stehen separat in `*.i2b.test.mjs` und im Gate als
`i2b-szenarien`, damit noch fehlender I2b-Code nicht den I2-Nachweis verdeckt.
Das Gate meldet Szenarien mit SKIP oder TODO ebenfalls als SKIP (Exit 0 heißt dort nur:
keine ausgeführte Prüfung fehlgeschlagen). Einzelne fehlgeschlagene Checks führen
weiter zu Exit 1. Das Gate führt jeden angegebenen Hub separat aus, nicht als
zusammengeführtes Produkt. Pfade in `V3_GATE_HUBS` dürfen keine Leerzeichen haben;
für solche Pfade positionale, gequotete Argumente verwenden.

## Nachweise und Grenzen

- `sync.test.mjs`: echte Sync-/Geräte-HTTP-Handler mit Ed25519-Challenge-Login.
  Projekt hochladen, Manifeststatus prüfen, Rückläufer und SHA-256 prüfen;
  Traversal, Absolut-/Windows-Pfad, Symlink, Hardlink, bestehender Ziel-Symlink
  und **3-GiB-Größenheader ohne große Nutzlast/Allokation** werden abgewiesen.
  Ein vorangestellter gültiger Tar-Eintrag darf bei Angriff nicht zurückbleiben.
- `empfang.test.mjs`: echter Empfang, echte Sync-HTTP-Handler, Fake-OIDC/JWKS
  und temporäre Slots; Check-in → Upload → Vorschau → Archiv → lokale Sicherung
  → Erhalten → Leeren/Prüfen → freier Poolplatz. Alter Token ist ungültig.
  Fake-pi aus dem Hub antwortet auf `get_state` und wird vor dem Leeren beendet.
  Dazu 20 parallele HTTP-Check-ins, FIFO-Promotion, fehlschlagendes Leeren und
  verschlüsseltes Abholfach (fremde Person, falscher Signaturschlüssel,
  falscher Entschlüsselungsschlüssel, erfolgreicher Besitzer-Abruf).
- `belegung.i2b.test.mjs`: echte v1-Hub-HTTP-Routen, flache Ed25519-Signaturen;
  N2 wiederholt byte-gleiche Belegung/Freigabe auch nach Hub-Neustart, prüft
  unveränderten Gerätezugang und weist veränderte Nutzlast ab. N3 leert nur ein
  Temp-Home und erhöht die externe Temp-Epochendatei; alte Belegung/Freigabe
  bleiben gesperrt, die aktuelle Epoche funktioniert.
- `recovery.i2b.test.mjs`: N6-Abbruch ohne Archiv, FIFO-Nachrücken, Widerruf und
  fehlender Leeren-Nachweis; Empfang-Neustart nach verlorener Hub-Antwort vor
  Aktivierungspersistenz, byte-gleicher Retry und kein doppelter Poolplatz.
  Fehlende I2b-Funktionen werden mit konkretem Grund übersprungen (keine
  Fehlerinterpretation als SKIP). Feature-Erkennung erfolgt vor dem Test am
  Quelltext, weil v1 noch kein Capabilityfeld für diese Zusätze hat.
- `pool.test.mjs`: Reservierung/Persistenz/Warteschlangenablauf am Pool-Kern.
  Kein Ersatz für den zusätzlich vorhandenen parallelen HTTP-Test.
- `oidc.test.mjs`: RS256/ES256 und negative Claims-/Signaturtests gegen lokalen
  JWKS-Issuer. Kein vollständiger Systembrowser-/PKCE-/authentik-Livetest.
- `abholfach.i2c.test.mjs`: erzwungener Check-out über HTTP, bestätigtes Slot-Fake-Leeren, erneuter Fake-OIDC-Check-in desselben `sub` mit zweitem Gerät; Abruf zulässig, Entschlüsselung mit neuem X25519-Schlüssel abgewiesen. Nach Empfang-Neustart bei 72 h − 1 ms noch vorhanden, bei exakt 72 h per Sweep physisch gelöscht. Node-Clientsimulation mit echter S7-Kryptofunktion; separat als `i2c-szenarien` ausgeführt.
- `abholfach.test.mjs`: Schlüsselbindung, Manipulation und 72-h-Ablauf.
- `fixtures.test.mjs`: validiert die selbst erzeugten Miniarchive/Auswertung.

**Testdoubles:** `empfang-harness.mjs` injiziert einen expliziten Service-Adapter
für Belegung/Freigabe/Archiv. Dieser prüft S7-Signaturen und registriert Geräte
im echten DeviceStore, ersetzt aber die produktiven `/v1/belegung`-/
`/v1/freigabe`-Routen. Auch `clearSlot` ist eine sichere Implementierung nur für
`mkdtemp`-Ordner, **nicht** `cocopi-slotctl`. Der Archivadapter benutzt die echte
Sync-Archivfunktion. Epochendateien liegen ebenfalls nur unter `mkdtemp` und
werden beim Fake-Leeren erhöht. `detectEmpty` aktiviert ausschließlich in N6-
Szenarien den Leer-Adapter; bestehende Archivtests bleiben bewusst unverändert.
Empfang-Retry-Backoff wird aufgezeichnet, aber ohne Wartezeit ausgeführt.
Der Fake-pi-Prozess ist ein modellfreier RPC-Prozess und
kein Beweis für die produktive Hub-Prozessverwaltung. Lokales Rückspielen ist
nur ein Testfixture, nicht die Rust-Client-Implementierung/deren Konfliktlogik.

**Mitsteuern:** `consent.test.mjs` prüft standardmäßig den öffentlichen S8-
Automaten aus dem eigenen Repository. `I2_SEMINAR_DIR` kann bewusst ein anderes
geprüftes Repository auswählen; das Gate setzt standardmäßig seinen Zielworktree.
Es erzeugt eine eigene temporäre Rust-Testcrate aus
`seminar-consent.rs` und baut offline mit drei Jobs im S6-Target (oder explizitem
`CARGO_TARGET_DIR`), ohne S8-Quellen zu ändern. Zwei unabhängige Angriffstests
prüfen erfundene Requests, ausstehende Zustimmung, Nur-Ansicht, Ablehnung,
Check-out, falsche Session und Ende. Ein explizit erlaubter Prompt muss genau
einmal mit Leitungsattribution im MemoryNet ankommen. Fehlt der Automat im
gewählten Repository, bleibt ein sichtbares SKIP mit Pfadgrund (kein TODO).
Das prüft den Automaten, **nicht** echte P2P-/UI-/Hub-Verdrahtung.

**Noch offen:** Kein UI-, Geräte-, Systemaccount-, Quoten-,
Systemd- oder echter R2-Löschtest. Weder Signatur-/Sync-Integration zwischen den
separaten Hub-Worktrees noch Datenschutzkonformität wird durch diese Tests
pauschal behauptet; siehe `coord/BUGS.md` und S6-Bericht im Koordinationsbaum.

Alle Dienste binden an zufällige **127.0.0.1**-Ports, verwenden temporäre Daten
und schließen eigene Server/Prozesse im Test-Cleanup. Kein npm, kein externer
OIDC-/Modellaufruf, keine privaten pi-Konfigurationen. Für direkte Aufrufe nur
den bekannten lokalen Fake-Issuer verwenden. Für isolierte Anwendungshomes und
offline Cargo mit eigenem Target das Gesamtgate verwenden.
