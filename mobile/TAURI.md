# Tauri 2 Mobile · Auslieferungsplan, kein gebautes Produkt

I1 liefert PWA-Dateien/Transport. Es gibt **kein** Android-/iOS-Projekt, APK/AAB, TestFlight-Build oder Signaturnachweis. Keine neue Abhängigkeit/Installation in I1. Desktop bleibt nativ egui ohne Webview (E2); dieser Plan gilt nur mobil (E3).

## Hülle (I2/I3, eigene Freigabe)

1. Tauri-2-Mobile-Projekt mit denselben `mobile/`-Dateien und genau diesem Transport; keine zweite Chatimplementierung. Der Hub muss als freigegebene HTTPS-Adresse konfiguriert sein. Zugelassene Ursprünge in CSP/Capabilities klein halten; keine Navigation beliebiger QR-Hosts im privilegierten Webview.
2. Native Brücke nur für Geräteidentität, Signatur, Kamera und später Push. Nicht beliebige Dateisystem-/Shell-/HTTP-Plugins freischalten. Token bleiben kurzlebig im RAM. Eine HTTPS-Origin-Änderung darf keine stillschweigende Übernahme von Hostvertrauen sein.
3. Geräte-ID/öffentlicher Schlüssel in geschützter Appablage; privater Schlüssel im nativen Schutzbereich. iOS Keychain `ThisDeviceOnly`/nur entsperrt, Android Keystore mit Appbindung und Backup-Ausschluss prüfen. **Nicht behaupten**, dass Ed25519 im Secure Enclave/Hardware-Keystore überall unterstützt ist: Ed25519-Unterstützung und nicht-exportierbare Signaturfunktion auf Mindest-OS-Versionen praktisch prüfen. Falls native API nur andere Kurven anbietet, S3-Vertragsentscheidung nötig; kein stiller Algorithmenwechsel. Software-Keychain/verschlüsselte appgebundene Ablage nur nach Sicherheitsreview, nicht als Hardware-Schlüssel bezeichnen.
4. Abmelden: serverseitigen Widerruf bestätigen, dann Schlüssel entfernen. App-Neuinstallation/OS-Backup, Sperrbildschirm, verloren gegangenes Gerät, Browser→App-Wechsel und Tokenablauf getrennt testen. Push-Gerätebindung bei Widerruf ebenfalls entfernen.

## Rechte – nur im Kontext fragen

| Fähigkeit | iOS | Android | Zeitpunkt / Ablehnung |
|---|---|---|---|
| QR-Kamera | verständliche `NSCameraUsageDescription` („Zum Scannen des Codes am Rechner.“), Systemdialog | `CAMERA`, Laufzeitfreigabe | Erst bei „Kamera öffnen“. Ohne Erlaubnis bleibt Code eintippen verfügbar. Keine Galerie-/Mikrofonrechte. |
| Benachrichtigung | UserNotifications/APNs-Berechtigung; entitlements/Signierung separat | Android 13+ `POST_NOTIFICATIONS`, Kanal | Erst bei Opt-in in M-EI, I2. Nur „fertig“/„wartet auf dich“, keine Titel/Prompt-/Ergebnisinhalte. Ablehnung beeinträchtigt Sessionlesen nicht. |
| Netzwerk | ausschließlich HTTPS/ATS | Internet, kein unbeschränktes Cleartext | Hubadresse vorher prüfen; Loopback-HTTP nur isolierter Entwicklungsbuild. |

Kein dauerhafter Kamera-/Mikrofon-/Datei-/Standortzugriff, keine Hintergrund-SSE-Garantie. Nach Vordergrundwechsel Zustand/Verlauf neu abgleichen. Push ersetzt nicht den authentisierten Verlauf.

## Fragment vs. Query – Antwort an S1/S3

- **Empfehlung bleibt Freeze A3:** Erzeuger schreiben HTTPS-Fragment `.../c#1.k.<code>`. Webparser akzeptiert zusätzlich `?c=1.k.<code>` und Rohcode. Der Code ist nie ein Zugangstoken und wird erst nach Bestätigung eingelöst.
- **Nicht auf Geräten nachgewiesen:** Ob Androids Kamera-Intent und iOS Universal Links das Fragment in dieser konkreten Tauri-Konfiguration vollständig erhalten, ist auf R2 ohne Geräte/Toolchains nicht entscheidbar. Orcas Query-Begründung ist kein eigener Plattformnachweis; kein Orca-Code übernommen.
- Prüfreihe: iOS Kamera → Safari/App und Android Kamera (mindestens zwei verbreitete Scanner) → Browser/App, Vorder-/Hintergrund und Coldstart, beide URL-Formen, abgelaufene/verbrauchte Codes. App Links/Universal Links und Domainzuordnung brauchen Betreiberfreigabe. Intentdaten als untrusted behandeln, nie automatisch koppeln oder eine Aktion bestätigen.
- **Query ist nur Kompatibilitätseingang, kein neuer Standard.** Querycodes können schon beim ersten Navigationsrequest in Proxy-/Serverlogs gelangen. Parserseitiges `history.replaceState` verhindert das nicht. S3/Betreiber müssen `/c`-/`/m/`-Accesslogs redigieren/deaktivieren und Referrer unterdrücken. Fragment bevorzugen; bei nachgewiesenem Verlust manuelle Codeeingabe anbieten. Änderung des erzeugten Formats nur Antrag an K/S1/S3.
- Rechnerbestätigung (`b`) erst implementieren, wenn eine **authentisierte** Aktionsvorschau vom Hub vorliegt (Beschreibung/Bindung/Frist, Zustimmung separat). QR-Inhalt allein darf keinen Freigabetext bestimmen. Aktuell wird b nur erkannt und erklärt abgewiesen.

## Freigaben und Abnahme

Apple-/Google-Konten, Bundle-ID, Signierung, Domain/Associated Domains, Gerätezugang und Verteilung (PWA/APK/TestFlight/Store) entscheidet der Mensch (D-M1). Keine Anmeldung/Storekosten in diesem Auftrag. Vor Pilot: mindestens ein echtes iPhone und Android, Schlüssel-/Backup-/Widerrufstests, Kamera-Fallback, Push ohne Inhalte, offline/Funkloch/Replay, Tab-Neustart und barrierefreie Bedienung. Native Hülle und PWA getrennt abnehmen.
