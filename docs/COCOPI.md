# cocopi · Stand des ersten Prototyps

Branch `cocopi`, Ausgangspunkt copi `b3c89a3`. Rust/egui zuerst; WebKit ausdrücklich später.
Die referenzierte Konzept-PDF wurde nur gelesen, nicht verändert oder abgelegt.

## Was läuft

- `cocopi` verwendet copi-Module direkt (`include!("main.rs")`), kein kopierter UI-Fork.
- Feature `cocopi` ergänzt Netz-Seitenleiste; Agentenstatus unverändert über der Eingabe.
- `pi-net` ohne egui/Tauri-Abhängigkeiten: serialisierbare Karten und Räume, iroh-Transport.
- Echte direkte QUIC-Verbindung mit iroh 1.3.0 / TLS-ring, Minimal-Preset ohne öffentliche Relays/Discovery.
- Start nur nach Klick; Schlüssel je Start flüchtig, nur RAM. Vollständige Kontaktadressen manuell austauschen und beidseitig bestätigen. Adressen enthalten IP-Adressen.
- Chat, Code, Prompt: Senden mit Bestätigung; Empfang ohne Ausführung/KI-Aufruf; Promptübernahme nochmals bestätigen, nur editierbare Eingabe.
- Begrenzte Kartengröße, Steuerzeichen-/Bidi-Bereinigung. Noch keine Dateien.
- Lokale Projekträume / Tutorial-Ablauf / Handheben, ausdrücklich NICHT synchronisiert.
- Seminare gesperrt bis zur verifizierten Lehrendenrolle. Kein selbst vergebenes UdK-Abzeichen.

## Nicht fertig / nicht für vertraulichen Lehreinsatz

Kein MLS, keine authentik-Anmeldung, keine Einmal-Einladungen, keine Gruppenübertragung,
kein Relay-only, keine verschlüsselte dauerhafte Ablage, keine Keychain, keine Offline-Zustellung,
keine Sicherheitswörter, kein Mehrgerätebetrieb. TLS schützt die direkte Verbindung,
ersetzt weder MLS noch die sichere persönliche Prüfung des Kontakt-Schlüssels.
Der Prototyp kann durch langsame Verbindungen verzögert werden (10s Timeout, serieller Worker).
Transportwarteschlange begrenzt, aber Empfangskarten noch ohne Gesamtspeicherlimit.
Kein Produktionsversprechen: zuerst Ressourcenlimits, Protokollzustandsautomat und Review.

## Bauen / testen / Updates

```sh
CARGO_BUILD_JOBS=2 cargo test --offline --locked --workspace --features cocopi --manifest-path copi/Cargo.toml
CARGO_BUILD_JOBS=2 cargo build --offline --locked --release --features cocopi --bin cocopi --manifest-path copi/Cargo.toml
copi/target/release/cocopi
bash copi/scripts/update-cocopi.sh
```

Neue copi-Änderungen landen in derselben Codebasis nach explizitem Upstream-Merge.
Git-Branches übernehmen keine Commits von selbst. Das Update-Skript erledigt Fetch,
Merge, Tests und Build; Konflikte bleiben zur Prüfung stehen. Kein stiller Auto-Update
laufender Sessions, kein Force-Push. Nicht automatisch auf copi/main zurückmergen.

## Tauri vorbereitet, nicht implementiert

`pi-net` ist eigenständiges path-Crate und kann aus `src-tauri/Cargo.toml` verwendet werden.
Gemeinsame DTOs `Card`, `Kind`, `Space`, `SpaceKind` sind serde-kompatibel.
Tauri-Adapter: Node als Mutex-State; explizite Befehle start/stop/pair/send;
Event-Weitergabe über Tauri events; Empfang nie als pi-RPC-prompt dispatchen.
Die Web-UI rendert Karten als Text, kein ungeprüftes HTML. Dieselben Kern- und Consenttests
verwenden; separate WebKit-Interaktion testen. Noch kein behaupteter WebKit-Support.
Branch-Ausnahme von bestehender Paritätsregel gilt nur für cocopi-Kollaboration.

## Priorisierte nächste Entwicklung

1. Transportworker entkoppeln; Ressourcen-/Verbindungs-/Kartenlimits, zwei Rechner im LAN testen.
2. Dauerhafte Ed25519-Identität im Systemschlüsselbund, Schlüsselfingerprints und geprüfte Sicherheitswörter.
3. Eigener Relay-Adapter ohne fremde Defaults; Relay-only auf Netzwerkebene nachweisen.
4. OpenMLS evaluieren/pinnen; Einladungs-, Gruppen-/Epochenverwaltung; Ausschluss-/Replay-/Join-Tests.
5. OIDC-PKCE + signierter Geräteausweis; Rollen serverseitig prüfen. Seminare dann freischalten.
6. Geteilte Projekt-/Tutorialereignisse, Gäste, Dateikarten, verschlüsselte lokale Ablage und Löschfristen.
7. Tauri-Adapter, unabhängiges Security-Review, kleiner freiwilliger Seminarpilot.

## Prüfung

Workspace-Tests mit cocopi-Feature bestanden, einschließlich realer Übertragung zwischen
zwei iroh-Endpunkten auf derselben Maschine. Das ist kein Test zweier physischer Geräte,
kein WAN-/Relaytest und kein UI-Livetest. Neue UI headless separat prüfen.
Version/Lizenz: iroh 1.3.0 (MIT/Apache-2.0), crates.io API lieferte 403;
Version über Cargo Sparse-Index, Quellen und erfolgreichen Cargo-Build geprüft.
OpenMLS noch nicht installiert oder geprüft; keine Versionsbehauptung.
