# Modellidentität und Kontextdateien

pi-lite ergänzt beim Start des Haupt-RPC-Prozesses eine Systemanweisung via
`--append-system-prompt`. Der vorhandene Systemprompt und Projektkontext bleiben
erhalten. CLAUDE.md wird nicht gelöscht und Kontextdateien werden nicht deaktiviert.

Die Anweisung unterscheidet Harness-/Projektkonventionen von Modellidentität.
Selbstauskünfte und geerbte PI_MODEL/PI_PROVIDER-Variablen sind keine Beweise.
Bei Modellfragen ist der aktuelle Sessionzustand maßgeblich, sofern verfügbar;
andernfalls auf die Modellanzeige verweisen und Unsicherheit ausdrücken. Eine
Provider-ID belegt nicht unabhängig das Modell hinter einem Gateway. Frühere
unbelegte Identitätsbehauptungen sollen korrigiert werden.

Gilt nach Neustart und anschließendem Öffnen der Session. Frühere Nachrichten
werden nicht verändert. Dies ist eine Verhaltensregel, keine technische Garantie
gegen falsche Antworten. Andere pi-Frontends und die globale CLAUDE.md bleiben
unverändert. Rust-Tests und Build erfolgreich; neuer Modellantworttest nicht erfolgt.
