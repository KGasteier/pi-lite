# Dein Auftrag: S2 · Nachbesserung — I1-S2-fix (≤ 90 min)

Rolle S2, Worktree `/home/medienhaus/Work/pi-lite-wt/s2` (`v3/s2`, Stand `9c8857c`).
Lies `coord/runs/I1-S3-review-bericht.md` (N4, „mode::init“) und deinen eigenen Bericht. S3 arbeitet parallel an
`I1-S3-fix` (typisierte Fehlercodes `[code] was tun` bzw. HubError, Signatur mit Kontextbindung). Heartbeat
`coord/heartbeat/s3` meldet „FIX fertig <commit>“; dann `git merge --no-edit v3/s3` und den Adapter auf die Codes umstellen
(exakte A3-Texte und Limitdialog jetzt auch für echte HTTP-Fehler). Bis dahin:

1. `pi_backend::mode::init()` beim App-Start **vor** jedem Backend-Aufbau aufrufen + Test (Reihenfolge).
2. **N4:** `app/src/net.rs` bei kaputter `backends.json` → `backends::parse("[]")`, kein `BackendCfg::local()` + Test.
3. Gate-Befunde in `copi/app/src/desktop.rs` beheben: `clippy::possible_missing_else` bei Zeilen ~656, ~871, ~902
   (prüfe, ob das ein echter Logikfehler ist — insbesondere ob „Abbrechen“ im Koppel-Dialog immer erreichbar ist —
   und schreibe Zeilenumbrüche/Blöcke lesbar, keine Einzeiler-Kaskaden); dead_code `Offline`, `Messages`, Feld `0` bei ~105
   entfernen oder nutzen. Ziel: in deinen Dateien **keine neuen** Clippy-Warnungen gegenüber `eb534bb`.
4. Deine neuen Dateien (`desktop.rs`, `desktop/hub.rs`) mit `rustfmt --edition 2021` formatieren (kein workspaceweites fmt).
5. UI-Feinschliff aus den Screenshots (K-Befund): Chat zeigt „Mira“/„cocopi · R2“ ohne Abstand/Trenner — Sprecher als
   eigene, dezente Zeile oder fett, Nachrichten mit etwas Innenabstand; Sessionliste: Zustand und „…“-Knopf in einer Zeile mit dem
   Titel, ausgewählte Session klar hinterlegt; Koppel-QR-Dialog nicht über dem Einstellungsfenster verschachtelt erscheinen
   lassen (Einstellungen dahinter abdunkeln oder schließen); Modellzeile zeigt „udk2/…“ — zeige den Anzeigenamen aus der
   Freigabeliste oder „Modell: Standard“. Screenshots neu erzeugen (gleiche Szenen, `coord/shots/I1/`).
Tests: cocopi + copi Binaries grün. Bericht nach `coord/runs/I1-S2-fix-bericht.md`.
