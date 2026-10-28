# Dein Auftrag: S5 · Signatur M3 nachziehen — I1-S5-fix (≤ 40 min)

Rolle S5, Worktree `/home/medienhaus/Work/pi-lite-wt/s5` (`v3/s5`). Zuerst `git merge --no-edit cocopi-v3`
(enthält jetzt S3-Fix: Protokoll v1 mit **kontextgebundener Anmeldesignatur**, siehe `shared/protocol/v1/PROTOKOLL.md`
Abschnitt Anmeldung, und `coord/runs/I1-S3-fix-bericht.md` Zeile M3).
Signiert wird exakt `cocopi-v1-anmelden\n<hub_origin>\n<slot>\n<geraet_id>\n<challenge_id>\n<nonce>` (LF, kein Schluss-LF;
`hub_origin` = `new URL(baseUrl).origin` aus der bewusst eingelösten Einladung, **nicht** aus der Challenge-Antwort;
`slot` aus dem gespeicherten Konto). Passe `ui/transport.js` + `mobile/device.js` an, Tests in `mobile/test/` inkl.
„Signatur für Hub A wird von Hub B abgewiesen“. Der Integrationstest nutzt den Hub unter
`/home/medienhaus/Work/pi-hub-wt/s3` (jetzt `e2eabfa`) und muss grün sein; prüfe auch, dass b-Codes nicht mehr per SSE
kommen (H2) und Kopplung bei vorhandenem Gerät wartet (H1) — passe nur deinen Client an, nicht den Hub.
`node --test mobile/test/*.mjs` grün. Bericht nach `coord/runs/I1-S5-fix-bericht.md`.
