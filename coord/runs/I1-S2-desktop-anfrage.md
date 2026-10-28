# I1-S2 → S3/K · Schnittstellenanfrage (aktualisiert)

Stand 28.10., 19:20. Branch `v3/s2`, Desktop `9c8857c`; S3 bis `64e7154` gemergt.
120 cocopi/100 copi Tests und isolierter App-Adapter-Mocktest grün.

## Weiter offen: Fehlercodes im Rust-Client

`pi_backend::hub_v1::packet_error()` verwirft noch `code` und liefert nur `was tun`.
Bitte S3 additiv `[code] was tun` liefern oder typisierten Fehler anbieten. S2
editiert keine Crates und implementiert keinen zweiten HTTP-Client. Der App-Adapter
versteht bereits `[abgelaufen]`, `[verbraucht]`, `[unbekannt]`, `[zu_viele]`,
`[zu_viele_offen]`, `[nicht_angemeldet]`. Ohne Code funktioniert im echten HTTP-Pfad
nur die generische Fehleranzeige; die exakten A3-Texte/Limitdialog sind dann nicht
vollständig erreichbar. K muss das vor I1-Abnahme als Lücke behandeln.

## Erledigte Anfragen

- S3 `open_session/send/stop/rest/view` sind verfügbar und angebunden.
- Zweiter Desktop: `bestaetigung`-QR plus `pairing_status` angebunden.
- Prüfwort: Freeze noch einmal geprüft, **in I1 optional**, I2 Pflicht.
  Fehlendes Prüfwort wird erklärt, Zustimmung nach Gerätenamenprüfung bleibt möglich.
  Frühere Heartbeat-Aussagen über gesperrte Freigabe sind überholt.

Weitere App-Lücken stehen im Abschlussbericht: lokale Hybrid-Sessions, Rückfragen,
Verlauf nach Reconnect, Modellpicker und Ressourcenanzeige sind nicht vollständig.
