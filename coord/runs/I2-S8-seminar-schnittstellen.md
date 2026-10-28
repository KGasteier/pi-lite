# S8 → K/S2/S3/S7 · Schnittstelle in Arbeit

S8 nutzt S7-Hülle `{inhalt:{…A6},signatur:base64url}` und rekursiv sortiertes JSON, Zeiten Unix-Sekunden. pi-net verifiziert mit iroh-Ed25519 (keine neue Dependency). Neuer optionaler Ausweisaustausch; v1 bleibt unverändert nutzbar.

S2/K bitte `desktop/seminar.rs` als Modul (Feature `cocopi`) registrieren, ein Feld `Seminar` und genau einen Zugang `seminar_tab(&mut self,ui)` anbieten; `seminar::scene(name)` bei `Desktop::scene` berücksichtigen. S8 ändert desktop.rs nicht.

S2 bitte `Desktop::send_as(von,text)` (ggf. mit expliziter Session-ID) für vom Teilnehmer lokal geprüfte Freigaben bereitstellen; S8 liefert vorerst `SeminarNet::senden(session,text,von)` und Mock. Kein fremder Hub-Token wird geteilt. Zertifikat muss auch lokal gültig sein. Detailbericht folgt.
