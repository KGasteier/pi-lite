# Dein Auftrag: S2 · Desktop-UI — I1-S2-desktop (≤ 150 min)

Rolle S2 (§5, §8). Worktree `/home/medienhaus/Work/pi-lite-wt/s2`, Branch `v3/s2`.
Zuerst `git merge --ff-only cocopi-v3` (holt docs/v3 + coord/FREEZE-I1.md).
**Verbindlich:** `/home/medienhaus/Work/pi-lite/coord/FREEZE-I1.md`, `docs/v3/INVENTAR.md` (A5), `docs/v3/FLOWS.md`,
`docs/v3/KONZEPT.md` (Texte wörtlich), `docs/v3/ACCEPTANCE.md`.

Du besitzt `copi/app/**` (inkl. `copi/app/Cargo.toml` für `qrcodegen = "1.8.0"`), `THIRD_PARTY.md` (neu). Nicht: `copi/crates/**` (S3).

Abhängigkeit: S3 baut parallel in `/home/medienhaus/Work/pi-lite-wt/s3` die Spawn-Sperre (`pi_backend::mode`,
`winshim::pi_command(purpose,args) -> Result<…>`) und danach `pi_backend::hub_v1`. Beobachte
`/home/medienhaus/Work/pi-lite/coord/heartbeat/s3` (Zeilen „A2 fertig <commit>“, „A1-v1 fertig“). Sobald A2 fertig ist:
`git merge --no-edit v3/s3` (nur lesen/übernehmen, nichts in crates/ ändern) und app/ an die Signaturen anpassen.
Bis dahin UI-Arbeit, die nicht davon abhängt.

Aufgaben I1 (je Schritt Checkpoint-Commit, Heartbeat):
1. **Inventar umsetzen (A5):** alle als „weg“/Dublette markierten Funktionen aus der Oberfläche entfernen (Code darf hinter
   `#[cfg(feature…)]` oder tot bleiben nur, wenn Entfernen riskant ist — dann im Bericht listen). Insbesondere: pi-Version/
   „pi update“ auf der Startkarte, KI-Hilfsmodell (#12–14, #41), pi-CLI-Befehle (#34), Bookmark-Zyklus/Palette/Sortierung,
   Herkunftsfilter, Backends-Dublette `net_bar_inner` vs `coco_panel` (#45–49). Orte gemäß D-ST/D-SL/D-KZ/D-CH/D-EI/D-DA/D-SZ/D-SE.
2. **Feste Statuszeile D-SZ:** `☁ Nur Remote · <Person> · <slot> @ R2 · <verbunden|offline>` bzw. `⇄ Hybrid …`; Klick → Einstellungen › Modus.
3. **Einstellungen › Modus** mit Bestätigungsdialog Hybrid (Texte wörtlich aus KONZEPT A2); ruft `pi_backend::mode::set`.
   `app/src/net.rs:588`: kein Rückfall auf `BackendCfg::local()`. `SpawnBlocked` → Meldung aus KONZEPT A2.
4. **Startkarte D-ST** „Willkommen bei cocopi“ mit Einladungsfeld + [Anmelden], Zustände prüfen/Erfolg/Fehlertexte wörtlich
   (KONZEPT A3 §1). Bis `hub_v1` da ist: gegen ein Trait/Closure, das S3s Client später füllt; Demo-Szenen mit festen Antworten.
5. **Einstellungen › Geräte**: Liste, [Handy koppeln] mit **QR-Bild** (qrcodegen → egui-Textur) + Code-Text + Countdown,
   [Abmelden …] mit Dialog (Texte wörtlich).
6. **Sessiongrenze (T3, UI):** Fehler `zu_viele_offen` als Text + Knopf „‚{Titel}‘ ruhen lassen“; Zustände ruht/arbeitet/
   wartet auf dich/fertig/Fehler in D-SL (Farben: Akzent `#e454c4` nur für „arbeitet“, Signalfarbe für „wartet auf dich“).
7. **Demo-Szenen** (`COPI_SCENE=`): `v3-start`, `v3-start-fehler`, `v3-main-remote`, `v3-modus`, `v3-hybrid-dialog`, `v3-geraete`,
   `v3-koppeln-qr`, `v3-grenze`. Szenen dürfen **nie** `pi` starten (sie laufen im Modus RemoteOnly; S6 prüft das mit
   einer pi-Attrappe im PATH). Screenshots: `copi/target/…/cocopi --shot /home/medienhaus/Work/pi-lite/coord/shots/I1/<szene>.png --size 1400x880`
   (Verzeichnis anlegen). **Kein** Fenster auf einem Desktop öffnen (R2 hat keinen; `--shot` ist headless).
8. **Plattformkonventionen** in I1 nur: Einstellungen über Menü/⌘, (macOS) bzw. Strg+, ; keine weiteren Kürzel neu.
9. `THIRD_PARTY.md` (Orca-Hinweis aus `docs/v3/ORCA-SYNTHESE.md` §Lizenz; qrcodegen MIT).

Tests: `cargo test --offline -p copi --features cocopi --bin cocopi` + `--bin copi` (ohne Feature) grün; neue Tests für
Statuszeilentext, Modusumschaltung (Dialog nötig für →Hybrid, keiner für →RemoteOnly), Fehlertexte A3, QR-Matrix nicht leer.
Neue CLI-Optionen nur mit Eintrag in `main.rs::check_args` + `args_tests`. Dependency-Änderung: `cargo check --offline`
(ohne --locked), dann Lockfile committen.
Bericht nach `coord/runs/I1-S2-desktop-bericht.md`, mit Screenshot-Liste.
