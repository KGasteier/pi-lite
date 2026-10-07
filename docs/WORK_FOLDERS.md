# Arbeitsordner-Auswahl

Beim Start ist der tatsächliche Startordner im Arbeitsordnerfeld voreingestellt.
Die benachbarte Auswahl bietet Standard (Startordner), Home, vorhandene Shared-Folder
(`/mnt/mac`, `/media/sf_shared`, `/mnt/shared`) und „Ordner auswählen …“.
Es wird nur Existenz geprüft, nicht ob ein echtes Shared-Mount vorliegt.

Der eingebaute Ordnerbrowser zeigt Unterordner einschließlich versteckter Ordner.
Enter/Klick öffnet einen Unterordner; „Übergeordnet“ navigiert zurück. Ein Pfad
kann direkt eingegeben und mit Enter geöffnet werden. „Diesen Ordner wählen“
übernimmt den angezeigten Ordner, Abbrechen/Esc verwirft die Auswahl.
Tab bleibt im Dialog; Pfeile navigieren die Knöpfe. Im Pfadfeld gelten normale
Texttasten. Dateisystemzugriffe laufen im Hintergrund, Fehler werden angezeigt.

Ein ausgewählter Ordner gilt für die **nächste neue Session**. Der aktuelle Agent
wird nicht neu gestartet. Bestehende Sessions setzen beim Öffnen ihren eigenen
Arbeitsordner. Die Wahl wird nicht zwischen App-Neustarts gespeichert: Der
Startordner bleibt der Standard. Die Ressourcenübersicht folgt wie bisher dem
Arbeitsordnerfeld, nicht automatisch dem noch laufenden Agenten.

JavaScript-Syntax, History-/Vorschautests, sechs bestehende Rust-Tests und
Release-Build erfolgreich. Der Ordnerbrowser ist noch nicht live geprüft und hat
noch keine spezifischen automatisierten Tests. Kein nativer Betriebssystemdialog.
