# Dein Auftrag: S8 · Seminar: P2P-Chat, DMs, Mitsteuern im v3-Desktop — I2-S8-seminar (≤ 150 min)
Neue Rolle S8. Worktree `/home/medienhaus/Work/pi-lite-wt/s8` (`v3/s8`), Build `target-s8`.
Lies `docs/COCOPI_V2.md`, `docs/COCOPI_SECURITY.md`, `copi/app/src/{coco.rs,coco_view.rs}` (v2-Seminar: Kontakte,
Gruppen, DMs, Remote View/Control, A2A) und `copi/crates/pi-net/src/{remote.rs,node.rs,model.rs,store.rs}`.
1. `copi/crates/pi-net/src/ausweis.rs` (neu, additiv in `lib.rs`): Geräteausweis nach A6 prüfen (Ed25519, kanonisches JSON,
   exp/iat, `net_pubkey` == Peer-Schlüssel), Rolle als Enum `Seminarleitung|Teilnehmende|Gast`. Wire additiv: Ausweis beim
   Verbindungsaufbau austauschen; ohne gültigen Ausweis nur „unbestätigt“ (kein Mitsteuern, Chat markiert).
2. `copi/app/src/desktop/seminar.rs`: `pub struct Seminar` mit `ui(&mut self, ui)` und reinem Zustandsmodell, testbar ohne
   Netz (Trait `SeminarNet`, In-Memory-Mock). Ansicht: links Teilnehmende (Name, Rolle-Abzeichen, online, eingecheckt-Slot),
   rechts Gruppenchat „Seminar“ bzw. DM; oben je Person Knöpfe „Nachricht“, für Seminarleitung zusätzlich „Ansehen“ und
   „Mitsteuern anfragen“ (Auswahl der Session). Wiederverwendung der v2-Logik (`coco.rs`/RemoteBook) wo möglich, nicht
   kopieren-und-verändern: gemeinsame Funktionen in pi-net extrahieren ist erlaubt (additiv).
3. Mitsteuern nach A9: Anfrage-Dialog bei Teilnehmenden (Ablehnen fokussiert, 30 min / nur ansehen / ablehnen), aktiver
   Zustand mit Rahmen `#4fb8d8` und Leiste inkl. Restzeit + „Beenden“; Seminarleitung: Leiste „Du steuerst …“ + „Zurückgeben“,
   eigene Eingabe geht als `Steuern{text}` an den Peer; der Client der Teilnehmenden leitet als `senden{text,von}` an seinen
   Hub (Schnittstelle zu S2: `Desktop::send_as(von, text)`, als Bitte im Bericht, bis dahin über Trait gemockt).
   Chatanzeige „Aylin · Seminarleitung“ mit Farbmarke. Ende bei Ablauf/Beenden/Verbindungsverlust > 60 s/Check-out.
4. Demo-Szenen `v3-seminar-chat`, `v3-seminar-dm`, `v3-mitsteuern-anfrage`, `v3-mitsteuern-aktiv-student`,
   `v3-mitsteuern-aktiv-leitung`; Szenenregistrierung über eine Funktion `seminar::scene(name)->Option<Seminar>`, die S2
   in `Desktop::scene` aufruft (Bitte an S2/K). Screenshots via eigenem Test-Harness nach `coord/shots/I2/`.
Tests: ohne Ausweis kein Mitsteuern; Rolle Teilnehmende kann nicht anfragen; Ablehnen speichert nichts; Ablauf beendet;
Prompt der Leitung erscheint markiert. Bericht `coord/runs/I2-S8-seminar-bericht.md`.
