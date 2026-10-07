# Checkliste: Veröffentlichung auf GitHub

Noch **nicht** veröffentlicht. Der Nutzer entscheidet, wann die Version reif genug ist.

## Vorher klären
- [ ] **Lizenz wählen** (z. B. MIT) und `LICENSE` anlegen; `Cargo.toml` (`license`) und README anpassen
- [ ] Lizenztexte der Vendor-Bibliotheken beilegen (`ui/vendor/LICENSES/`) – siehe `THIRD_PARTY.md`
- [ ] Verhältnis zu hyperpuncher/pi-ui klären: eigene Entwicklung, ähnliches Verhalten; kein Code übernommen
      (pi-ui-Repo wurde nicht angesehen). Verweis im README genügt, ggf. dem Autor Bescheid geben.
- [ ] Projektname prüfen (`pi-lite` – Kollisionen mit bestehenden Projekten?)
- [ ] `identifier` in `tauri.conf.json` (`dev.kg.pilite`) anpassen, falls eine andere Kennung gewünscht ist
- [ ] Persönliches entfernen: Pfade wie `/home/user`, E-Mail in der Git-Konfiguration, Hyprland-Hinweise
      in `docs/TESTING.md` auf das Nötige kürzen
- [ ] Prozess-Session (siehe `DEVELOPMENT.md`) sichten, bevor sie jemand bekommt

## Technisch vorbereiten
- [ ] Mindestens Rust-Unit-Tests für `read_info` und `delete_session` (Pfadprüfung!)
- [ ] macOS und Windows bauen und prüfen (`Cmd+B`, `Backspace`, Pfade mit `\`, `USERPROFILE`)
- [ ] Rechtsklick-Menü mit Maus prüfen
- [ ] `bundle.active` ist derzeit `false`; für Releases auf `true` und Ziele festlegen (AppImage, dmg, msi)
- [ ] GitHub Actions: Build-Matrix Linux/macOS/Windows (`tauri-apps/tauri-action`)
- [ ] Screenshot(s) ins README
- [ ] Versionsnummer, `CHANGELOG.md`

## Veröffentlichen
```bash
cd ~/Work/pi-lite
git log --oneline                       # Verlauf prüfen
gh repo create <name> --public --source=. --remote=origin   # oder manuell auf github.com anlegen
git push -u origin main
```
(`gh` ist möglicherweise nicht installiert; sonst Repo auf github.com anlegen und `git remote add origin …`.)
