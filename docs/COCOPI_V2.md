# cocopi v2 – Architektur und Plan

Stand: Branch `cocopi-v2` (Basis `cocopi` b65a1e7: copi inkl. Remote-Hauptchat + P2P-Prototyp).
Sicherheitskonzept: [`COCOPI_SECURITY.md`](COCOPI_SECURITY.md).

## Ziel

Eine konsistente cocopi-Anwendung = copi (lokale/remote Agenten-Sessions) + Zusammenarbeit:

- Menschen über P2P (iroh-QUIC, direkt, dauerhafte Geräteidentität, bestätigte Kontakte)
- Rollen: **Seminarleitung**, **Seminarteilnehmende**, **Projektentwicklung** (hierarchiefrei)
- Chats: **Gruppe** (Menschen / Agenten / gemischt), **DM** (1:1), Verwaltung (anheften,
  stummschalten, archivieren, verlassen, lokal löschen, Mitglieder/Rollen)
- **Agent2Agent**: laufende Entwicklungs-Sessions und **Nebenagenten mit Persönlichkeit**
  (Personas) sprechen in Chats miteinander – mit Rundenbudget, Not-Aus, optionaler Freigabe
- **Fernzugriff**: Seminarleitung sieht (und ggf. steuert) nach Freigabe die Session einer
  teilnehmenden Person

## Schichten

| Schicht | Ort | Inhalt |
|---|---|---|
| Vertrag/Modell | `crates/pi-net/src/model.rs`, `wire.rs` | Typen, Umschläge, Grenzen |
| Logik (ohne I/O) | `store.rs`, `remote.rs`, `a2a.rs`, `persona.rs`, `identity.rs` | Chat-/Rollenverwaltung, Eingangsprüfung, Fernzugriff-Zustandsmaschine, Dirigent, Persona-Dateien |
| Transport | `node.rs` | iroh-Endpoint, Allowlist, Warteschlangen, Ratenbegrenzung |
| App-Laufzeit | `app/src/coco.rs` | verbindet alles mit `App` (`coco_pump`, `coco_apply`, `coco_submit`) |
| Oberfläche | `app/src/coco_view.rs` | rechte Leiste, Chat im Hauptbereich, Dialoge, Demo-Szenen |

`pi-net` bleibt GUI-frei und serde-basiert (ein späterer Tauri-Adapter kann denselben Kern nutzen).
`direct.rs` ist der alte Prototyp und wird nach v2 entfernt.

## Teilnehmende

```text
Participant::Human    { peer }                         Mensch = Geräteschlüssel
Participant::DevAgent { owner, backend, instance }     laufende pi-Session (owner-Gerät)
Participant::Persona  { owner, persona }               Nebenagent (owner-Gerät)
```

`DevAgent { backend: "@main", instance: "" }` = Haupt-Session von cocopi.
**Agenten werden nur von ihrem Besitzer-Gerät angesprochen.** Andere Geräte sehen nur deren
Nachrichten im Chat.

## Rollen

| Raum | Wer | Rechte |
|---|---|---|
| Seminar | Leitung (= Ersteller, Schlüssel `founder`) | Mitglieder verwalten, Schritt setzen, Fernzugriff **anfragen** |
| Seminar | Teilnehmende | schreiben, eigene Agenten einbringen, Fernzugriff **freigeben/beenden** |
| Projekt | Projektentwicklung (alle gleich) | schreiben, Mitglieder einladen, nur **eigene** Agenten entfernen |
| beide | Agent | schreibt nur im Auftrag des Besitzers |

Rolle „Leitung“ ist kryptografisch an das Gerät gebunden, **nicht** institutionell geprüft
(authentik/OIDC später, siehe `docs/concepts/NETZ.md`). Die UI sagt das deutlich.

## Hauptbereich und Eingabe

Der Hauptbereich zeigt genau eines (Priorität): cocopi-Chat bzw. Fernsitzung (`coco.main`) →
gewählte Instanz (`remote_single`, lokal oder remote) → Raster → normale Session. Die **normale
Eingabe** schreibt immer in das, was der Hauptbereich zeigt; Platzhalter und Signalfarbe sagen wohin.

## Rechte Seitenleiste

- Kopf: Identität, rechts der Knopf **„Backends n“**. Er schaltet zwischen den Reitern
  (Chats · Kontakte · Räume · Agenten) und der Backend-Ansicht um.
- Backend-Ansicht: Dropdown **„Einzelchat“** mit Haupt-Session und allen laufenden Instanzen
  (Magenta = dieses Gerät, Cyan = Server; die Signalfarbe folgt nur echten Server-Instanzen).
  Daneben **„▦ Raster n“**: zeigt die markierten Instanzen nebeneinander. Instanzkarten sind
  vollflächig klickbar (Klick = Einzelchat, ▦ = ins Raster, Strg/Cmd+Klick = ins Raster,
  Rechtsklick = Stoppen/Neu starten/Abruf).
- Reiter Agenten: Nebenagenten mit „+ Neu …“, „↻ Neu laden“ (Ordner neu einlesen), je Karte
  „Chat“, bei mitgelieferten „Als Kopie bearbeiten …“ (neue Kennung `<id>-kopie`) und
  „Anpassen …“ (gleiche Kennung, überschreibt), bei eigenen „Bearbeiten …“ und „Löschen“
  bzw. „Original wiederherstellen“. Löschen entfernt nur die reguläre Datei `<id>.md`
  (kein Symlink) und beendet eine laufende Instanz, sofern sie gerade keine Runde spricht.

## Ablauf Agent2Agent (Gerät mit Agenten)

1. Neue Nachricht im Chat (A2A an) → `Conductor::next(chat, me, busy)`.
2. `Speak{agent,prompt}` → App schickt `prompt` (Verlauf als **zitierte Daten**) an die
   Instanz des Agenten; Persona-Instanzen werden bei Bedarf gestartet (`--no-tools`).
3. `agent_end` → letzte Assistentenantwort wird als Nachricht des Agenten gepostet (und an
   entfernte Mitglieder verteilt) → zurück zu 1.
4. Budget erschöpft / Not-Aus → pausiert; ein Mensch setzt fort.

## Fernzugriff (Leitung ↔ Teilnehmende)

`RemoteRequest` → Dialog bei Teilnehmenden (Umfang/Dauer wählbar, Session wählen) →
`RemoteGrant` → Teilnehmende spiegeln ihre Session als `RemoteFrame`s → Leitung sieht sie im
Hauptbereich. Eingaben der Leitung: `View` keine, `Suggest` als Vorschlagskarte, `Control`
direkt an den Agenten (max. 30 min, Banner, Not-Aus). Ablauf/Widerruf lokal durchgesetzt.

## Persistenz

`<config>/copi/cocopi/identity.key` (0600), `chats.json` (0600, atomar), `personas/*.md`.
Fernzugriff-Freigaben werden **nicht** gespeichert (enden mit dem Prozess).

## Arbeitsteilung (Subagents, Worktrees unter `.agents-wt/`)

| Agent | Dateien | Aufgabe |
|---|---|---|
| A Kern | `store.rs`, `identity.rs`, `model.rs` (nur Tests/Helfer) | Chat-/Rollenverwaltung, Eingangsprüfung, Persistenz, Sicherheitswörter |
| B Netz | `node.rs`, `remote.rs` | iroh-Transport v1, Fernzugriff-Zustandsmaschine |
| C Agenten | `a2a.rs`, `persona.rs` | Dirigent, Rahmung, Persona-Dateien + Beispiele |
| D Laufzeit | `app/src/coco.rs` | Verdrahtung in die App, Persona-Instanzen, A2A-Ausführung, Spiegeln |
| E Oberfläche | `app/src/coco_view.rs` | Leiste, Chat-Ansicht, Dialoge, Demo-Szenen, Screenshots |

Zusammenführen: A → C → B → D → E, nach jedem Merge Tests.

## Fehlaufrufe durch Desktop-Zuordnung

Ist cocopi/copi irrtümlich als Terminal (`xdg-terminals.list`, `Categories=TerminalEmulator`),
Browser oder Standard-App (`mimeapps.list`, `MimeType=`) eingetragen, starten Omarchy-Aktionen
wie Super+A (Web-App) oder der Bildschirmschoner jedes Mal ein neues Fenster. Seit diesem Stand
beendet sich cocopi bei fremden Optionen (`-e …`, `--app=…`, `--class=…`) sofort mit Code 2,
ohne Fenster. `copi/scripts/check-desktop.sh` zeigt die Ursache, `--apply` entfernt sie
(Sicherung `*.bak-copi`; Hyprland-Konfiguration wird nur angezeigt, nie geändert).
