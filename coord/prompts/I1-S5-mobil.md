# Dein Auftrag: S5 · Mobil — I1-S5-mobil (≤ 130 min)

Rolle S5 (§5, §8). Worktree `/home/medienhaus/Work/pi-lite-wt/s5`, Branch `v3/s5`.
Zuerst `git merge --ff-only cocopi-v3`. **Verbindlich:** `coord/FREEZE-I1.md`, `docs/v3/INVENTAR.md` §2 (Mobil-Orte
M-LI/M-SE/M-VO/M-EI/M-QR), `docs/v3/KONZEPT.md` A1/A3, `docs/v3/FLOWS.md`, Prompt §4.2.

Du besitzt `mobile/**` (neu) und `ui/transport.js` (neu). Nicht: übrige `ui/*` (Änderungsbitte), `copi/**`.
Referenz (nur lesen): pi-lite `ui/`, pi-hub-Client `/home/medienhaus/Work/pi-hub/client/` (mobicopi, schon mobil + SSE),
Protokoll v1 von S3 (entsteht parallel in `/home/medienhaus/Work/pi-lite-wt/s3/shared/protocol/v1/`; Heartbeat
`coord/heartbeat/s3` meldet „A1-v1 fertig“). Bis dahin gegen KONZEPT A1 bauen, danach angleichen.

I1-Ziel: **Mobil zeigt Sessions read-only** + Koppeln-Grundlage.
1. `ui/transport.js`: ES-Modul, eine Schnittstelle `createTransport({baseUrl, getToken})` mit `hallo()`, `sessions()`,
   `events(onMsg)` (SSE mit Last-Event-ID-Replay, Puls-Überwachung 45 s → Zustand „unterbrochen“), `einloesen(code)`,
   `koppeln(code)`, `abmelden()`. Token **nur** im Authorization-Header (für SSE: `fetch`-Stream, kein `EventSource ?token=`).
   Keine Abhängigkeit von Tauri. Parser für QR-Formen (Fragment `#1.k.<code>`, Query `?c=`, Rohcode mit Bindestrichen).
2. `mobile/`: `index.html`, `app.js`, `style.css`, `manifest.webmanifest`, `sw.js` (Service Worker cacht **nur** App-Hülle,
   keine Inhalte/API-Antworten), Icons aus vorhandenem cocopi-Icon (`packaging/` o. ä.) abgeleitet.
   Ansichten: M-LI (Sessions mit Zustand ruht/arbeitet/wartet auf dich/fertig/Fehler, oben Konto + Verbindung),
   M-SE read-only (Verlauf), M-EI (Dieses Gerät, Abmelden), M-QR (Code eintippen; Kamera-Scan via `BarcodeDetector`
   wenn vorhanden, sonst nur Eintippen — keine fremde Bibliothek in I1). Texte wörtlich aus KONZEPT A3 §2.
   Gestaltungsregeln §3: eine Hauptaktion je Ansicht, große Ziele (≥ 44 px), ruhig. Keine Funktionen außerhalb §4.2.
3. Gerätschlüssel im Browser: WebCrypto Ed25519 (wenn verfügbar) mit nicht-exportierbarem Schlüssel in IndexedDB;
   Fallback dokumentieren. Tauri-2-Mobile-Hülle in I1 nur als **Plan** (`mobile/TAURI.md`: Berechtigungen Kamera/Benachrichtigung,
   sichere Ablage Keychain/Keystore, Fragment-vs-Query beim Kamera-Intent bewerten — Antwort an S1/S3).
4. Tests (Node 22, ohne Browser; R2 hat kein Chromium): `mobile/test/*.mjs` — Transport gegen einen kleinen Fake-HTTP/SSE-Server
   (Replay ohne Dubletten/Lücken, Puls-Timeout, 401 → „abgemeldet“), QR-Parser, Zustandsabbildung → Anzeige-Text,
   Service Worker cacht keine `/v1/`-Antworten (statische Prüfung). `node --check` aller JS. Optional: wenn
   `/home/medienhaus/Work/pi-hub-wt/s3` schon einen v1-Hub hat, ein Integrationstest dagegen (überspringen, wenn nicht).
5. `mobile/README.md`: Start lokal (statisch ausgeliefert vom Hub unter `/m/` — als **Änderungsbitte an S3** formulieren), Grenzen.

Keine Installation von Paketen (kein npm install). Bericht nach `coord/runs/I1-S5-mobil-bericht.md`.
