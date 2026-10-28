# cocopi · Roadmap zur gemeinsamen Arbeitsumgebung

Stand: 26.10.2026 · Planungsdokument, keine Zusage bereits implementierter Funktionen. Geprüfter Ausgangsstand und RPC-Skill-Frage: [ONBOARDING.md](ONBOARDING.md). Arbeitspakete: [TODO-WEITERENTWICKLUNG.md](TODO-WEITERENTWICKLUNG.md).

## 1. Produktziel und Abgrenzung

cocopi verbindet in einer konsistenten Oberfläche den persönlichen Agenten-Hauptchat (lokal/remote), menschliche P2P-Kontakte, Gruppen und später Agenten-Kollaboration. Mensch, Agent, Eigentümer, ausführender Host und Zielgespräch bleiben sichtbar. Eine Nachricht an einen Menschen darf nie versehentlich zum Modellprompt werden; ein Remote-Hauptchat darf beim Ausblenden einer Leiste nie lokal weitersenden.

Zwei Arbeitskontexte:

- **Seminar:** verifizierte Seminarleitung und Teilnehmer; klar begrenzte Moderation, freiwillige Teilnahme, zugängliche Alternative. Leitungsrolle gibt weder Leserecht für private DMs noch Zugriff auf private Agentensessions.
- **Entwicklung:** gleichberechtigte Menschen und explizit beauftragte Agenten. Keine permanente Leiterrolle; temporäre, sichtbare Verwaltungsmandate können gemeinschaftlich vergeben und widerrufen werden. Gleichberechtigung bedeutet nicht gemeinsame API-Schlüssel oder unkontrollierte Schreibrechte.

Gruppenarten **humans**, **agents**, **mixed**; DMs Mensch↔Mensch, Mensch↔Agent sowie freigegebene Agent↔Agent-Aufgaben. Auch die Bedienperson kann direkt einem entfernten Agenten schreiben. Agents-only-Gruppen brauchen trotzdem einen verantwortlichen Menschen und erreichbaren Not-Aus.

## 2. Schichten und offene Entscheidungen

| Schicht | Heute / Kandidat | Entscheidung und Nachweis noch nötig |
|---|---|---|
| Persönlicher Remote-Agent | pi-RPC über Node-Hub, HTTP/SSE; ARM-Client | Replay-Lücken, Rückfragen und Sitzungsbesitz abnehmen |
| P2P-Karten | iroh 1.3.0 direkt, flüchtige Identität | Limits, langlebige Identität, zwei Geräte |
| Gruppen-E2EE | OpenMLS als Evaluationskandidat, nicht eingebaut | Bibliotheksversion, Protokoll, Schlüssel-/Epochenspeicher, Review |
| Identität/Rollen | authentik/OIDC-PKCE + Geräteausweis als Konzept | Betreiber, Claims, Rollenmapping, Wiederherstellung und Widerruf |
| NAT/Relay | eigener Relay-Adapter geplant | Version, Betrieb, Datenschutz, Relay-only-Netznachweis |
| Speicherung | pi-Sessions + Hubregistry; P2P nicht dauerhaft | lokale Verschlüsselung, Gruppenlog, Offline-Postfach, Fristen |
| A2A | möglicher interoperabler Aufgabenadapter | konkrete Spezifikation/Version, Bibliothek, Auth und Threat Model offen |
| Persona | versionierte deklarative Dateien als Vorschlag | Format, Ablage, Signaturen, Freigabe-/Migrationsprozess offen |
| Mobile/Tauri | separate Clients, P2P-Adapter offen | Umfang und Reihenfolge erst nach nativem Sicherheitskern |

Technik nicht allein durch diesen Plan festschreiben: Architecture Decision Records (ADRs) mit Alternativen, Aufwand, Lizenz-/Versionsprüfung, Sicherheitsgrenzen und Abbruchkriterien anlegen. Keine eigene Kryptografie. Hub und Relay sind unterschiedliche Dienste; der Hub ist aktuell kein Mehrnutzer-Seminarserver.

### Agent2Agent ist nicht MCP

**A2A/Agent2Agent** bezeichnet Zusammenarbeit eigenständiger Agenten mit Identität, Fähigkeiten, delegierten Aufgaben, Status, Abbruch und Ergebnissen; bei Verwendung des benannten A2A-Protokolls muss dessen konkrete Version geprüft werden. Nicht jedes Subagent-Werkzeug implementiert dieses Protokoll.

**MCP (Model Context Protocol)** stellt Werkzeuge, Ressourcen und Prompts über eine standardisierte Client/Server-Schnittstelle bereit. Es ist kein automatischer Gruppenchat, kein Ersatz für Geräteidentität/MLS und kein Beleg für gleichberechtigte Agentenverhandlung. Ein Agent kann MCP-Werkzeuge benutzen und zusätzlich über A2A Aufgaben mit anderen Agenten austauschen. Der vorhandene pi-RPC/Hub-Transport implementiert für sich weder A2A noch ein P2P-Gruppenprotokoll. Native begrenzte Aufgabenvergabe darf zunächst genügen; Interoperabilität nur nach geprüftem Bedarf.

## 3. Rollen-Rechte-Matrix (Zielvertrag)

Rollen gelten pro Raum, nicht global. „Freigabe“ heißt expliziter, protokollierter Vorgang; tatsächliche Rechte sind server-/protokollseitig zu prüfen, nicht nur Buttons zu verstecken.

| Aktion | Seminarleitung | Teilnehmer | Gleichberechtigte Entwickler | Agent / Persona |
|---|---|---|---|---|
| Menschen-DM beginnen | Mit Kontaktzustimmung | Mit Kontaktzustimmung | Mit Kontaktzustimmung | Nein, nur als gekennzeichneter Agentenpartner |
| Agenten-DM / Task starten | Eigener Zugang, Budget | Eigener Zugang, Budget | Eigener Zugang, Budget | Nur mandatierter Peer/Task |
| Seminarraum anlegen | Verifizierte Kursrolle | Nein; Anfrage möglich | Nur außerhalb Seminar, Projektregeln | Nein |
| Projektgruppe anlegen | Wie normales Mitglied | Nach Raum-/Kursregeln | Alle gleichberechtigt | Nur nach menschlicher Freigabe |
| Mitglieder einladen | Kursrecht + Zustimmung | Nur delegiertes Einladungsrecht | Gemeinsame Regel / zeitliches Mandat | Keine autonome Einladung |
| Mitglied entfernen | Transparent, Kursraum | Austreten/blockieren | Mandat oder vereinbartes Quorum | Keine autonome Rechteänderung |
| Raumtyp humans/agents/mixed ändern | Mit sichtbarer Zustimmungsregel | Zustimmen/ablehnen/austreten | Gleiche Stimme, gemeinsamer Beschluss | Keine entscheidende Stimme |
| Gruppenverlauf lesen | Nur eigene Mitgliedsepochen | Nur eigene Mitgliedsepochen | Nur eigene Mitgliedsepochen | Nur freigegebener Kontext/Zeitraum |
| Fremde/private DMs lesen | Nein | Nein | Nein | Nein |
| Tools, Dateien, Provider nutzen | Eigene Rechte + Einzel-Freigaben | Eigene Rechte + Einzel-Freigaben | Eigene Rechte + Einzel-Freigaben | Kleinste explizite Capability |
| Persona/Agent hinzufügen | Sichtbare Raumfreigabe | Vorschlagen | Gemeinsame Raumfreigabe | Keine Selbstvervielfältigung |
| Moderieren/Schreibpause | Kursraum, nachvollziehbar | Eigene Sperre/Melden | Zeitliches Mandat/Quorum | Keine autonome Sanktion |
| Schlüssel widerrufen | Eigene; fremde nach Kursverfahren | Eigene | Eigene; Gruppenverfahren | Nur Besitzer/autorisiertes Verfahren |
| Export/Löschen | Eigene Daten; fremde Rechte achten | Ebenso | Ebenso | Kein Export ohne Freigabe |

Leitungsrolle kommt aus verifiziertem Rollenmapping, nie aus Selbstvergabe im Client. OIDC-Logout ist nicht automatisch MLS-Ausschluss. Rollenwechsel und Mitgliedschaft müssen verschlüsselte Gruppenepochen sowie lokale Berechtigungen konsistent verändern. Ein ausgeschlossener Peer kann schon erhaltene Klartexte nicht nachträglich vergessen; „für alle löschen“ ist keine garantierte Fernlöschung.

## 4. Chat- und Kontaktverwaltung

Gemeinsame Liste mit klaren Typ-/Host-/Identitätsmarkierungen, Suche, Filter, ungelesen, angeheftet/Bookmark, Archivieren und explizitem Ziel vor Senden. Unterstützen: erstellen, benennen, verlassen, blockieren, melden, lokal löschen, Export mit Warnung und Bestätigung. Gruppen zusätzlich: Mitglieder/Rollen, Einladungen widerrufen, Raumregeln, Agentenzugang, Aufbewahrung und Moderationsprotokoll.

Private Agentensession, P2P-DM und Gruppenraum besitzen getrennte stabile IDs; ein Anzeigename ist keine Identität. Lokale Sessionpfade und Hubinstanz-IDs dürfen nicht als globale Personen-ID dienen. Keine stillschweigende Übernahme privater Hauptchat-Historie in Gruppen oder A2A-Tasks. Beim Teilen Inhalt, Empfänger, Agent-/Providerbeteiligung, erwartete Kosten und Dauer getrennt anzeigen.

## 5. Phasen und Akzeptanztests

Jede Phase endet mit dokumentierten, reproduzierbaren Tests und separat ausgewiesener Live-Abnahme. R2 entwickelt; ARM überwacht, prüft Diffs und baut aus freigegebenen Quellen das ARM-Ergebnis. Kein R2-Quellcodeüberschreiben durch pauschales rsync. Phasen sind Qualitätsgates, kein Zeitversprechen.

### Phase 0 · Bestand, Architekturvertrag und sichere Übergabe

Inventar/Hashvergleich, Versionsprüfung, Skills auf ausführendem Host, Testmatrix und ADRs. Veraltete Statusangaben konsolidieren; keine Builds nötig, um Dokumentationslücken zu schließen.

**Akzeptanz:** HEAD/Branch/Änderungen pro Host nachvollziehbar; Secrets ausgeschlossen; Skill-Dry-Run ohne Schreibzugriff; kein Modellprompt; geplanter Datenfluss zeigt Menschen/Agenten/Hub/Relay/Provider und Einwilligungsgrenzen. Neue Schlüssel-/Persona-/A2A-Technik bleibt ausdrücklich offen.

### Phase 1 · Remote-Hauptchat zuverlässig und konsistent

Einzel-Remote und Raster klar trennen; Workspace-/Modell-/Status-/Abbruchrouting; Verlauf virtualisieren; Capabilityanzeige und ehrliche Fehlerzustände. Persistenzvertrag: Sessioninhalt, Hubmetadaten, UI-Auswahl und offene Dialoge getrennt behandeln.

**Akzeptanz:** Mit Mock gegen eigenen Testhub: Leiste ausblenden, Backend wechseln und Instanz beenden – niemals versehentlich lokales Senden/Abbrechen. SSE unterbrechen, Duplikate und Pufferüberlauf erzwingen; Lücken anzeigen/nachladen. Hubneustart mit Registry: Session neu öffnen, verlorene Laufzeit nicht als fortgesetzten Job melden. Rückfragen weder doppelt noch automatisch beantworten. Echter Modelltest erst mit festem Kostenrahmen und Nutzerfreigabe; echtes ARM-Fenster separat abnehmen.

### Phase 2 · Menschliche DMs und belastbarer P2P-Kern

Worker entkoppeln, Gesamt-/Peer-/Karten-/Queue-Limits, Timeouts und Abbruch; langlebige Geräteidentität im OS-Schlüsselbund, Schlüsselprüfung, Blockieren, Kontaktverwaltung. Zunächst kleine Testdaten, keine vertraulichen Gruppen.

**Akzeptanz:** Zwei physische Geräte tauschen DMs bidirektional aus; Neustart erhält geprüfte Identität oder meldet Schlüsselwechsel deutlich. Langsamer/feindlicher Peer blockiert UI und andere Peers nicht; Flood/überlange/fehlerhafte Daten bleiben innerhalb messbarer Limits. Empfang löst weder Modellaufruf noch Shell/Dateiöffnung aus. Identitätsverlust/Restore/Blockierung geprüft. Nachrichtenzustände unterscheiden lokal angenommen, übertragen, zugestellt und gelesen; Lesebestätigung opt-in.

### Phase 3 · Identität, sichere Einladungen, NAT und Gruppen-E2EE

OIDC-PKCE und challengegebundene Gerätebindung evaluieren; Relay/NAT-Konzept und Gruppenprotokoll auswählen. Kurzlebige Einmal-Einladungen mit minimalen Rechten, Widerruf und sichtbarer Raum-/Identitätsprüfung. Gruppen humans/agents/mixed kryptografisch getrennt berechtigen.

**Akzeptanz:** Falscher Issuer/audience, abgelaufener Ausweis, Rollenfälschung, Einladungs-Replay und zweiter Gebrauch abgewiesen. Join/Leave/Kick, parallele Gruppenänderung, Epochenwechsel, Replay/Out-of-order und verlorene Schlüssel getestet; ausgeschlossene Mitglieder entschlüsseln keine künftigen Nachrichten. Neue Mitglieder erhalten alte Historie nur nach bewusster Regel/Freigabe. LAN, zwei NATs, Campus/Heimnetz, Relay-Ausfall/-Fallback praktisch testen; Relay-only per Netzbeobachtung ohne direkte IP-Publikation belegen. Relay darf Inhalte nicht entschlüsseln; Metadaten/IP-Sichtbarkeit ehrlich erklären. Unabhängiges Protokollreview vor Freigabe.

### Phase 4 · Räume, Rollen, Verwaltung und Offline-Verhalten

Seminarleitung/Teilnehmer serverseitig prüfen; Entwicklergruppen ohne permanente Hierarchie. Chat-Verwaltung, synchronisierte Projekt-/Tutorialereignisse, Handheben, Moderation, Gäste und Löschfristen. Offline zunächst explizit „nicht zugestellt“, optional später begrenztes verschlüsseltes Postfach.

**Akzeptanz:** Matrix-Aktionen positiv/negativ testen, insbesondere private DMs gegen Leitung schützen, Selbstvergabe verhindern und Entwicklergleichberechtigung nachweisen. Austritt/Widerruf entzieht künftigen Zugriff. Offline-Client, volle Queue, Ablauf, wiederholtes Senden, Konflikte und Replay erzeugen keine stillen Verluste oder doppelten Aktionen. Kein Versand nach abgelaufener Einladung oder an ausgeschlossenen Empfänger. Postfach speichert nur Chiffrate mit Quoten/TTL; Offline-Ausweise haben bewusst begrenzte Gültigkeit. Export/Löschen/Wiederanlauf und lokale Verschlüsselung praktisch prüfen.

### Phase 5 · Begrenzte Agent2Agent-Entwicklung

Zunächst beauftragte Entwicklungsagenten: task_id, Auftraggeber, Parent/Trace-ID, Capability-/Workspacegrenze, Budget, Deadline, erwartetes Artefakt, Status und Abbruch. Adapter zu konkretem A2A-Protokoll nur nach ADR; MCP-Werkzeuge bleiben separate Capabilityebene. Menschen bleiben Besitzer und Freigabeinstanz.

**Akzeptanz:** Zwei Agenten (zunächst Mocks) delegieren eine abgegrenzte Aufgabe, liefern prüfbaren Patch/Artefakthash zurück, lehnen unbekannte Fähigkeiten ab, brechen bei Timeout/Budget/Not-Aus ab. Duplikate werden idempotent, zyklische Delegation und Wiederholungen erkannt. Kein automatischer Commit/Push/Deploy, kein Schreibzugriff auf fremdes Workspace, kein Session-Sharing. Prompt-Injection aus Task/Code/Toolausgabe kann weder Rechte noch Budget oder Freigabestatus ändern. Kein echter Providerlauf ohne genehmigten Kostenrahmen.

### Phase 6 · Persona-Nebenagenten und gemischte Zusammenarbeit

Deklarative Persona-Verwaltung: erstellen/importieren, ansehen, bearbeiten, versionieren, testen, aktivieren/pausieren, zurücksetzen/löschen. Agenten sichtbar als künstliche Rollen kennzeichnen; kein Menschenimitat ohne Kennzeichnung und keine eigenmächtige Einladung.

**Vorgeschlagene Dateifelder:** stabile Persona-ID, Anzeigename, Beschreibung/Persona-Text, Herkunft, Versionshash, Besitzer, Skills/Capabilities-Allowlist, Modellpräferenz ohne Geheimnisse, erlaubte Räume/Daten, Speicher-/Löschregeln und Budgetobergrenzen. Secrets gehören in getrennte lokale Zugangsspeicher, nicht in Persona-Dateien. Persona-Text kann technische Limits nicht erhöhen. Änderungen erzeugen neue Revision und brauchen vor weiterer Ausführung Sichtung; alte Läufe referenzieren ihren Hash.

**Akzeptanz:** Manipulierte/importierte Persona bleibt in Quarantäne bis Sichtung; keine Shell-/Extensioninstallation durch Import. Drei Raumtypen und DMs mit Agenten werden korrekt zugelassen/abgewiesen. Kontext nur nach Zustimmung; private DMs bleiben privat. Kosten pro Persona/Task/Raum/Person sichtbar, Pause und Widerruf wirken auf laufende und wartende Jobs. Menschen können Antworten korrigieren/stoppen und Agentenbeiträge filtern; keine selbstlaufenden Unterhaltungsschleifen.

### Phase 7 · Pilot und weitere Frontends

Kleiner freiwilliger Seminar-/Entwicklungspilot nach Security-/Datenschutzreview, dann Tauri-P2P und mobile Clients separat. Betreiber, Updates, Restore und Notfallkontakt benennen.

**Akzeptanz:** Dokumentierte Schlüsselverlust-/Widerrufsübung, Datenexport/-löschung, Löschfristen, barrierefreie Bedienung, OS-Schlüsselbund und NAT-Matrix; eigenes ARM-/x86-/WebKit-/Mobilprotokoll. Native Tests beweisen keine WebKit-/Handyparität. Freiwillige Alternative ohne KI-Upload verfügbar; keine produktive Freigabe bei offenen kritischen Befunden.

## 6. Querschnitt: Kosten, Schleifen und Mensch-Freigabe

Vor jeder Agentenaufgabe: Provider/Ziel, betroffene Daten und maximalen Rahmen zeigen. Harte Limits im ausführenden Dienst, nicht nur im Prompt: Betrag, Token-/Turnzahl, Walltime, parallele Jobs, Retryzahl, Delegationstiefe, Fan-out und Nachrichtenrate. Unbekannte Preise als unbekannt ausweisen und konservativ begrenzen; Kosten auch für Hilfsmodelle, Retry/Compaction und Nebenagenten zählen. Clientdisconnection hebt Limits nicht auf. Erneute Freigabe beim Erhöhen; vorhandene Limits niemals aus Peerinhalt erhöhen.

Trace-/Parent-IDs, deduplizierte Task-IDs, TTL/Hoplimit, Cooldown und Zyklenerkennung; ein Budget zählt über Delegationsgrenzen. Keine endlose Agent↔Agent-Reaktion auf jede Nachricht. Standard: Agenten reagieren nur auf adressierte, autorisierte Aufgaben. Not-Aus stoppt Queue, aktive Prozesse soweit sicher möglich und neue Delegationen; bereits externe Aktionen sind nicht rückwirkend ungeschehen.

Netznachrichten, Dateien, Skills und Persona-Text sind untrusted Input. Herkunft/Vertrauensniveau sichtbar; keine automatische Ausführung oder Aufwertung zu Systemanweisungen. Kleine Capabilitysets, getrennte Workspace-/OS-/Containergrenzen, eingeschränkter Netzzugriff und kurzlebige Credentials. Ein Modellprompt allein ist keine Sandbox; Hub-Pfadsandbox beschränkt pi-Shell nicht.

Mensch-Freigabe für Datenweitergabe an Provider/Peers, Toolrechteausweitung, Schreiben außerhalb des Aufgabenbereichs, Personaaktivierung, Kostenanhebung, Einladungen und Veröffentlichung. Freigaben an Inhaltshash, Ziel, Identität, Betrag und Ablauf binden; kein globales „immer erlauben“. Ausstehende Dialoge bei Reconnect nicht automatisch bestätigen. Audit nur minimal und ohne Geheimnisse; Endnutzer müssen Auswirkungen und Grenzen verstehen.

## 7. Noch nicht entschiedene Betriebsfragen

Wer betreibt Identitätsdienst, Relay, möglichen Offline-Speicher und Reviews? Welche Kosten trägt jede Person? Welche Geräte-/Mehrgeräte-/Recoverystrategie gilt? Wie wird ein gleichberechtigtes Quorum konfliktfest umgesetzt? Welche Ausweisgültigkeit ist offline vertretbar? Welche Kryptobibliothek und welches A2A-Profil passen nach Versionsprüfung? Welche Texte/Artefakte dürfen in gemischten Gruppen zu welchem Provider? Diese Entscheidungen benötigen Betreiber-/Nutzerfreigabe und teilweise Datenschutzberatung, nicht bloß Code.
