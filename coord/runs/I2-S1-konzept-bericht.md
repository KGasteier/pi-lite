# Bericht I2-S1-konzept (S1, Branch `v3/s1`, Basis `e8f000e`)

## Commits
- `838faf0` KONZEPT Fassung 2 mit Teil B (Pool, Check-in/out, authentik, Sync, No-Retention)
- `f215234` FLOWS F5–F9; die alten F5/F6 heißen jetzt F10/F11
- `97378fe` ACCEPTANCE T13–T16 messbar, mit Nachweis und prüfender Rolle
- `df25dac` GLOSSAR neu; Begriffe in KONZEPT und ACCEPTANCE angeglichen

Kein push, kein merge. Gestaged wurden nur die 4 Dateien unter `docs/v3/`. Der Worktree ist sauber.

## Dateien und Ergebnisse
- **KONZEPT.md** (+169)
  - Kopf auf Fassung 2 gehoben. Teil A = I1.
  - A4 ist als **„ersetzt durch E7“** markiert, mit Liste dessen, was weiter gilt. Nichts wurde gelöscht.
  - Neuer **Teil B**:
    - B1 Laiensprache
    - B2 Pool/FIFO
    - B3 authentik/Gast/Ausweis/Empfang
    - B4 Sync-Auswahl
    - B5 Check-out/Leeren/Abholfach
    - B6 Datenklassen-Tabelle mit Beispielen aus Kunst, Design und Code (nach `shared/sync/klassen.json`)
    - B7 Seminar/Mitsteuern
    - B8 Lebenslauf eines Arbeitsplatzes als **Mermaid und ASCII**
    - B9 Technikabsatz
    - B10 Speicherdauer-Tabelle
    - B11 Fehlertexte (was/tun)
- **FLOWS.md** (+161)
  - I2-ASCII-Ansichten: Check-in, Warteschlange/Auswahl, Check-out-Vorschau, Mitsteuern-Rahmen.
  - Neue Flüsse **F5 Check-in, F6 Sync-Auswahl, F7 Check-out (inkl. Abholfach), F8 Seminar-Chat/DM, F9 Mitsteuern**. Spalten je Schritt: was man sieht, Hauptaktion, Abbruch, Fehler (was/tun).
  - **Umbenennung:** Die alten „F5 Hybrid“ und „F6 Fehler“ heißen jetzt **F10/F11** („bis I1: F5/F6“ steht in der Überschrift). Den Verweis in ACCEPTANCE T1.11 habe ich angepasst.
  - Code und Tests verweisen nirgends auf F5/F6. Geprüft mit grep über alle Worktrees.
- **ACCEPTANCE.md** (+78)
  - Neue Gates G3 (v3-gate ohne SKIP) und G4 (Wortlaut per grep).
  - T13.1–12, T14.1–7, T15.1–10, T16.1–8. Je Zeile Testname, Befehl oder Screenshot und die Rolle, die prüft.
  - **[da]** markiert Tests, die es mit diesem Namen schon auf den Branches von S2, S3, S4, S6, S7 und S8 gibt. Der Rest sind Vorschläge.
- **GLOSSAR.md** (neu, 60 Zeilen)
  - 22 Kernbegriffe I2 mit „Nicht verwenden“, technischem Namen und EN.
  - Dazu Rollen, weitergeltende I1-Begriffe und gefundene Abweichungen.

## Nachweis
- `git log e8f000e..HEAD` und `git diff --stat`: 4 Dateien, +456/−12.
- Prüfskript (Python): Codeblöcke in allen 4 Dateien geschlossen, alle Tabellen mit gleicher Spaltenzahl je Zeile.
- Alle 45 Testnamen [da] habe ich per grep gefunden (Schleife über alle Namen, 0 fehlend). Worktrees: `pi-lite-wt/{s2,s3,s4,s6,s7,s8}` (`pi-sync` liegt in `pi-lite-wt/s7`) und `pi-hub-wt/s7`. Szenen `v3-warteschlange`, `v3-sync-auswahl`, `v3-checkout-vorschau` und `v3-abholfach` sind in `v3/s2` vorhanden.
- Wörtliche Texte in F5–F9 habe ich weitgehend aus dem Code von `v3/s2` (`checkin.rs`, `sync_ui.rs`) und `v3/s8` (`seminar.rs`) übernommen. Ausnahme sind die markierten Client-Fehlertexte aus B11.

## Geprüft / ungeprüft
- **Geprüft:**
  - Inhalt gegen FREEZE-I2 E7–E10 und A6–A9.
  - Codes und Wortlaut gegen `PROTOKOLL.md` §I2 (S3) und den Empfang (S7, `empfang.mjs`).
  - Klassen gegen `shared/sync/klassen.json` und `faelle.json`.
- **Ungeprüft:**
  - Keiner der Tests wurde von mir ausgeführt.
  - Mermaid nicht gerendert (kein `mmdc`); die Syntax habe ich nur von Hand geprüft.
  - Die Screenshots T13.11, T14.1, T15.1, T15.8 und T16.4 gibt es noch nicht.

## Offene Fragen
1. **Zweiter Check-in derselben Person** (anderer Rechner, gleiche `pid`): Ich schlage vor, auf den bestehenden Arbeitsplatz zu führen statt einen zweiten zu vergeben (KONZEPT B2). Das Verhalten steht nicht im FREEZE. K entscheidet.
2. **Speicherdauer Seminarchat und DMs** auf den Rechnern: Mein Vorschlag ist flüchtig, bis cocopi geschlossen wird (B10). Das steht nicht im FREEZE.
3. **Fingerabdruck beim ersten Check-in** (FLOWS F5 2E „Empfang vertrauen“): Der FREEZE sagt „angezeigt und bestätigt“. Womit vergleicht eine Studentin den Fingerabdruck (Aushang, Tutorial)? Ohne Vergleichsquelle ist der Schritt eine reine Formsache.
4. Ist der Name **F10/F11** für die alten I1-Flüsse in Ordnung, oder soll K die Nummern anders festlegen?

## Änderungsbitten an andere Rollen
- **S2:**
  - Die Client-Fehlertexte aus KONZEPT B11 bitte übernehmen, wo sie fehlen: `browser_fehlt` (heute „Der Browser konnte nicht geöffnet werden. {e}“ ohne tun), `anmeldung_abgebrochen`, `warteschlange_verfallen`, `empfang_offline`, `ordner_fehlt`, `zu_gross`, `pruefsumme_falsch`, `kein_platz_lokal`, `abholfach_fremd`.
  - Abschlusstext Check-out wie in F7 Schritt 6.
  - Szene `v3-mitsteuern` für T16.4.
  - D-SZ-Text „· eingecheckt“ bzw. „nicht eingecheckt“.
- **S3:** Für `pfad_nicht_erlaubt` bitte das tun „Wähle einen Ordner in ~/cocopi.“ verwenden. Test `freigabe_wartet_auf_prozessende` (T15.10).
- **S7:**
  - Tests T13.8 (Einladung einmalig, nur Hash, nur Seminarleitung), T13.12 (Logs ohne Namen, 7 Tage), T14.6 `upload_same_name_rejected`, T15.3 `erhalten_falscher_hash_loescht_nichts`.
  - Den Fehler `unbekannt` auf `warten/:id` bitte vom Client als „Warteplatz verfallen“ unterscheidbar machen.
- **S8:**
  - Den Dialogtitel „Mitschreiben erlauben?“ in „Mitsteuern erlauben?“ ändern (GLOSSAR).
  - Tests `control_frame_visible_in_every_view`, `dm_only_two_peers`, `unsent_message_stays_in_field`.
- **S5:** `scripts/test-checkout-revokes-mobile.cjs` (T15.9).
- **S6:** Für G3 die T13–T16-Zeilen als Liste ins Gate aufnehmen; SKIP zählt nicht als bestanden.
- **K (Abweichung vom FREEZE, nur Bitte):** In E7 „Warteliste“ durch „Warteschlange“ ersetzen, damit ein Begriff gilt.
