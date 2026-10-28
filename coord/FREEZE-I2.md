# I2 · Eingefrorene Absprachen (K, 28.10.2026, nach Nutzerauftrag „Pool, Check-in/out, authentik, Sync, DSGVO“)

Neue feste Entscheidungen (ergänzen E1–E6, ersetzen die personenfeste Slot-Zuordnung aus A4):

| # | Entscheidung |
|---|---|
| **E7** | **Pool statt fester Konten.** `account01…account10` sind **flüchtige, universelle Arbeitskonten**. Niemand besitzt ein Konto. Jeder verifizierte **Check-in** bekommt ein freies Konto; sind alle belegt: **Warteliste** mit sichtbarem Platz („Alle 10 Arbeitsplätze sind belegt. Du bist Nr. 2 in der Warteschlange.“). |
| **E8** | **authentik ist die Identität, P2P nutzt sie.** Check-in per authentik (OIDC, Authorization Code + PKCE S256, System-Browser, Loopback-Redirect, öffentlicher Client). Gäste ohne authentik nur mit Einladungs-QR der Seminarleitung. Der Empfang stellt nach Check-in einen **Geräteausweis** aus (Ed25519, 12 h), der Person, Rolle und P2P-Knoten-ID bindet. **Rollen im P2P-Netz kommen nur aus dem Ausweis**, nie aus Client-JSON. |
| **E9** | **No data retention (DSGVO).** Beim **Check-out** werden alle flüchtigen Daten gelöscht und das Konto leer an den Pool zurückgegeben. Persönliche Daten gehen als Archiv auf den Client. Auf R2 bleibt nichts Personenbezogenes, außer (a) während der Belegung und (b) dem **Abholfach** bei erzwungenem Check-out: Archiv mit dem Geräteschlüssel verschlüsselt, höchstens 72 h, dann gelöscht. Logs ohne Namen und Inhalte (Pseudonym-Hash), Rotation 7 Tage. |
| **E10** | **Lokaler Arbeitsordner `~/cocopi/`** mit versteckten `~/cocopi/.sync/`. Bei jedem Check-in fragt cocopi mit Auswahl, was nach R2 geht. Beim Check-out kommt das Persönliche zurück. Git (GitHub/selbst gehostetes GitLab) ist **optionaler** Zusatz für Software-Projekte (I4), nicht Voraussetzung. |

## A6 Empfang und Belegung (Besitzer S3; Hub-Seite S3)

- Dienst **`cocopi-empfang`** (Node, Repo pi-hub, `empfang/`), Systemnutzer `cocopi-empfang`, `127.0.0.1:8800`, Zustand `/var/lib/cocopi-empfang` (0700). Von außen nur über TLS/Tailscale-Proxy; Proxy-Pfade: `/v1/empfang/*` → 8800, `/slot/accountNN/*` → `127.0.0.1:88NN`.
- Endpunkte (alle `protokoll:1`, Fehler `{code,was,tun}` wie v1):
  - `POST /v1/empfang/hallo` → `empfang_willkommen{slots_gesamt,slots_frei,warteschlange,oidc:{issuer,client_id,scopes},empfang_pubkey,vertrag:"cocopi-pool-v1"}`
  - `POST /v1/empfang/checkin` `{nachweis:{art:"oidc",id_token,nonce}|{art:"einladung",code}, geraet:{pubkey,name,art:"desktop"}, net_pubkey?}` → **201** `eingecheckt{checkin_id,slot,hub_pfad:"/slot/accountNN",person,rolle,belegung,geraeteausweis,gueltig_bis}` oder **202** `warteschlange{warte_id,platz,geschaetzt_min?}`
  - `GET /v1/empfang/warten/:warte_id` → `warteschlange{…}` oder `eingecheckt{…}`; Eintrag verfällt nach 90 s ohne Abfrage. Reihenfolge FIFO, Seminarleitung hat **keinen** Vorrang (kein Konto wird je jemandem weggenommen).
  - `POST /v1/empfang/checkout` (Gerät, Challenge wie v1 mit Kontext `cocopi-v1-empfang`) `{checkin_id}` → `checkout_bereit{checkout_id}`; nach Archivabruf `POST /v1/empfang/checkout/:id/erhalten{sha256}` → `ausgecheckt{}` (danach Leeren + Freigabe).
  - `GET /v1/empfang/status` (nur Rolle seminarleitung) → Belegung je Slot ohne Inhalte: `{slot,person,seit,zustand}`.
- **OIDC-Prüfung** im Empfang: iss, aud=client_id, exp, iat, nonce, Signatur per JWKS (RS256/ES256), JWKS-Cache 10 min. Rolle aus Claim `groups`: `cocopi-seminarleitung` → `seminarleitung`, `cocopi-teilnehmende` → `teilnehmende`, sonst Abweisung `keine_berechtigung`. Einladung → `gast`. Gespeichert wird nur `person` (Anzeigename) und `pid` = HMAC-SHA256(empfang_geheimnis, iss|sub), solange belegt.
- **Belegung** = vom Empfang signiertes JSON (Ed25519, kanonisch: Schlüssel sortiert, kein Leerraum) `{v:1,typ:"belegung",slot,checkin_id,person,rolle,geraete:[{geraet_id,pubkey,name,art}],gueltig_bis}`. Hub: Env `HUB_EMPFANG_PUBKEY`; neuer Endpunkt `POST /v1/belegung{belegung}` registriert genau diese Geräte (Quelle `belegung`). Login danach unverändert per Challenge. `POST /v1/freigabe{freigabe}` (signiert `{v:1,typ:"freigabe",slot,checkin_id}`) widerruft alle Geräte, stoppt alle pi, antwortet erst, wenn alle Prozesse beendet sind.
- **Geräteausweis** (Ed25519 vom Empfang, kanonisch) `{v:1,typ:"geraeteausweis",pid,person,rolle,net_pubkey,geraet_pubkey,iat,exp}` (12 h). pi-net prüft ihn gegen den beim ersten Check-in angezeigten und bestätigten `empfang_pubkey` (Fingerabdruck sichtbar in Einstellungen › Konto).
- Handy: koppelt wie in I1 per k-QR **am Slot-Hub** des eingecheckten Rechners (gilt nur bis Check-out). Eigenständiger Handy-Check-in erst I4.
- **Zwangs-Check-out**: Leerlauf ohne verbundenes Gerät > 8 h, oder Tagesende `EMPFANG_TAGESENDE=23:30`, oder Seminarleitung. Ablauf: Archiv bauen → mit X25519 (Gerät erzeugt beim Check-in `archiv_pubkey`, im Checkin-Body) verschlüsseln → Abholfach `/var/lib/cocopi-empfang/abholfach/<checkin_id>` ≤ 72 h → Konto leeren. Beim nächsten Check-in mit demselben `pid` bietet cocopi das Abholen an.

## A7 Sync und Datenklassen (Besitzer S7; Client-Crate `copi/crates/pi-sync`, Hub `lib/sync-v1.mjs`)

- **Client-Ordner** `~/cocopi/` (Windows `%USERPROFILE%\cocopi`): sichtbar `Projekte/`, `Ergebnisse/`, `Skills/`, `Verläufe/`; versteckt `.sync/` mit `zustand.json` (letzter Check-in, Auswahl, Manifest), `archiv/<JJJJ-MM-TT_HHMM>_<slot>.tar.gz` (Rückläufer), `konflikte/`, `protokoll.jsonl` (nur lokal).
- **Slot-Seite**: `~/arbeit/{Projekte,Ergebnisse}`, Skills `~/.pi/agent/skills/`, Verläufe `~/.pi/agent/sessions/`. `HUB_ROOTS=~/arbeit`.
- **Klassen** in `shared/sync/klassen.json` (versioniert, eine Quelle für Rust und Node): Klasse `persoenlich` = Ergebnisse, Quellen, Skills, Verläufe, alle `.md`, alles unter `Ergebnisse/`; Klasse `fluechtig` = `target/ node_modules/ .cache/ __pycache__/ .venv/ venv/ build/ dist/ .next/ .gradle/ coverage/ tmp/ temp/ .pytest_cache/ .mypy_cache/` sowie `*.o *.a *.so *.dylib *.dll *.exe *.class *.pyc *.log *.tmp *.swp *~ .DS_Store core core.*`, alles außerhalb der vier Bereiche. Regeln: erste passende Regel gewinnt; **unter `Ergebnisse/` ist alles persönlich**; Dateien > 200 MB → `gross` (Person entscheidet ausdrücklich); Symlinks, Geräte, FIFOs nie.
- **Check-in-Upload**: `POST <hub>/v1/sync/hoch` (Gerät) Body = tar.gz-Stream, Header `X-Cocopi-Bereich: Projekte|Ergebnisse|Skills|Verlaeufe`, `X-Cocopi-Name`; max. 2 GiB je Check-in. Server prüft jeden Eintrag (relativ, kein `..`, kein Absolutpfad, nur Datei/Ordner, Rechte 0644/0755), entpackt in Temp und verschiebt atomar. Antwort `sync_hoch_ok{bereich,name,dateien,bytes,manifest_sha256}`; Manifest (Pfad→sha256) bleibt im Slot bis Check-out.
- **Check-out**: `POST /v1/sync/vorschau` → `sync_vorschau{persoenlich:[{pfad,bytes,status:neu|geaendert|unveraendert}],fluechtig:{bytes,dateien,beispiele[≤20]},gross:[…]}`; `POST /v1/sync/archiv{auswahl:{zusaetzlich:[pfad…],ausschliessen:[pfad…]}}` → tar.gz-Stream + Header `X-Cocopi-Sha256` (Trailer-frei: zuerst `archiv_bereit{archiv_id,sha256,bytes}`, dann `GET /v1/sync/archiv/:id`). Unveränderte, hochgeladene Dateien kommen **nicht** zurück (Client hat sie).
- **Client-Rückspielen**: Archiv nach `.sync/archiv/` speichern, prüfen (sha256), dann in `~/cocopi/` entpacken. **Nie überschreiben**: lokal geänderte Datei → R2-Fassung als `Name (R2 TT.MM. HH-MM).ext` daneben, Eintrag in `.sync/konflikte/`. Erst nach erfolgreichem Entpacken `erhalten{sha256}` an den Empfang.
- Mobil holt keine Archive; es kann einzelne Ergebnisse ansehen/teilen (I1-Lesepfad).

## A8 Slot-Leeren und R2-Betrieb (Besitzer S4)

- Hubs laufen als **System-Units** `cocopi-hub@accountNN.service` (`User=accountNN`, Slice `cocopi.slice`/`cocopi-accountNN.slice`), nicht mehr als User-Units; Hub-Zustand in `~accountNN/.cocopi-hub/` (wird mit geleert).
- Root-Helfer **`/usr/local/sbin/cocopi-slotctl`** mit genau den Befehlen `leeren accountNN`, `status [accountNN]`, `pruefen accountNN`. Aufruf durch `cocopi-empfang` nur per sudoers-Zeile genau auf diese Befehle. `leeren`: Hub stoppen → alle Prozesse der UID beenden (`systemctl stop user@UID`, `pkill -U UID` nach Prüfung) → Home + `/mnt/data/accountNN` leeren (ohne Symlink-Folgen, `find -xdev -mindepth 1 -delete`) → Skelett aus `/etc/cocopi/skel/` → Rechte → Quotenprüfung (Belegung ≈ 0) → Hub starten → Log-Zeile `{slot,zeit,dauer_ms,bytes_geloescht}`.
- Alle Skripte idempotent, `--dry-run` Standard, `--ausfuehren` nur nach Freigabe D-R1.
- Einrichtung auf R2 zusätzlich als **pi-Dialogprompt** (`deploy/r2/EINRICHTEN-MIT-PI.md`): pi führt Schritt für Schritt, fragt vor jedem `sudo` nach, zeigt Dry-Run-Ausgabe zuerst.

## A9 Mitsteuern (Sessionübernahme durch Seminarleitung; Besitzer S8 Desktop-Netz, Hub-Feld S3)

- P2P über `pi-net` (v2: Kontakte, Gruppenchats, DMs, Remote View/Control) wird in den **v3-Desktop** zurückgeholt: Bereich **„Seminar“** (eine Stelle) mit Gruppenchat, Direktnachrichten, Teilnehmendenliste. Peers identifizieren sich mit Geräteausweis (A6); Rolle sichtbar als Abzeichen.
- Seminarleitung wählt in der Teilnehmendenliste Person → Session → **„Ansehen“** (≤ 120 min) oder **„Mitsteuern anfragen“** (≤ 30 min). Nur Rolle `seminarleitung` darf „Mitsteuern“ anfragen.
- Teilnehmende bekommt Dialog: „Aylin (Seminarleitung) möchte in ‚Seminar-Handout‘ mitschreiben.“ — **Erlauben 30 min** / **Nur ansehen** / **Ablehnen** (Ablehnen fokussiert).
- Aktiv: Teilnehmenden-Fenster mit farbigem Rahmen (`#4fb8d8`) und Leiste „Aylin steuert mit · noch 27 min · **Beenden**“; Seminarleitung sieht „Du steuerst Miras Session ‚Seminar-Handout‘ · **Zurückgeben**“. Prompts der Seminarleitung laufen über den Client der Teilnehmenden zum Hub (`senden{text, von:{person,rolle}}`, additiv) und erscheinen im Chat als „Aylin · Seminarleitung“ mit Farbmarke. Rückfragen beantwortet weiter die Teilnehmende, außer sie erlaubt es.
- Jederzeit beendbar von beiden Seiten; Ende bei Check-out, Verbindungsverlust > 60 s oder Ablauf.

## Nachtrag K nach I2-Runde 1 (28.10.2026, 22:40) — verbindlich für I2b

- **N1 Signaturformat:** flach `{...inhalt, signatur}` ist kanonisch (S7-Erzeuger). Die verschachtelte Variante bleibt nur lesend erlaubt.
- **N2 Idempotenz statt Recovery-Vertrag:** Kommt die *byte-gleiche* signierte Belegung erneut und ist für diese `checkin_id` aktiv, antwortet der Hub **200 `belegung_aktiv`** mit gleichem Inhalt. Eine erneute Freigabe einer bereits freigegebenen `checkin_id` ergibt **200 `freigegeben`**. Eine abweichende Nutzlast mit derselben `checkin_id` bleibt `verbraucht`. Der Empfang darf damit nach verlorenen Antworten gefahrlos wiederholen (höchstens 3-mal, Backoff 1/3/9 s).
- **N3 Slot-Epoche gegen Replay nach Home-Leeren:** `cocopi-slotctl leeren` erhöht `/var/lib/cocopi/slots/accountNN.epoche` (root, 0644, Ganzzahl). Der Empfang liest sie und schreibt `epoche` in Belegung und Freigabe. Der Hub verlangt `epoche == aktuelle Datei` (Env `HUB_EPOCHE_DATEI`). Fehlt die Datei, gilt Epoche 0, nur im Test.
- **N4 Zeiten und Schlüssel:** Ausweis-/Belegungszeiten in **Unix-Sekunden** (`iat`,`exp`,`gueltig_bis`). `net_pubkey` = iroh-Knoten-ID hex (64 Zeichen, klein).
- **N5 Proxy:** Von außen gesperrt sind `/v1/belegung`, `/v1/freigabe` und `/v1/sync/dienst/*`, auch mit Präfix `/slot/accountNN/…`.
- **N6 Abbruch während Check-in:** Kommt der Abbruch nach der Vergabe, ruft der Client `checkout` ohne Upload auf. Der Empfang gibt den Platz ohne Archiv frei, weil er leer ist.
