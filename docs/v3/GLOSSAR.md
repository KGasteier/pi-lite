# cocopi v3 · Glossar (S1, Fassung 2 · I2)

**Regel: ein Begriff je Sache, in allen Texten gleich** (Oberfläche, Tutorial, KONZEPT, FLOWS, ACCEPTANCE, Fehlertexte).
- Spalte **Nicht verwenden** zeigt die Wörter, die in Oberflächentexten nicht vorkommen sollen.
- Technische Bezeichner (z. B. `slot`, `checkin_id`) bleiben im Code und im Protokoll, wie sie sind.
- Englisch als zweite Sprache ist vorbereitet (Spalte **EN**).

## Kernbegriffe I2

| Begriff | Bedeutung (in einem Satz) | Nicht verwenden | Technisch | EN |
|---|---|---|---|---|
| **Arbeitsplatz** | Eines der 10 gleichen Konten auf R2, das man beim Check-in aus dem Pool bekommt und beim Check-out leer zurückgibt. | „Slot“, „Account“, „mein Konto“, „Rechner auf R2“ | Konto `account01…account10`, Feld `slot`, Hub `cocopi-hub@accountNN` | workspace |
| **Pool** | Alle 10 Arbeitsplätze zusammen; keiner gehört jemandem. | „Kontingent“ | E7, `pool.mjs` | pool |
| **Warteschlange** | Reihenfolge der Personen, die auf einen freien Arbeitsplatz warten; man rückt ohne Klick nach. | „Warteliste“, „Queue“ | `warteschlange{warte_id,platz}`, FIFO, 90 s | queue |
| **Check-in** | Anmelden am Empfang mit UdK-Konto oder Einladungscode; danach hat man einen Arbeitsplatz. | „Login“, „Anmelden“ (für den Pool), „Einloggen“ | `POST /v1/empfang/checkin`, `eingecheckt` | check in |
| **Check-out** | Arbeitsplatz zurückgeben: Persönliches kommt auf den Rechner, alles andere wird gelöscht, der Arbeitsplatz ist danach leer. | „Logout“, „Abmelden“, „Auschecken“ als Substantiv | `POST /v1/empfang/checkout`, `ausgecheckt` | check out |
| **erzwungener Check-out** | Check-out, den R2 selbst macht (8 h ohne Gerät, Tagesende 23:30, Seminarleitung); Persönliches landet im Abholfach. | „Zwangsabmeldung“, „Timeout“ | Zwangs-Check-out (A6) | forced check-out |
| **Abholfach** | Verschlüsselte Ablage auf R2 nach erzwungenem Check-out, höchstens 72 h; nur der eigene Rechner kann sie öffnen. | „Postfach“, „Backup“, „Sicherung“ | `/var/lib/cocopi-empfang/abholfach/<checkin_id>` | pickup box |
| **Empfang** | Der Dienst auf R2, der Check-in und Check-out abwickelt, Arbeitsplätze vergibt und Geräteausweise ausstellt. | „Server“, „Rezeption“, „Login-Dienst“ | `cocopi-empfang`, `127.0.0.1:8800` | reception |
| **Geräteausweis** | Vom Empfang unterschriebene Bestätigung (12 h): dieses Gerät gehört dieser Person mit dieser Rolle. | „Zertifikat“, „Token“, „Ticket“ | `geraeteausweis`, Ed25519, `pid`, `net_pubkey` | device pass |
| **Mitsteuern** | Die Seminarleitung schreibt mit Zustimmung der Person Aufträge in deren Session; Rahmen und Leiste zeigen das. | „Fernsteuerung“, „Remote Control“, „Übernehmen“, „Mitschreiben“ (nur im Dialogsatz aus A9) | A9, `RemoteScope::Control` | co-steer |
| **Ansehen** | Die Seminarleitung sieht eine Session nur an, ohne etwas zu senden (≤ 120 min). | „Remote View“, „Zuschauen“ | `RemoteScope::View` | view |
| **persönlich** | Gehört der Person und kommt beim Check-out zurück: Ergebnisse, Projekte/Quellen, Skills, Verläufe. | „privat“, „wichtig“, „Nutzerdaten“ | Klasse `persoenlich` (`shared/sync/klassen.json`) | personal |
| **flüchtig** | Entsteht beim Arbeiten neu und wird beim Check-out gelöscht: Builds, Cache, temporäre Dateien. | „temporär“ allein, „Müll“, „unwichtig“ | Klasse `fluechtig` | disposable |
| **groß** | Persönliche Datei über 200 MB; geht nur mit eigenem Häkchen mit. | „zu groß“ (außer im Fehlertext) | Klasse `gross` | large |
| **Sync-Auswahl** | Die Häkchenliste beim Check-in: was aus `~/cocopi` nach R2 mitgeht. | „Upload-Dialog“, „Synchronisation“ (es läuft kein Dauerabgleich) | `POST /v1/sync/hoch` | selection |
| **Vorschau** (Check-out) | Die zwei Spalten „Kommt zurück auf deinen Rechner“ / „Wird gelöscht“ vor dem Check-out. | „Zusammenfassung“ | `sync_vorschau` | preview |
| **Konflikt** | Die Datei wurde lokal und auf R2 geändert; die lokale bleibt, die R2-Fassung liegt daneben als `Name (R2 TT.MM. HH-MM).ext`. | „Überschreiben“, „Kollision“ | `.sync/konflikte/` | conflict |
| **`~/cocopi`** | Der eigene Ordner auf dem Rechner mit `Projekte/`, `Ergebnisse/`, `Skills/`, `Verläufe/`. | „Sync-Ordner“, „cocopi-Verzeichnis“ | `%USERPROFILE%\cocopi` unter Windows | — |
| **Seminar** | Der Bereich mit Seminarchat, Direktnachrichten und Teilnehmenden (ein Reiter). | „Netz“, „P2P“, „Räume“ | pi-net, `desktop/seminar.rs` | seminar |
| **Seminarchat** | Der Chat für alle im Seminar. | „Gruppenchat“, „Raum“ | — | seminar chat |
| **Direktnachricht** | Nachricht an genau eine Person im Seminar. | „DM“ (in der Oberfläche), „PN“ | — | direct message |
| **UdK-Konto** | Die Hochschul-Anmeldung, technisch über authentik. | „authentik“ (in der Oberfläche), „SSO“ | OIDC + PKCE S256 | UdK account |
| **Einladungscode** | Einmaliger Code (5 min) aus dem Einladungs-QR der Seminarleitung für den Gast-Check-in. | „Gästepasswort“, „Token“ | `nachweis:{art:"einladung",code}` | invitation code |

## Rollen

| Begriff | Bedeutung | Technisch |
|---|---|---|
| **Seminarleitung** | Leitet das Seminar, darf einladen, ansehen und mitsteuern anfragen; hat beim Pool **keinen** Vorrang. | `seminarleitung`, Gruppe `cocopi-seminarleitung` |
| **Teilnehmende** | Studierende mit UdK-Konto. | `teilnehmende`, Gruppe `cocopi-teilnehmende` |
| **Gast** | Ohne UdK-Konto, nur mit Einladungscode. | `gast` |

## Begriffe aus I1, die weiter gelten

| Begriff | Bedeutung |
|---|---|
| **R2** | Der Rechner der UdK, auf dem die Arbeitsplätze laufen. |
| **Gerät** | Ein Rechner oder Handy, das an einem Arbeitsplatz angemeldet ist (Schlüssel bleibt auf dem Gerät). |
| **Koppeln** | Handy per QR an den Arbeitsplatz des eingecheckten Rechners hängen; endet mit dem Check-out. |
| **Session** | Eine Unterhaltung mit dem Agenten; höchstens 2 offen, 1 arbeitet. |
| **Nur Remote / Hybrid** | Modus des Rechners (KONZEPT A2). |
| **Konto** | Nur noch technisch und in A4 (ersetzt durch E7). In der Oberfläche heißt es „Arbeitsplatz“, außer in „UdK-Konto“ und „Einstellungen › Konto“. |

## Gefundene Abweichungen (Stand 28.10., Branches `v3/s2`, `v3/s8`)

- `v3/s2` Desktop: „Auschecken …“ (Knopf) und „Auschecken und Dateien holen“ sind als **Verb** in Ordnung. Das Substantiv bleibt „Check-out“.
- `v3/s8` Dialogtitel „Mitschreiben erlauben?“: Bitte an S8, ihn in „Mitsteuern erlauben?“ zu ändern. Der Dialogsatz aus A9 („… möchte in ‚…‘ mitschreiben.“) bleibt wörtlich.
- `shared/protocol/v1` `pfad_nicht_erlaubt` / tun „Wähle einen Projektordner deines Kontos.“: Bitte an S3, „Wähle einen Ordner in ~/cocopi.“ zu verwenden.
- FREEZE-I2 E7 schreibt „Warteliste“, der wörtliche Text sagt „Warteschlange“. Es gilt **Warteschlange**.
