# cocopi-v3 · Testversion selbst bauen

Diese Anleitung ist für **Testerinnen und Tester**, die cocopi-v3 auf dem eigenen Rechner bauen wollen.
Sie gilt für ARM64 (z. B. Omarchy-VM auf Apple Silicon, Raspberry Pi 5) und x86_64, Linux und macOS.
Es gibt **keine fertigen Binärdateien**: Jeder Rechner baut aus dem Quellcode, so passt die Datei sicher zur eigenen Architektur.

## 1. Voraussetzungen

| Was | Linux (Arch/Omarchy) | macOS |
|---|---|---|
| Rust | `mise use -g rust@stable` oder https://rustup.rs | ebenso |
| C-Compiler | `sudo pacman -S base-devel` | `xcode-select --install` |
| Speicher | etwa 6 GB frei | etwa 6 GB frei |
| Zeit | erster Build 5–20 min (ARM: eher 20) | 5–15 min |

## 2. Holen und bauen

```sh
git clone -b cocopi-v3 https://github.com/KGasteier/pi-lite.git
cd pi-lite
bash copi/scripts/build-testversion.sh             # nur bauen + Selbsttest
bash copi/scripts/build-testversion.sh --install   # zusätzlich ~/.local/bin/cocopi (alte Datei → .bak)
```

Wer schon einen Klon hat:

```sh
git fetch origin && git switch cocopi-v3 && git pull --ff-only
```

Auf schwachen Rechnern `CARGO_BUILD_JOBS=2` lassen. Bei 8 GB RAM nicht höher als 2 gehen.

## 3. Ohne Server ansehen: Demo-Szenen

Jede Ansicht lässt sich ohne R2, ohne Netz und ohne Anmeldung mit erfundenen Daten öffnen:

```sh
COPI_SCENE=v3-checkin cocopi             # Fenster mit der Szene
COPI_SCENE=v3-checkin cocopi --shot a.png  # nur Bild speichern
```

| Szene | Was man sieht |
|---|---|
| `v3-start` | Startbildschirm, Anmelden bei der UdK |
| `v3-start-fehler` | Anmeldung fehlgeschlagen, mit verständlicher Hilfe |
| `v3-checkin` | Arbeitsplatz wird zugewiesen |
| `v3-warteschlange` | Alle zehn Plätze belegt, Position in der Warteschlange |
| `v3-sync-auswahl` | Welche Ordner aus `~/cocopi` mitgenommen werden (Großdateien einzeln) |
| `v3-sync-fortschritt` | Hochladen läuft |
| `v3-arbeiten-eingecheckt` | Normales Arbeiten auf dem R2-Arbeitsplatz |
| `v3-checkout-vorschau` | Was beim Auschecken zurückkommt |
| `v3-checkout-fortschritt` | Zurückspielen läuft |
| `v3-checkout-konflikt` | Eine Datei wurde beidseitig geändert, die R2-Fassung liegt daneben |
| `v3-checkout-fertig` | Ausgecheckt, Arbeitsplatz ist leer |
| `v3-abholfach` | Nach einem Zwangs-Check-out: verschlüsseltes Abholfach |
| `v3-seminar-chat` | Gruppenchat im Seminar |
| `v3-seminar-dm` | Direktnachricht |
| `v3-mitsteuern-anfrage` | Seminarleitung bittet ums Mitsteuern |
| `v3-mitsteuern-aktiv-student` | Mitsteuern läuft (Sicht Teilnehmende, cyanfarbener Rahmen) |
| `v3-mitsteuern-aktiv-leitung` | Mitsteuern läuft (Sicht Seminarleitung) |
| `v3-koppeln-qr` | Handy per QR koppeln |
| `v3-geraete` | Gekoppelte Geräte verwalten |
| `v3-main-remote` | Remote-Session (klassisch) |
| `v3-grenze` | Hinweis auf eine Grenze (z. B. Speicher) |

## 4. Was die Testversion kann und was nicht

- **Kann:** alle Ansichten, Demo-Daten, lokale Ordnerauswahl, Remote-Sessions zu einem bestehenden pi-hub, P2P-Chat im LAN.
- **Noch nicht:** produktiver Pool-Betrieb auf R2. Accounts, systemd und Quoten sind noch nicht eingerichtet (Freigabe ausstehend).
- **Noch nicht:** authentik-Anmeldung, weil die App bei auth.udk.digital noch nicht angelegt ist.
- Rückmeldungen bitte mit Szene/Name, Rechner (`uname -m`) und Screenshot.

## 5. Probleme

| Meldung | Lösung |
|---|---|
| `Rust/Cargo fehlt` | Rust installieren (siehe 1.) und neues Terminal öffnen |
| `linker cc not found` | C-Compiler installieren |
| Build bricht mit „killed“ ab | zu wenig RAM: `CARGO_BUILD_JOBS=1` |
| Fenster bleibt schwarz | Wayland/X11 fehlt (z. B. in SSH-Sitzung); lokal am Desktop starten |
| `--shot` geht nicht | ohne GPU normal, das Fenster selbst funktioniert |
