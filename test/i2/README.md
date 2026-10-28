# I2-Gate und Angriffsszenarien (S6)

## Aufruf (ohne Installation)

```sh
node --test test/i2/*.test.mjs
I2_HUB_DIR=/pfad/pi-hub-worktree node --test test/i2/*.test.mjs
bash test/v3-gate.sh "$PWD" /pfad/hub-s3 /pfad/hub-s7
V3_GATE_HUBS="/pfad/hub-s3 /pfad/hub-s7" bash test/v3-gate.sh "$PWD"
python3 test/self-test.py
```

Standard-Hub für direkte Node-Aufrufe: `/home/medienhaus/Work/pi-hub-wt/s7`.
Fehlende Module ergeben **SKIP**, keine grüne Funktionsbehauptung. Das Gate
meldet I2-Szenarien mit SKIP oder TODO ebenfalls als SKIP (Exit 0 heißt dort nur:
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
- `pool.test.mjs`: Reservierung/Persistenz/Warteschlangenablauf am Pool-Kern.
  Kein Ersatz für den zusätzlich vorhandenen parallelen HTTP-Test.
- `oidc.test.mjs`: RS256/ES256 und negative Claims-/Signaturtests gegen lokalen
  JWKS-Issuer. Kein vollständiger Systembrowser-/PKCE-/authentik-Livetest.
- `abholfach.test.mjs`: Schlüsselbindung, Manipulation und 72-h-Ablauf.
- `fixtures.test.mjs`: validiert die selbst erzeugten Miniarchive/Auswertung.

**Testdoubles:** `empfang-harness.mjs` injiziert einen expliziten Service-Adapter
für Belegung/Freigabe/Archiv. Dieser prüft S7-Signaturen und registriert Geräte
im echten DeviceStore, ersetzt aber die produktiven `/v1/belegung`-/
`/v1/freigabe`-Routen. Auch `clearSlot` ist eine sichere Implementierung nur für
`mkdtemp`-Ordner, **nicht** `cocopi-slotctl`. Der Archivadapter benutzt die echte
Sync-Archivfunktion. Der Fake-pi-Prozess ist ein modellfreier RPC-Prozess und
kein Beweis für die produktive Hub-Prozessverwaltung. Lokales Rückspielen ist
nur ein Testfixture, nicht die Rust-Client-Implementierung/deren Konfliktlogik.

**Noch offen:** Mitsteuern ohne Zustimmung hat ein sichtbares TODO, bis die
S8-Schnittstelle vereinbart ist. Kein UI-, Geräte-, Systemaccount-, Quoten-,
Systemd- oder echter R2-Löschtest. Weder Signatur-/Sync-Integration zwischen den
separaten Hub-Worktrees noch Datenschutzkonformität wird durch diese Tests
pauschal behauptet; siehe `coord/BUGS.md` und S6-Bericht im Koordinationsbaum.

Alle Dienste binden an zufällige **127.0.0.1**-Ports, verwenden temporäre Daten
und schließen eigene Server/Prozesse im Test-Cleanup. Kein npm, kein externer
OIDC-/Modellaufruf, keine privaten pi-Konfigurationen. Für direkte Aufrufe nur
den bekannten lokalen Fake-Issuer verwenden. Für isolierte Anwendungshomes und
offline Cargo mit eigenem Target das Gesamtgate verwenden.
