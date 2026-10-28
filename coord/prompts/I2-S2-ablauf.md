# Dein Auftrag: S2 · Desktop-Ablauf Check-in → Sync → Arbeiten → Check-out — I2-S2-ablauf (≤ 150 min)
Worktree `/home/medienhaus/Work/pi-lite-wt/s2` (aktuell `cocopi-v3`). Lies dazu `docs/v3/FLOWS.md`, FREEZE-I2.
Neue Startkarte statt „Einladung einfügen“: **„Einchecken mit UdK-Konto“** (Hauptaktion, authentik) und darunter klein
„Ich habe einen Einladungscode“ (Gast). Ablauf in **einer** Kartenfolge, je Schritt eine Hauptaktion:
1. Check-in → System-Browser (Callback-Schnittstelle), Status „Warte auf Anmeldung im Browser …“ + Abbrechen.
2. Warteschlange: „Alle 10 Arbeitsplätze sind belegt. Du bist Nr. N.“ ruhig, ohne Klick nachrückend, Abbrechen.
3. Sync-Auswahl: Liste aus `~/cocopi/` (Projekte einzeln, Skills, Verläufe, Ergebnisse) mit Häkchen, Größe,
   „Builds/Cache werden nicht übertragen (x MB)“; Standard: zuletzt benutztes Projekt; „Ohne Dateien starten“ gleichrangig klein.
   Fortschritt mit Abbruch.
4. Arbeiten (bestehende Hauptansicht). Statuszeile: „Nur Remote · Mira · account03 @ R2 · eingecheckt seit 14:02“.
   Menü/Einstellungen: **„Auschecken …“** an genau einem Ort (Kopfzeile rechts, Knopf, nicht im Menü versteckt).
5. Check-out-Vorschau: zwei Spalten „Kommt zurück auf deinen Rechner“ (Liste, Größe, neu/geändert) und „Wird gelöscht“
   (Builds, Cache … mit Summe, aufklappbar); Hinweis „Danach ist account03 leer und für andere frei. Auf R2 bleibt nichts.“
   Hauptaktion „Auschecken und Dateien holen“. Fortschritt, dann Abschluss „Alles zurück in ~/cocopi · Ordner öffnen“.
   Konflikte sichtbar: „2 Dateien hattest du lokal geändert – die R2-Fassung liegt daneben.“
6. Abholfach-Hinweis beim nächsten Check-in, falls vorhanden (Ablauf „Dateien vom letzten Mal abholen“).
Technik: `DesktopApi` um Requests `Checkin`, `Wait`, `SyncPlan`, `Upload`, `CheckoutPreview`, `Checkout` erweitern;
In-Memory-Adapter + Demo-Szenen `v3-checkin`, `v3-warteschlange`, `v3-sync-auswahl`, `v3-arbeiten-eingecheckt`,
`v3-checkout-vorschau`, `v3-checkout-fertig`, `v3-seminar-platzhalter` (Tab „Seminar“ neben Chat/Dateien ruft
`fn seminar_tab(&mut self, ui)` auf – leer mit Hinweis, S8 füllt es). Echter Adapter nutzt später S3 `empfang.rs`
+ S7 `pi-sync`; bis dahin Trait-Grenze sauber halten, kein eigener HTTP-Code. Screenshots `coord/shots/I2/`.
Tests (Ablaufzustände, Abbruch in jedem Schritt, kein Upload ohne Bestätigung, Auschecken nur mit Vorschau). rustfmt nur
eigene Dateien. Bericht `coord/runs/I2-S2-ablauf-bericht.md`.
