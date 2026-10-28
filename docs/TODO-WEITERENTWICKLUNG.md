# cocopi · Todo Weiterentwicklung

Stand: 26.10.2026 · Priorisierte Arbeitsliste; nicht aus älteren Checklisten automatisch als erledigt übernommen. Kontext: [ONBOARDING.md](ONBOARDING.md), Qualitätsgates und Rechte: [ROADMAP-COCOPI.md](ROADMAP-COCOPI.md).

## Erledigt in dieser Dokumentationsarbeit

- [x] Lokalen und R2-Gitstand lesend verglichen: cocopi `b65a1e7`, Hub `2060b1c`; bestehende untracked Nutzerpfade erhalten.
- [x] ARM-Installation gegen vorhandene Release-Dateien per SHA256 geprüft (kein Neubau); beide identisch.
- [x] Vollständige installierte pi-1.4.0-Skill-/RPC-Dokumentation samt einschlägigen Ressourcen-/Trust-/Befehlsreferenzen gelesen. RPC-Skill-Expansion und Discovery dokumentiert, keinen Prompt gesendet.
- [x] Skill-/Agenten-/Extension-Dateiinventar per Hash mit R2 verglichen, ohne Credentials auszugeben. Alle 16 vorhandenen R2-Skill-Dateien vor Aktualisierung identisch; alle fünf Agentendateien identisch; sieben lokale Extension-Dateien fehlen im R2-Nutzerverzeichnis.
- [x] Laufenden Hub nur lesend geprüft: HOME/PATH/cwd, pi-Shim auffindbar und beobachteter Kindprozess-cwd getrennt dokumentiert.
- [x] Sicheres kuratiertes `scripts/sync-r2.sh` angelegt; ausschließlich Dry-Run, kein Quellrepo-Transfer. copidev nach Backup konsolidiert.
- [x] Zielbild, Roadmap, Rollenmatrix, Sicherheits-/Abnahmekriterien und deutsche md/txt-Übergabe erstellt.

## P0 · Vor weiterer Ausführung / Übergabe

- [ ] R2-pi-Version hinter mise-Shim und effektive Skill-Discovery der gewünschten Instanz kostenfrei prüfen (`get_commands` nur nach Freigabe eines separaten Diagnosekanals; keine fremde Session stören). Fehlende Extensions nicht mit fehlenden Skills verwechseln. Projektvertrauen prüfen, nicht blind `--approve` setzen.
- [ ] Neuen copidev-Diff lokal prüfen, Dry-Run bewerten; `--apply` nur ausdrücklich freigeben. R2 bleibt bis dahin beim alten Skill. Agenten- und Extension-Abhängigkeiten separat prüfen; Subagentdefinitionen allein registrieren kein Werkzeug.
- [ ] Fachstatus `HANDOFF.md`, `COCOPI.md` sowie Hub `STATUS.md`/`TODO.md` gezielt konsolidieren: historische „nie gebaut“-/Persistenz-/Installationsaussagen, bereits vorhandene Instances-Integration und Sessionaktionen nicht pauschal erneut implementieren. Neue Onboarding-Angaben sind die aktuelle Bestandsaufnahme.
- [ ] ADRs/Threat Model für Gesprächsidentitäten, Sitzungseigentümerschaft, Rollen, E2EE, NAT/Relay, Offline und A2A anlegen; Betreiber-/Nutzerentscheidungen einholen. Noch keine Dependencywahl als entschieden behandeln.

## P1 · Remote-Hauptchat abnehmen (Roadmap Phase 1)

- [ ] Mocktests für Routing bei Leiste aus, Raster an/aus, fehlender Instanz und Backendwechsel erweitern: Senden/Abbruch/Modell/Workspace bleiben am sichtbaren Ziel.
- [ ] Lange Remote-Verläufe virtualisieren und UI-Ressourcenlimits messen.
- [ ] SSE-Abbruch, Replayduplikate, Ringpufferlücke und Session-Nachladen testen; Lücken explizit anzeigen statt Vollständigkeit vortäuschen.
- [ ] Hubregistry/Neustart mit Testsession abnehmen: neue Prozesse statt RAM-Fortsetzung, keine doppelte Session-Schreiberschaft, offene Dialoge sicher rekonstruieren/abbrechen.
- [ ] Echten ARM-Fenstertest mit Nutzerfreigabe planen; lokales Vorlesen hörbar prüfen. Kein ungefragter Desktop-/Tastaturzugriff.
- [ ] Erst nach Kostenfreigabe einen harmlosen echten Remote-Prompt mit Streaming, Abbruch und Extension-Rückfrage abnehmen. Testprotokoll ohne private Inhalte.
- [ ] copi-/pi-lite-Remoteparität konkret erfassen; Tauri-Remote-Hauptchat-/Workspace-/Modelllücken separat umsetzen und in echtem WebKit prüfen.

## P2 · Menschen-DMs und Transportgrenzen (Phase 2)

- [ ] Seriellen Worker entkoppeln; Gesamt-/Peer-/Karten-/Queue-/Verbindungslimits und Timeouts mit Flood-/Slow-Peer-Tests einführen.
- [ ] Zwei physische Geräte im LAN testen, beide Richtungen, Stop/Neupaarung, Promptübernahme ohne Modellversand, kein automatisches Öffnen/Ausführen.
- [ ] Persistente Geräteidentität/Systemschlüsselbund, Fingerprint-/Sicherheitswortprüfung, Rotation/Verlust/Restore und Blockliste entwerfen und testen.
- [ ] Kontakt-/DM-Liste mit stabilen IDs und menschlich/agentisch/remote-Markierung; Zustell-/Lesestatus und klare Nichtzustellbarkeit implementieren.
- [ ] Privaten Agentenchat, DM und Gruppe strikt trennen; Teilung nur mit Inhalt-/Empfängerfreigabe.

## P3 · Gruppen, Rollen und Infrastruktur (Phasen 3–4)

- [ ] Relayversion/Clientkompatibilität evaluieren; Betreiber, DNS/TLS, Limits, minimale Logs und Datenschutz klären. NAT-/WAN-/Relay-only-Testmatrix praktisch durchführen.
- [ ] Gruppen-E2EE-Bibliothek (z.B. OpenMLS) nach aktueller Versions-/Lizenz-/API-Prüfung auswählen; keine eigene Kryptografie. Join/Leave/Kick/Epochen/Replay/parallel Updates testen.
- [ ] OIDC-PKCE, Gerätebesitz-Challenge, kurzlebiger signierter Ausweis, serverseitige Kursrolle, Widerruf und Gäste abnehmen. OIDC-/KI-Token getrennt halten.
- [ ] Einmal-Einladungen mit Rechten, Ablauf, Widerruf und Replay-Schutz implementieren; vollständige Identitäts-/Raumprüfung vor Beitritt.
- [ ] Gruppen humans/agents/mixed und DMs mit Menschen/Agenten; negative Rechteprüfungen laut Roadmapmatrix (auch Leitung darf keine privaten DMs lesen).
- [ ] Gleichberechtigte Entwicklerrechte, temporäre Mandate/Quorum und Konfliktfälle spezifizieren; keine verdeckte permanente Leitung einbauen.
- [ ] Chat-Verwaltung: erstellen, benennen, filtern, suchen, ungelesen, pinnen, archivieren, verlassen, blockieren/melden, exportieren/lokal löschen und Mitgliedschaft verwalten.
- [ ] Synchronisierte Projekte/Tutorials/Handheben von lokalen Platzhaltern ablösen.
- [ ] Offline-Vertrag entscheiden: zunächst ehrlich nicht zustellbar; später ggf. begrenztes Chiffratpostfach, TTL/Quoten, Deduplizierung und Epochen-/Widerrufskonflikte. Keine Exactly-once-Zusage ohne Nachweis.
- [ ] Lokale Verschlüsselung, Löschfristen, Mehrgeräte-/Recoverystrategie und unabhängiges Securityreview vor Seminarfreigabe.

## P4 · Agent2Agent für Entwicklung (Phase 5)

- [ ] Erst native begrenzte Aufgabenvergabe vs. interoperables A2A-Protokoll entscheiden; konkrete Spezifikation/Version/Adapter prüfen. MCP nur als Werkzeug-/Ressourcenebene behandeln.
- [ ] Task-/Trace-/Parent-IDs, Capabilities, Besitzer, Workspacegrenzen, Artefakthashes, Deadline, Status/Abbruch und idempotente Wiederholung definieren.
- [ ] Serverseitige harte Kosten-/Token-/Zeit-/Parallelitäts-/Retry-/Delegationstiefen-/Fan-out-Limits plus globalen Not-Aus implementieren.
- [ ] Zwei Mockagenten mit abgeschlossener Entwicklungsaufgabe testen: Zyklus, Duplikat, Timeout, Preis unbekannt, Budget erschöpft, Agent weg und feindliche Toolausgabe.
- [ ] Mensch-Freigabe für Providertransfer, Schreibrechteausweitung, Kostenanhebung und Veröffentlichung; keine autonomen Pushes/Deploys, keine Credentials/Privatsessions als Taskkontext.
- [ ] Erst mit expliziter Freigabe Provider-Livetest mit kleinem Budget; tatsächliche Kosten inkl. Nebenagenten/Compaction/Retry erfassen.

## P5 · Persona, gemischte Gruppen und Pilot (Phasen 6–7)

- [ ] Persona-Dateiformat/Versionierung/Importprüfung festlegen; Geheimnisse getrennt, Capability- und Budgetobergrenzen nicht durch Persona-Text überschreibbar.
- [ ] Verwaltung erstellen/importieren/prüfen/bearbeiten/revisionieren/testen/aktivieren/pausieren/widerrufen/löschen; ausgeführte Revision per Hash festhalten.
- [ ] Räume/DMs mit Personaagenten sichtbar kennzeichnen, Kontexttransfer freigeben, Agentenreaktionen adressiert statt selbstlaufend; keine autonome Vermehrung oder Einladung.
- [ ] Prompt-Injection-/Rechteausweitungs-/Schleifentests und Datenschutz-/Protokollreview bestehen.
- [ ] Freiwilligen kleinen Pilot mit Ansprechpartner, Schlüsselverlust-/Ausschlussübung, Export/Löschung und Nicht-KI-Alternative durchführen.
- [ ] Tauri-P2P-Adapter und mobile Clients erst nach stabilem Kern; echte WebKit-/Android-/iOS-Tests getrennt ausweisen.

## Arbeitsregel für jede nächste Tranche

1. R2-/ARM-Gitstand und Arbeitsbereich lesen; Nutzeränderungen nicht überschreiben. Konkretes kleines Paket und Akzeptanztest auswählen; kein cc-style-Verfahren erforderlich.
2. R2 entwickelt in abgesprochenem Branch/Workspace; ARM überwacht Logs und prüft Diff/Testbefund. Keine automatische Quellcodeüberschreibung oder parallele Session-Schreiberschaft.
3. Geprüfte Quellen/Commits/Patches nach ARM übernehmen, keine x86_64-Artefakte/Targets. ARM baut nur nach Buildauftrag und Ressourcenprüfung; atomare Installation separat mit Hashvergleich und Nutzerneustart.
4. Mock/Unit/Headless/Live und Plattformen sauber trennen. Modellkosten, Datenübertragungen, Neustart, Commit und Push nur gemäß Auftrag. Offenes ausdrücklich dokumentieren.

## Nicht durch diese Arbeit geprüft

Keine neuen Rust-/JS-Tests oder Builds; keine neue Installation, keine Interaktion mit Nutzerfenstern, kein Remote-Modellchat, kein Reconnect-/Hubneustartversuch, kein WAN-/Relay-/Gruppen-/E2EE-/A2A-/Persona-Livetest. Der aktive R2-Dienst und Dateihashes ersetzen diese Nachweise nicht.
