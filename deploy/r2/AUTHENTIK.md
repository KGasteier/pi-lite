# authentik für cocopi einrichten

**Entwurf zur späteren, ausdrücklich freigegebenen Administration. Hier wurde
nichts in authentik oder auf R2 eingerichtet.** Vor Freigabe S3/S7-Vertrag testen;
`einrichten.sh` bleibt ohne D-R1 und Pool-Abnahme gesperrt.

## 1. Zuständigkeiten und Adressen

Vor Beginn festlegen: authentik-Administration `[Kontakt]`, öffentliche
Empfangs-Origin `https://cocopi.example.invalid`, authentik-Issuer
`https://auth.example.invalid/application/o/cocopi/`, betroffene Gruppen,
Testpersonen und Rücknahmeplan. Beispieldomains ersetzen, keine echten Tokens
in dieses Dokument oder Git übernehmen. Die Issuer-URL muss exakt der
Discovery-Antwort entsprechen, einschließlich abschließendem Slash.

## 2. Gruppen und Rollen

In authentik zwei Gruppen anlegen:

- `cocopi-seminarleitung`: darf Einladungen und erzwungenen Check-out verwalten.
- `cocopi-teilnehmende`: darf einen freien Poolplatz nutzen.

Mitgliedschaft vergibt nur die zuständige Administration. Keine Rolle aus
Client-JSON, Anzeigename oder frei editierbarem Profil ableiten. Wenn beide
Gruppen gelten, ist die Leitung ausdrücklich als höhere Rolle zu bestätigen.
Nichtmitglieder müssen abgewiesen werden. Gäste erhalten ausschließlich kurz
lebende Einmal-Einladungen durch die Leitung, keine persönliche Dauer-Slotbindung.
Alle zehn Unixkonten sind gleichberechtigte, flüchtige Arbeitsplätze.

## 3. OAuth2/OpenID-Provider und Anwendung

In **Applications → Providers → Create → OAuth2/OpenID Provider** (Menünamen
können je authentik-Version abweichen):

1. Name `cocopi Desktop`; passenden Authorization-/Consent-Flow bewusst wählen.
2. **Client type: Public**. Client-ID notieren; **kein Client-Secret** im nativen
   Programm, QR-Code, Mobile Client oder Empfang hinterlegen. Kein Implicit Flow.
3. Authorization Code und **PKCE S256** verlangen; `plain`, fehlenden
   `code_challenge` und fehlenden/falschen Verifier im Negativtest zurückweisen.
   Falls die eingesetzte authentik-Version dies bei öffentlichen Clients nicht
   erzwingt: nicht freigeben, Version/Policy klären.
4. Dedizierten Signierschlüssel und asymmetrischen unterstützten Algorithmus
   wählen (S7 prüft RS256/ES256 über JWKS); keine unkontrollierte Algorithmuswahl.
5. Scopes minimal **`openid profile`**. Kein `email`, kein `offline_access` nötig.
   `profile` darf nur notwendige Namens-/Rollenangaben enthalten. S7 meldet
   derzeit zusätzlich `groups` in `hallo`: vor Pilot mit S1/S7 abgleichen;
   entweder dedizierten `groups`-Scope explizit erlauben oder Gruppenclaim in
   `profile` abbilden und Client-Discovery auf `openid profile` korrigieren.
6. Anwendung `cocopi` an diesen Provider binden. Zugriffsbindung auf die beiden
   Gruppen beschränken; bestehende fremde Anwendungen nicht ändern.

### Loopback-Redirect mit beliebigem Port

Native App startet einen **nur an `127.0.0.1`** gebundenen kurzlebigen Listener auf
freiem Port, öffnet den **Systembrowser**, prüft `state` und `nonce`, tauscht den
Code mit PKCE-Verifier und schließt anschließend den Listener. Kein Webview-Login.
Den Callback-Pfad exakt mit dem freigegebenen Client abgleichen (hier `/callback`).

In authentik Redirect-URI-Matching ausdrücklich **Regex**, vollständig verankert:

```text
^http://127\.0\.0\.1:[0-9]{1,5}/callback$
```

Damit ist jeder vom Betriebssystem gewählte Loopback-Port möglich; der Client
muss den gültigen Bereich 1–65535 verwenden. Manche authentik-Versionen besitzen
eine eigene native-loopback-Portregel: nur nach geprüftem gleichwertigem Verhalten
statt dieser Regex verwenden. Kein `.*`, kein beliebiger Host, kein
`localhost.evil`, kein externer HTTP-Callback, keine fremden Pfade/Queries.
`localhost` und IPv6 `::1` zunächst nicht zusätzlich zulassen. Wenn der tatsächliche
Client einen anderen festen Pfad nutzt, beide Seiten gemeinsam ändern, nicht die
Allowlist verbreitern. Zwei verschiedene zufällige Ports positiv testen.

## 4. Claim-Mapping

Unter **Customization → Property Mappings → OAuth2/OpenID Scope Mapping** einen
dedizierten Mappingeintrag für `profile` (oder den abgestimmten `groups`-Scope)
anlegen. Beispielausdruck, vor Speicherung mit zwei Testkonten prüfen:

```python
return {
    "name": request.user.name,
    "groups": [
        group.name
        for group in request.user.ak_groups.all()
        if group.name in ["cocopi-seminarleitung", "cocopi-teilnehmende"]
    ],
}
```

S7 benötigt signiertes ID-Token mit `iss`, `aud` (= Client-ID), stabilem `sub`,
`exp`, `iat`, passendem `nonce`, `name` und **`groups` als Array von Strings**.
Keine gesamte Organisationsgruppenliste veröffentlichen. Im Provider genau diese
Mappings aktivieren, doppelte widersprechende `groups`-Claims vermeiden. Empfang
prüft Issuer, Audience, JWKS-Signatur, Ablauf und Nonce; Gruppen werden serverseitig
auf `seminarleitung` / `teilnehmende` abgebildet. Name ist Anzeige, keine Berechtigung.
OIDC-Token nur für den Check-in; kein KI-API-Schlüssel und kein dauerhafter Hubtoken.

## 5. `/etc/cocopi/empfang.env` und Schlüssel

Vorlage: [`empfang.env.example`](empfang.env.example). Erst die rootverwaltete
Kopie prüfen, dann jeden einzelnen sudo-Befehl gesondert bestätigen lassen:

```sh
sudo install -d -m 755 -o root -g root /etc/cocopi
sudo install -m 600 -o root -g root /usr/local/lib/cocopi-r2/empfang.env.example /etc/cocopi/empfang.env
sudoedit /etc/cocopi/empfang.env
```

**Nicht** über vorhandene Datei installieren: vorher Existenz prüfen, vorhandene
Datei gezielt mit `sudoedit` ergänzen. Das EnvironmentFile wird von systemd vor
UID-Wechsel gelesen; es ist kein Shellskript. Keine `export`-Anweisungen, kein
`source`, keine Tokens am Terminal anzeigen. Setzen:

- `EMPFANG_PORT=8800`, `EMPFANG_STATE=/var/lib/cocopi-empfang`.
- `EMPFANG_OIDC_ISSUER=<exakte HTTPS-Issuer-URL>`.
- `EMPFANG_OIDC_CLIENT_ID=<öffentliche Client-ID>`.
- `EMPFANG_ORIGIN=<öffentliche HTTPS-Origin ohne Pfad>`.
- `EMPFANG_SLOTS=account01,account02` im gesonderten Zwei-Platz-Test;
  erst nach Abnahme `account01..account10`.
- `EMPFANG_SLOTCTL=/usr/local/lib/cocopi-r2/slotctl-empfang`.
- `EMPFANG_TAGESENDE=23:30`; gilt in Server-Lokalzeit, Zeitzone vorher klären.

Empfang erzeugt Ed25519-Signier- und HMAC-Schlüssel privat im State-Verzeichnis
(0700, Konto `cocopi-empfang`). Keine Schlüssel im Environment erfinden. Öffentlichen
Ed25519-Schlüssel aus geprüftem inhaltsfreiem `/v1/empfang/hallo` entnehmen und
Fingerprint unabhängig bestätigen; exakt diesen **raw/base64url, 43 Zeichen** als
`HUB_EMPFANG_PUBKEY` in `/etc/cocopi/hub-accountNN.env` setzen. Jede Hubumgebung
enthält `HUB_ROOTS=/home/accountNN/arbeit`, loopback Host und eindeutigen Port.
Private Empfangsschlüssel nie ins Slotkonto kopieren. Erster Schlüsselbootstrap
vor allen Slotstarts ist ein gesonderter, freigegebener S7-Integrationsschritt.

## 6. Abnahme vor Netzwerkfreigabe

Nur synthetische Konten und Dateien, keine echten Arbeiten/Modelle:

- Teilnehmer → richtige Rolle, Leitung → Leitung, fremde Gruppe → abgewiesen.
- Falscher Issuer/Audience/Signatur, abgelaufenes Token, fehlende/falsche Nonce,
  wiederholter Code/Token und PKCE ohne S256 → abgewiesen.
- Falscher Redirect-Host/Pfad/Portformat → abgewiesen; zwei freie Loopback-Ports
  funktionieren; Callback erreicht ausschließlich den eigenen App-Listener.
- Voller Zehnerpool → FIFO-Warteschlange, keine Priorität für Leitung.
- Checkout: Archiv bestätigt empfangen → signierte Freigabe → Leeren → echter
  Leer-Nachweis vor Hubstart → erst dann Wiedervergabe. Fehler = gesperrt.
- Zwangscheckout: nur verschlüsseltes Abholfach, gleiche Person/Schlüssel erforderlich,
  spätestens 72 h; Fehler bei Archiv/Leeren darf nicht freigeben.
- Proxy-Sperren direkt UND unter Slotpfaden testen; Admin-Endpunkte niemals
  allein wegen Proxy-Loopback als vertrauenswürdig betrachten.
- `admin-check.sh` muss wirkliche Befunde liefern; „nicht geprüft“ ist kein Grün.

authentik-Abmeldung beendet nicht automatisch alle bereits ausgestellten
12-Stunden-Geräteausweise. Widerruf/Gruppenwechsel/Schlüsselrotation und Folgen
für laufende Check-ins separat mit S3/S7 abnehmen. Keine produktive Freigabe
allein aufgrund dieses Konfigurationsdokuments.
