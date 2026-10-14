# cocopi · externe Einrichtung (noch nichts davon bereitgestellt)

## 1. Server / Netzwerk

- Verantwortliche Person, Test-VM, DNS und TLS festlegen; beispielsweise `relay.<udk-domain>` und `identity.<udk-domain>` — keine bereits existierenden Dienste unterstellen.
- Aktuell unterstützte selbst gehostete iroh-Relay-Version wählen und passend zum Client testen. HTTPS/WebSocket-Upgrade durch Reverse-Proxy; weitere Ports ausschließlich gemäß gewählter Relay-Version öffnen (nicht blind QUIC/UDP durch HTTP-Proxy schicken).
- Relay ohne Inhaltsspeicher, minimale/kurze Logs, Limits und Monitoring; Wartung, Updates, Ausfallkonzept festlegen. Client muss eigene Relay-Liste verwenden, öffentliche Defaults ausschließen.
- Testmatrix: zwei Rechner, Campus-NAT, Heimanschluss, Relay-Fallback und Relay-only. Relay-only darf keine direkten Adressen/Discovery publizieren. Prototyp bietet das noch nicht.
- Kein Postfach zunächst. Optional später nur MLS-Chiffrate mit Größen-/Zeitlimits, autorisierter Zustellung und Quoten. Nicht einfach Klartext-WebSocket-Chat ergänzen.

## 2. authentik

- Bestehenden OIDC-Issuer, Discovery-URL, Administrator und Rollenmodell bestätigen. `auth.udk.digital` stammt aus dem Konzept; dessen konkrete Konfiguration ist hier nicht verifiziert.
- OIDC-Anwendung/Provider für nativen Client: öffentlicher Client, Authorization Code + PKCE S256; kein Client-Secret im Binary. Systembrowser und exakt zugelassene Loopback-Redirects (oder separat geprüftes Device Authorization, falls vorhanden).
- Minimal benötigte Scopes: openid und nur erforderliche Profile-/Rollenclaims. Rollen über serverseitiges Mapping, z.B. `lehrende` und freigegebene Kurskennungen. Keine gesamte Gruppenliste unnötig an Peers weitergeben.
- Issuer, audience, Signatur/JWKS, exp, nonce/state/PKCE prüfen; Refresh-Tokens nur Keychain. Abmelden/Widerruf testen. API-Token und OIDC-Token nicht verwechseln.

## 3. Geräteausweis-Dienst

Benötigt eine eigene kleine, aber sicherheitskritische Implementierung, nicht bloß einen ungeprüften JWT-Signierer.
- OIDC-Login prüfen; Gerätebesitz per kurzlebiger Challenge und Gerätesignatur nachweisen.
- Signierten kurzlebigen Ausweis an Geräteschlüssel binden; versionierter Vertrag mit issuer, audience `cocopi`, subject, Geräteschlüssel, minimalen Rollen/Kursen, issued-at, expiry, ID.
- Signierschlüssel nur serverseitig; öffentliche Trust-Roots/Rotation an Clients verteilen. Kein P2P-Geheimschlüssel zum Server hochladen.
- Lehrendenrolle vor Ausstellen serverseitig prüfen; keine Rollen aus Client-JSON akzeptieren. Gäste getrennt, begrenzte Gruppen-/Zeitrechte.
- Widerruf/kurze Gültigkeit und Offline-Grenzen definieren. authentik-Abmeldung entfernt nicht automatisch ein MLS-Mitglied: Gruppenadministration muss MLS-Commit auslösen.
- Rate-Limits, CSRF-/Replay-Schutz, Datenschutz, minimaler Auditlog. Vor Pilot unabhängig reviewen.

## 4. api.udk.digital einfach anbinden

Der KI-Zugang ist unabhängig vom P2P-Netz. cocopi startet denselben pi-Backendprozess wie copi.
Kein eigener API-Client und kein Verteilen eines gemeinsamen API-Schlüssels an Seminargruppen nötig.

Beobachtet: HTTPS-Startseite erreichbar; `GET /v1/models` ohne Schlüssel liefert einen JSON-Fehler „Invalid token“. Das bestätigt Authentifizierungspflicht, nicht vollständige Protokollkompatibilität.

1. Administrator bestätigt API-Basis, Protokoll, Modell-IDs, Limits und Zugang pro Person; eigenen begrenzten Token ausstellen, nicht authentik-Login-Token einsetzen.
2. Falls OpenAI Chat-Completions unterstützt, lokalen pi-Provider hinzufügen. Aktuelle pi-Doku unterstützt Umgebungsinterpolation. Beispiel in privater `~/.pi/agent/models.json` (bestehende Konfiguration ergänzen, nicht ersetzen):

```json
{
  "providers": {
    "udk-cocopi": {
      "baseUrl": "https://api.udk.digital/v1",
      "api": "openai-completions",
      "apiKey": "${UDK_API_KEY}",
      "models": [{"id": "VOM_ADMIN_BESTAETIGTE_MODELL_ID"}]
    }
  }
}
```

3. `UDK_API_KEY` lokal sicher bereitstellen; nicht ins Repo, Gruppenchat oder Desktop-Datei schreiben. Token muss beim Start von cocopi/pi im Prozess verfügbar sein; Shell-Exports erreichen einen Launcher nicht automatisch. Kein bestehendes env.sh hier verändert.
4. In cocopi Modell bewusst wählen. Zuerst harmlose Einzelanfrage, dann Streaming, Tool Calls, Abbruch und Fehler testen. Falls anderes Protokoll: passendes pi-API-/Providerplugin nach Quellprüfung, keine erfundenen Compatibility-Flags.
5. Kosten-/Rate-Limits je Person setzen. OIDC-to-API-Token-Austausch nur, wenn der Betreiber ihn ausdrücklich implementiert; authentik ist nicht automatisch KI-API-Authentifizierung.
6. Geteilte Karten gehen nie automatisch an diese API. Nutzer prüft, übernimmt und sendet separat; Datei-/Medienuploads brauchen zusätzliche Zustimmung.

## 5. Organisatorische Freigabe

Datenschutzbeauftragte: Verantwortlichkeit, kurze Informationen, Löschfristen, Relay-IP-Logs,
Ausweisclaims, Verarbeitungsverzeichnis und freiwillige Alternative im Seminar klären.
Systemschlüsselbund/Festplattenverschlüsselung und Speicher außerhalb Sync-Ordnern festlegen.
Vor Lehreinsatz: unabhängiger Kryptografie-/Protokollreview, Wiederherstellung/Schlüsselverlust,
Gastablauf, MLS-Ausschluss, KI-Einwilligung und Sicherheitsupdates praktisch abnehmen.
