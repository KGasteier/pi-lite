# Entscheidungsprotokoll cocopi-v3 (nur anhängen)

- 28.10.2026 K: E1–E6 aus Entwicklungsprompt übernommen, nicht neu verhandelt.
- 28.10.2026 K: Branch `cocopi-v3` lokal von `cocopi-v2@eb534bb`; Worktrees `../pi-lite-wt/s1…s6` (Branches `v3/s1…s6`), Hub-Worktree `../pi-hub-wt/s3` (Branch `v3/s3` von `master@3dae8ca`). Kein Push.
- 28.10.2026 K: Pfadkorrektur für R2: Tutorial liegt unter `~/cocopi-docs/remote-only-tutorial/` (nicht `/mnt/mac`); Orca read-only geklont nach `/tmp/orca-review` @ `05aab3a`.
- 28.10.2026 K: Provider-Präfix auf R2 ist `udk2/` (nicht `udk/`). terra nicht registriert.
- 28.10.2026 K: `coord/` lebt im Haupt-Worktree (Branch `cocopi-v3`); Subagenten schreiben nur `coord/heartbeat/<rolle>` und `coord/runs/<rolle>-*.md` (Berichte), sonst nichts in `coord/`.
- 28.10.2026 Mensch: Agentenlauf bis Ende I1 freigegeben (D-K1 für I1). S4 nutzt `udk2/cx/gpt-6.1-sol` statt terra (D-T1). Opus (diese Sitzung) koordiniert als K. `coord/` auf `cocopi-v3` committen und pushen erlaubt.
- 28.10.2026 K: S1-Ergebnis (`fdbde9f`) in `cocopi-v3` gemergt. A1–A5 eingefroren in `coord/FREEZE-I1.md` (Konflikt S1/S4 bei Namen/Port entschieden: `accountNN`, Port 88NN). Crate `qrcodegen 1.8.0` freigegeben. Spawn-Sperre (pi-backend) gehört S3, UI S2.
