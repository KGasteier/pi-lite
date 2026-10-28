# Konzept: privates Netz in copi und pi-lite

Stand: 13.10.2026 · Entwurf, noch nicht umgesetzt · UI-Dummy: `collab-mockup.png` (Quelle `collab-mockup.html`, Beispieldaten fiktiv)

## Ziel

Nutzende von copi und pi-lite verbinden sich in einem geschlossenen Kreis. Dort können sie:

- neben dem Agentenchat miteinander chatten,
- Dateien, Codeblöcke und fertige Prompts austauschen,
- sehen, wer online ist, und Kontakt erlauben oder pausieren,
- ein Tutorial halten und gemeinsam an Projekten arbeiten.

Wer teilnehmen darf, steht auf einer Whitelist mit Namen und Gruppen. Diese wird in authentik (`auth.udk.digital`) verwaltet.

## Grundsatz: wo authentik endet

authentik verwaltet **Identitäten und Gruppen**: wer darf hinein und wer gehört zu welchem Kurs oder Projekt. authentik ist aber kein Nachrichtendienst. Präsenz, Chat und Dateien brauchen deshalb einen eigenen Transportweg.

### Empfehlung: Matrix mit authentik-Anmeldung

Ein eigener Matrix-Homeserver unter UdK-Domain (Synapse oder Continuwuity) meldet Nutzende per OIDC über authentik an. Gründe:

- **Verschlüsselung:** Ende-zu-Ende-Verschlüsselung ist erprobt (`matrix-sdk-crypto`), ebenso Präsenz, verschlüsselte Dateiübertragung, Räume und Spaces.
- **Ein Kern für beide Apps:** `matrix-rust-sdk` ist Rust. Ein gemeinsames Crate `pi-net` kann copi direkt und pi-lite über Tauri-Befehle nutzen. Die Parität ist so schon in der Architektur angelegt.
- **Eigene Ereignistypen** sind vorgesehen, etwa `digital.udk.copi.prompt`, `….code` und `….tutorial.step`.
- **Rückfallweg:** Teilnehmende können notfalls auch mit einem normalen Matrix-Client (Element) dabei sein.

### Alternative: eigener kleiner Relay-Dienst

Ein eigener WebSocket-Dienst, angemeldet per authentik-OIDC, mit MLS-Verschlüsselung:

- **Vorteil:** schlanker, volle Kontrolle.
- **Nachteil:** Kryptografie, Zustellung bei Offline-Zeiten, Dateispeicher und Mehrgeräte-Schlüssel müssen selbst gebaut und geprüft werden. Davon rate ich ab.

## Ablauf

1. **Anmeldung:** „Mit UdK anmelden“ öffnet den Browser (OIDC mit PKCE, Rücksprung auf localhost). Das Token liegt im System-Schlüsselbund, nicht in der Konfiguration.
2. **Gruppenabgleich:** authentik-Gruppen wie `copi-kurs-a` oder `copi-projekt-klang` werden über einen kleinen Abgleichdienst zu Matrix-Spaces/-Räumen. Wer aus einer Gruppe entfernt wird, verliert dort den Zugang.
3. **Rollen** kommen ebenfalls aus authentik, z. B. `leitet` für Lehrende: Tutorial steuern, Prompts an die ganze Gruppe senden.
4. **Direktkontakt** ist nur zwischen Personen möglich, die sich eine Gruppe teilen, oder nach ausdrücklicher Annahme einer Kontaktanfrage.

## Funktionen und Integration

Die Ziffern beziehen sich auf den UI-Dummy.

| Nr. | Element | Verhalten |
|---|---|---|
| 1 | Netz-Chip in der Kopfzeile | Status, aktive Gruppe, Anzahl online. Menü: erreichbar, pausieren (30 min, 2 h, bis morgen, bis Fortsetzen), unsichtbar/abmelden, Sicherheitsoptionen, „Gruppen verwalten …“ (öffnet die authentik-Selbstverwaltung) |
| 2 | Netz-Region rechts | Einklappbar per Tastenkürzel. Tabs je Gruppe, Projekt und Direktgespräch; Ungelesen-Zähler. Der Agentenchat bleibt die Hauptfläche |
| 3 | Präsenz | Online, abwesend, pausiert, offline, dazu die Rolle. Es erscheinen nur Whitelist-Mitglieder der Gruppe |
| 4 | Karten | Dateien, Codeblöcke und Prompts erscheinen als Karten, nicht als Fließtext |
| 5 | Senden | ＋ Datei, { } Codeblock, ✎ Prompt-Vorschlag. Im Agentenchat bekommt jeder Code- und Antwortblock die Aktion „↗ Ins Netz teilen“ |
| 6 | Prompt-Übergabe | Eingehende Prompts erscheinen als Karte über der Eingabe: ansehen, in die Eingabe übernehmen (bleibt editierbar) oder verwerfen |
| 7 | Tutorial-Leiste | Schritt x/y, „Folgen“ oder eigenes Tempo, Hand heben |
| 8 | Geteilte Sessions | Netz-Markierung und Personenzahl in der Session-Liste |

## Sicherheitsregeln

- **Prompts werden nie automatisch gesendet oder ausgeführt.** Beim Übernehmen:
  - Absender und Herkunft bleiben sichtbar.
  - Unsichtbare und Bidi-Steuerzeichen werden entfernt.
  - Es gibt eine Längenbegrenzung.
  - Ist die Eingabe nicht leer, wird erst gefragt, ob ersetzt oder angehängt werden soll.
- **Dateien:**
  - werden nie automatisch geöffnet, ausgeführt oder in den Workspace gelegt;
  - werden erst nach Wahl des Zielordners gespeichert, ohne Ausführungsrecht;
  - zeigen Größenlimit und Prüfsumme an.
  - „Als Material“ verwendet den bestehenden Material-Weg mit Einwilligung.
- **Codeblöcke** werden nur als Text behandelt: kopieren oder in die Eingabe übernehmen, nie direkt ausführen.
- **Sitzung zeigen (nur Lesen)** ist immer opt-in, zeitlich begrenzt und jederzeit beendbar. Der Agentenverlauf kann Pfade oder Geheimnisse enthalten, darauf wird vorher hingewiesen.
- **Keine fremden Shells.** Fernsteuerung des Agenten oder des Rechners anderer gehört nicht zum Funktionsumfang.
- **Pausieren** stoppt Zustellung und Benachrichtigungen. Präsenz wird als „pausiert“ gezeigt oder auf Wunsch ganz verborgen. Nachrichten von Lehrenden lassen sich optional durchlassen.
- **Keine automatische Weitergabe an KI.** Netznachrichten gehen nur an den Agenten, wenn die Person sie ausdrücklich übernimmt.

## Tutorial und Zusammenarbeit

**Tutorial**

- Lehrende legen Schritte als Markdown an und blenden sie schrittweise ein. Schritte können Prompt-Vorschläge und Dateien enthalten.
- Teilnehmende sehen die Leiste, können „Folgen“ oder im eigenen Tempo arbeiten und die Hand heben.
- Optional melden Teilnehmende ihren Fortschritt („Schritt 3 erledigt“). Das ist nur eine Statusmeldung, kein Bildschirmmitschnitt.

**Projekt**

- Ein gemeinsamer Raum mit Chat, angehefteten Prompts und Codeblöcken.
- Code wird weiterhin über Git ausgetauscht. Das Netz ergänzt, ersetzt aber kein Repository.

## Umsetzungsschritte

| Phase | Inhalt |
|---|---|
| 0 | Entscheidungen klären (siehe offene Fragen); Homeserver und authentik-OIDC-Provider testweise aufsetzen |
| 1 | Crate `pi-net`: Anmeldung, Präsenz, Textchat. UI: Chip (1), Region (2), Präsenz (3) – in copi und pi-lite |
| 2 | Karten für Codeblöcke, Prompts und Dateien, Prompt-Übergabe (6), „Ins Netz teilen“ |
| 3 | Gruppenabgleich authentik → Spaces, Rollen, Tutorial-Leiste (7) |
| 4 | Nur-Lesen-Sitzungsfreigabe und Projektmarkierungen (8) |

Jede Phase wird in beiden Apps umgesetzt und getestet, mit Mock-Homeserver für automatisierte Tests.

## Offene Fragen

1. Gibt es unter `udk.digital` schon einen Matrix-Server, oder darf einer betrieben werden?
2. Wer legt Gruppen an: nur Admins in authentik oder auch Lehrende per Selbstverwaltung?
3. Sollen externe Gäste teilnehmen können (eigene authentik-Einladung)?
4. Aufbewahrung: Wie lange bleiben Chat und Dateien gespeichert? Braucht es eine Moderation oder ein Archiv? Das kollidiert teilweise mit Ende-zu-Ende-Verschlüsselung.
5. Datenschutz: Ist eine Abstimmung mit dem UdK-Datenschutz nötig, etwa wegen Präsenzanzeige oder Fortschrittsmeldungen?
