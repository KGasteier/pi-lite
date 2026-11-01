# Docker/Coolify · cocopi-v3d

Separater Infrastrukturpilot. Kein Ersatz des Unixpools in `deploy/r2` und noch
kein produktionsfertiger cocopi-v3-Containerpool. Beide Varianten bleiben erhalten.

## Dateien und sichere Reihenfolge

1. `bash 01-bestand.sh`: nur lesendes Hostinventar.
2. Coolify/Docker gesondert freigeben und gemäß offiziellem Verfahren installieren.
3. `bash 05-image-kontext.sh /pfad/pi-hub /neues/buildverzeichnis`: erlaubt nur
   getrackte Laufzeitdateien, keine persönlichen Konfigurationen/Token.
4. Image aus dem neuen Kontext mit geprüftem Node22-Digest bauen; pi ist auf
   1.4.0 gepinnt. Onlinebuild lädt Pakete; vorher genehmigen. Rust ist noch nicht
   enthalten. Image scannen, prüfen und unter Registry-Digest bereitstellen.
5. `bash 02-compose-erzeugen.sh 10 > compose.yaml`: zehn unabhängige Services,
   je eigenes Netzwerk/Home/Hubtoken, Ports ausschließlich Hostloopback.
6. `03-pilot-ablagen.sh`: standardmäßig Plan. Apply braucht Root und
   `COCOPI_CONTAINER_STORAGE_FREIGABE=ja`. Legt nur einen **neuen** festen
   Ablagebaum an, keine Quoten oder Löschlogik. Vor echten Daten Speicherquoten
   implementieren/abnehmen. Keine Überschreibung/Übernahme bestehender Pfade.
7. Compose in Coolify importieren, Digest konfigurieren, zunächst einen Service,
   dann drei, erst nach Ressourcenabnahme zehn starten. Keine öffentliche Domain
   im Infrastrukturpilot; Zugriff nur kontrolliert über SSH-Tunnel.
8. `04-pilot-modelle-container.sh`: optionale betreute kurzlebige Credentials als
   UID1000 im richtigen Container. Kein Key im Image/Compose/Chat. Kosten, Zuordnung,
   Lebensdauer und serverseitigen Widerruf separat freigeben.

## Noch fehlende Integration

Empfang akzeptiert aktuell nur Loopback-Slot-URLs. Belegung/Freigabe/interne
Syncdienste verlangen Loopback als Quelladresse. Der bisherige Roothelfer verwaltet
Unixhomes/systemd, nicht Docker. Ein Bridge-Deployment ist deshalb **nicht** durch
Setzen von `HUB_HOST=0.0.0.0` allein kompatibel.

Vor vollständigem cocopi-v3-Betrieb braucht es signierte, replaygeschützte interne
Containerkommunikation, eng begrenzten Lifecyclebroker ohne Docker-Socket in
Agenten/Empfang, atomare Epochen/Leerbelege, sichere volumenbasierte Löschung und
Reconciliation mit Coolify. Der bestehende OIDC/API-Credential-Broker fehlt ebenso.
Prüfungen nicht abschalten und keinen Hostnetzmodus als Abkürzung verwenden.

## Sicherheit und Grenzen

- Container laufen nichtprivilegiert, ohne Caps/Dockersocket/Hostnetz, mit
  schreibgeschütztem RootFS und CPU/RAM/PID-Grenzen. Das ist keine VM-Isolation.
- Normale bind mounts/named volumes haben **keine automatische 80-GB-Quote**.
  Gemeinsame UID1000 im Container bedeutet ohne Userns auch gemeinsame Host-UID;
  das ist keine Unix-UID-Quotentrennung. Storage separat projektquotieren.
- Home/Token bleiben nach Containerstop oder Recreate bestehen. Kein automatisches
  Checkout-Leeren im Pilot. Kein `docker compose down -v` als Löschvertrag.
- `logging: driver: none` vermeidet Runtimeinhalte im Dockerlog; Deployment-/Build-
  und Coolify-/Proxylogs bleiben gesondert zu prüfen. Debug separat genehmigen.
- Healthcheck beweist nur authentifiziertes Hub-HTTP, nicht Modell/OIDC/P2P.
- Coolify-Verarbeitung kann Netzwerke/Labels/Ports verändern: erzeugtes Deployment
  auf dem Host prüfen. Keine ungetesteten Sicherheitsgarantien aus YAML ableiten.

Vollständige Anleitung: separat ausgeliefertes `cocopi-v3d-Sysadmin-Docker-Coolify`.
