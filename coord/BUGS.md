# Bekannte Befunde · cocopi v3

Stand: 28.10.2026. Basisfehler bleiben sichtbar, sperren aber nicht als neue Regression das Merge-Gate. Sicherheitsbefunde sind nicht durch ein grünes Gate erledigt.

| ID | Schwere | Fundort | Beschreibung | Besitzer | Status |
|---|---|---|---|---|---|
| B1 | Mittel (Testbestand) | `scripts/test-statusline-net.cjs`, Basis `eb534bb` | Der Strukturtest erwartet `NET_PLACEHOLDER` in `copi/app/src/view.rs`; der Ausdruck fehlt bereits auf der Basis. Gate führt den Test weiter aus, meldet den Fehlschlag als `KNOWN`. | K zur Zuordnung an Test-/UI-Besitzer | Offen, in `test/js-baseline.txt` erfasst; nach Korrektur entfernen. |
| B2 | Niedrig (Formatbestand) | Rust-Dateien, Basis `eb534bb` | Workspaceweiter Formatcheck war schon auf der Basis rot. Einzeldateimessung: 57 von 61 Rust-Dateien entsprechen nicht rustfmt; Details und Abgrenzung in `test/BASELINE.md`. Gate prüft nur geänderte/neue Dateien. | K zur Aufteilung an Rust-Dateibesitzer | Offen, keine workspaceweite Neuformatierung durch S6. |
| B3 | Mittel (Pilot-Sicherheit/Betrieb) | S3-Review „Vor Pilot beheben“, Punkte 1–4 | Proxy/TLS/Absender-Konfiguration, SSE-/Tokenlimits, Lese-Timeout und Umgebungsproxy. Nur Sammelverweis auf M2, N1–N3 im Review, kein neuer Befund. | S3 (mit S4 für Betrieb) | Offen; separaten Nachweis vor Pilot einholen. |
| B4 | Niedrig (Pilot-Härtung) | S3-Review „Vor Pilot beheben“, Punkte 5–7 | Lokale Fehlertexte, Debug-Begrenzung von `COPI_MODE_FILE`, Admin-Widerruf und Register-/Prozessbereinigung. Nur Sammelverweis auf N5–N7. | S3 | Offen; separaten Nachweis vor Pilot einholen. |
| B5 | Mittel (Plattform-/Negativtests) | S3-Review „Vor Pilot beheben“, Punkte 8–9 | Geräteschlüssel in macOS-Keychain bzw. Windows-ACL samt Plattformtest; Rust-Client gegen bösartige Mock-Antworten prüfen. Nur Sammelverweis. | S3 (S6 unterstützt Tests) | Offen, hier weder implementiert noch abgenommen. |

Quelle für B3–B5: `coord/runs/I1-S3-review-bericht.md` im Koordinations-Worktree. Der Review enthält weitere Merge-/Vor-I2-Auflagen; diese Tabelle ersetzt sie nicht. Fixberichte anderer Rollen wurden hier nicht sicherheitsgeprüft, daher keine Erledigt-Markierung.
