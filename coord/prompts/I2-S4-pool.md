# Dein Auftrag: S4 · R2-Pool, Empfang, Leeren, authentik-Anleitung — I2-S4-pool (≤ 120 min)
Worktree `/home/medienhaus/Work/pi-lite-wt/s4`. Alles `--dry-run` Standard, nichts auf R2 ausführen (D-R1).
1. Umbau auf A8: System-Units `deploy/r2/systemd/cocopi-hub@.service` (User=%i, Slice, Env aus `/etc/cocopi/hub-%i.env`
   inkl. `HUB_EMPFANG_PUBKEY`, `HUB_ROOTS`), `cocopi-empfang.service` (Systemnutzer, `/var/lib/cocopi-empfang`), Slices.
2. `deploy/r2/cocopi-slotctl` (root-Helfer, Befehle `leeren|status|pruefen`, strenge Slot-Prüfung `^account(0[1-9]|10)$`,
   kein Symlink-Folgen, Skelett `/etc/cocopi/skel/`, Nachweis „leer“), sudoers-Datei genau dafür, Tests mit Fake-Root
   (Temp-Verzeichnis, `COCOPI_ROOT`-Präfix) inkl. Angriffsfälle (Symlink nach /etc, Slot `../`, laufende Prozesse).
3. `deploy/r2/einrichten.sh` (Gesamtablauf, Schritte nummeriert, Dry-Run-Ausgabe zum Lesen, `--ausfuehren`),
   `deploy/r2/proxy/` Vorlage Caddy **und** `tailscale serve`-Variante (Pfade `/v1/empfang/*`, `/slot/accountNN/*`,
   Sperre für `/v1/belegung`, `/v1/freigabe` von außen), `deploy/r2/AUTHENTIK.md` (Provider/Anwendung anlegen Schritt für
   Schritt: öffentlicher Client, PKCE, Redirect `http://127.0.0.1/*` (Loopback, beliebiger Port), Scopes `openid profile`
   + Gruppen-Claim, Gruppen `cocopi-seminarleitung`/`cocopi-teilnehmende`, Werte für `/etc/cocopi/empfang.env`).
4. `deploy/r2/EINRICHTEN-MIT-PI.md`: ein Prompt, den man in pi auf R2 einfügt; pi geht dialogisch durch, zeigt jeden
   Befehl, holt vor jedem `sudo` ein Ja, führt zuerst `--dry-run` aus, prüft danach mit `admin-check.sh`.
5. `admin-check.sh` erweitert: Pool-Status, alle Slots leer/belegt, Quoten, Empfang erreichbar, Proxy-Sperren.
6. Datenschutz-Merkblatt `deploy/r2/DATENSCHUTZ.md` (No-Retention, was wann gelöscht wird, Abholfach 72 h, Logs,
   Ansprechpartner-Platzhalter) in einfacher Sprache.
Tests `python3 deploy/r2/test_scripts.py` grün. Bericht `coord/runs/I2-S4-pool-bericht.md`.
