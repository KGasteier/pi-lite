# Remote-Hauptchat (copi/cocopi)

## Vertrag

- `NetState::remote_main` kennzeichnet die Remote-Standardansicht unabhängig von
  der Sichtbarkeit der Backends-Leiste. `remote_ctx()` verlangt genau eine
  existierende Remote-Instanz und `grid == false`.
- Remote-Klick ohne Strg/Cmd und Öffnen einer Remote-Session aktivieren diese
  Ansicht. Lokale Instanzauswahl und „Einzel-Chat“ führen zum lokalen Chat.
  „Raster“ und Strg/Cmd-Auswahl zeigen Kacheln, auch bei nur einer Auswahl.
- Der vorhandene `remote_log` zeichnet Markdown, Werkzeuge und Rückfragen ohne
  Kachel-Kopf oder zweite Eingabe. Standard-Senden verwendet `send_prompt`
  (Busy/Waiting: steer); Texteingabe-Rückfragen werden beantwortet.
  Esc/Abbrechen wirken auf die gewählte Remote-Instanz, nicht auf lokales pi.
- Status zeigt Remote-State, keine lokalen Laufzähler oder lokalen Shell-Jobs.
  Nicht verbundene/deaktivierte Backends erhalten einen Hinweis im Hauptchat.
  Die gespeicherte Backend-Aktivierung wird dabei nicht geändert.

## Bedienung

In der Backends-Leiste das Häkchen bei R2 setzen. Danach eine Remote-Session in
 der Sessionliste öffnen oder eine Remote-Instanz anklicken. „Einzel-Chat“ ist
lokal, „Raster“ zeigt die gewählten Kacheln. Die Leiste darf ausgeblendet werden,
ohne das Ziel von Senden/Abbrechen zu wechseln.

## Prüfung und Grenzen

Regressionen in `src/tests.rs`: Auswahl/Moduswechsel, Metadaten, Routing der
Standard-Eingabe, Demo-Senden-Notiz nur nach explizitem Senden, Esc bei versteckter
Leiste, Sessionöffnung, Offline-Hinweis und Frames ohne Panik.
Demo: `COPI_SCENE=remote` (Standard-Remote-Chat), `COPI_SCENE=net` (Mehrfachraster).
Kein echter Server-Chat oder Klicktest im Nutzerfenster. Tauri-Parität bleibt offen.
Release/Installation dieser Änderung wegen weniger als 2 GB freiem Platz auf `/`
nicht ausgeführt; bestehende installierte Binaries bleiben unverändert.
