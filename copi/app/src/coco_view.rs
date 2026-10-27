//! cocopi-Oberfläche: rechte Leiste (Chats, Kontakte, Räume/Rollen, Agenten), Chat und
//! Fernsitzung im Hauptbereich, Dialoge (neuer Chat, Mitglieder, Persona, Fernzugriff).
//!
//! Liest `App::coco`, ändert nur `App::coco.ui` direkt; alles andere als
//! [`crate::coco::CocoAct`] über `App::coco_apply`.
use crate::app::App;

/// Reiner Oberflächenzustand (Auswahl, Entwürfe, offene Dialoge).
#[derive(Default)]
pub struct CocoUi {}

impl App {
    /// Inhalt der rechten Leiste in cocopi (ersetzt den alten Prototyp-Bereich).
    pub(crate) fn coco_panel(&mut self, ui: &mut egui::Ui) {
        let _ = ui;
    }
    /// Hauptbereich, wenn `coco.main` gesetzt ist.
    pub(crate) fn coco_main_view(&mut self, ui: &mut egui::Ui) {
        let _ = ui;
    }
    /// Dialoge/Overlays (nach dem Zeichnen der Panels).
    pub(crate) fn coco_overlays(&mut self, ctx: &egui::Context) {
        let _ = ctx;
    }
    /// Platzhalter für die normale Eingabe, wenn `coco.main` gesetzt ist.
    pub(crate) fn coco_placeholder(&self) -> Option<&'static str> {
        None
    }
    /// Demo-Szenen für Screenshots (`--shot … --scene coco-…`): füllt `coco` mit synthetischen Daten.
    pub(crate) fn coco_demo(&mut self, scene: &str) -> bool {
        let _ = scene;
        false
    }
}
