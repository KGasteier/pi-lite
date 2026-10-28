# Entscheidungsprotokoll cocopi-v3 (nur anhängen)

- 28.10.2026 K: E1–E6 aus Entwicklungsprompt übernommen, nicht neu verhandelt.
- 28.10.2026 K: Branch `cocopi-v3` lokal von `cocopi-v2@eb534bb`; Worktrees `../pi-lite-wt/s1…s6` (Branches `v3/s1…s6`), Hub-Worktree `../pi-hub-wt/s3` (Branch `v3/s3` von `master@3dae8ca`). Kein Push.
- 28.10.2026 K: Pfadkorrektur für R2: Tutorial liegt unter `~/cocopi-docs/remote-only-tutorial/` (nicht `/mnt/mac`); Orca read-only geklont nach `/tmp/orca-review` @ `05aab3a`.
- 28.10.2026 K: Provider-Präfix auf R2 ist `udk2/` (nicht `udk/`). terra nicht registriert.
- 28.10.2026 K: `coord/` lebt im Haupt-Worktree (Branch `cocopi-v3`); Subagenten schreiben nur `coord/heartbeat/<rolle>` und `coord/runs/<rolle>-*.md` (Berichte), sonst nichts in `coord/`.
- 28.10.2026 Mensch: Agentenlauf bis Ende I1 freigegeben (D-K1 für I1). S4 nutzt `udk2/cx/gpt-6.1-sol` statt terra (D-T1). Opus (diese Sitzung) koordiniert als K. `coord/` auf `cocopi-v3` committen und pushen erlaubt.
- 28.10.2026 K: S1-Ergebnis (`fdbde9f`) in `cocopi-v3` gemergt. A1–A5 eingefroren in `coord/FREEZE-I1.md` (Konflikt S1/S4 bei Namen/Port entschieden: `accountNN`, Port 88NN). Crate `qrcodegen 1.8.0` freigegeben. Spawn-Sperre (pi-backend) gehört S3, UI S2.

## 28.10.2026 abends · I1-Verlauf
- S1 (opus), S4 (+A4-Folge), S5 fertig; S6 (sol-pro) nach 100 min rc=124: Kontextverdichtung „Request timed out“ 3×. K hat den S6-Stand übernommen (`a66f63a`), Nachauftrag S6 läuft mit **sol statt sol-pro**.
- Gemergt in cocopi-v3: v3/s1, v3/s4, v3/s5, v3/s6. **Noch nicht:** v3/s2, v3/s3.
- Opus-Sicherheitsreview S3: „ja mit Auflagen“, H1 (Handybestätigung umgehbar), H2 (b-Code per SSE), H3 (T3 per spätem Exit) → Nachauftrag I1-S3-fix; S2-fix (mode::init, N4, Clippy, UI-Feinschliff).
- T3: zweite aktive Session wird **abgewiesen** (kein Queue) — gilt für I1; globale Heavy-Job-Queue I2.
- Gate fmt/js-tests scheitern bereits auf eb534bb → S6 macht sie baselinebezogen.
- Laufprotokolle (jsonl/err/status/heartbeat/sessions) werden nicht versioniert (coord/.gitignore).
- 21:xx: S3-fix (H1–H3, M1, M3, M4-Teil, M5, N2, N3, N5), S2-fix (mode::init, N4, Clippy, UI-Feinschliff), S5-fix (Signatur M3), S6-fix (baselinebezogenes Gate) gemergt. Integriertes Gate grün bis auf bekannten Basisfehler B1; no-local-spawn PASS in 4 Szenen; Hub 43 v1-Prüfungen + Smoke grün. **I1 abgeschlossen.**
- Hub-Stand liegt auf pi-hub `v3/s3` (e2eabfa), noch nicht in pi-hub master und nicht installiert (~/.local/share/pi-hub unverändert).
