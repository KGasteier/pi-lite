# Dein Auftrag: S5 · Mobil andocken und Ergebnisse — I2-S5-mobil (≤ 120 min)
Worktree `/home/medienhaus/Work/pi-lite-wt/s5` (aktuell). FREEZE-I2: Handy dockt per k-QR am Slot-Hub des
eingecheckten Rechners an (nur bis Check-out). Umsetzen:
1. Nach Check-out des Rechners: klare Anzeige „Dein Rechner hat ausgecheckt. Dieses Handy ist abgemeldet.“ (401 nach
   Freigabe) und Rückweg „Erneut koppeln“; lokale Daten im Handy-Speicher sofort löschen (nur Schlüssel bleibt, neu
   erzeugbar) – No-Retention.
2. Sessions: Rückfrage beantworten, kurzen Auftrag senden, Abbrechen (A1-Endpunkte), Doppelversand-Schutz (Client-ID je
   Prompt, Knopf gesperrt bis Bestätigung).
3. Ergebnisse ansehen: Liste aus `/v1/sync/vorschau` Kategorie „persönlich“ unter `Ergebnisse/` (read-only), Bild/PDF/Text
   anzeigen (PDF über Browser-Viewer, kein Upload), „Teilen“ über Web Share API wenn vorhanden.
4. Mitsteuern-Hinweis: wenn `session_ereignis` mit `von.rolle=seminarleitung` kommt, Nachricht markiert anzeigen.
5. Barrierefreiheit: Tippflächen ≥ 44 pt, Fokus, `aria-live` für Zustände.
Tests `node --test mobile/test/*.mjs` grün, Integration gegen `/home/medienhaus/Work/pi-hub-wt/s3`. Bericht
`coord/runs/I2-S5-mobil-bericht.md`.
