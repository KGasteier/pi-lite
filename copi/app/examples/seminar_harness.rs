//! Isolierter S8-Prüfstand: echte Seminar-UI, bestehender CPU-Screenshotrenderer.
//! Kein App-Start, keine Konfiguration, kein Node/Hub/Modell und kein I/O außer PNG.
#[cfg(feature = "cocopi")]
#[path = "../src/headless.rs"]
#[allow(dead_code, clippy::chunks_exact_to_as_chunks)]
mod headless;
#[cfg(feature = "cocopi")]
#[allow(dead_code)]
#[path = "../src/desktop/seminar.rs"]
mod seminar;
#[cfg(feature = "cocopi")]
mod theme {
    pub const BG: egui::Color32 = egui::Color32::from_rgb(23, 27, 33);
}
#[cfg(feature = "cocopi")]
fn setup_ctx(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        s.spacing.item_spacing = egui::vec2(12.0, 10.0);
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
    });
}
#[cfg(feature = "cocopi")]
mod app {
    pub struct App {
        pub seminar: super::seminar::Seminar,
    }
    impl App {
        pub fn new(_: egui::Context, _: bool) -> Self {
            Self {
                seminar: super::seminar::scene("v3-seminar-chat").expect("Szene"),
            }
        }
        pub fn demo_scene(&mut self, name: &str) {
            self.seminar = super::seminar::scene(name).expect("Szene");
        }
        pub fn draw(&mut self, ui: &mut egui::Ui) {
            self.seminar.ui(ui);
        }
    }
}
#[cfg(not(feature = "cocopi"))]
fn main() {
    eprintln!("Bitte mit --features cocopi starten.");
}
#[cfg(feature = "cocopi")]
fn main() {
    let dir = std::env::args().nth(1).expect("Ausgabeordner erforderlich");
    std::fs::create_dir_all(&dir).unwrap();
    for scene in [
        "v3-seminar-chat",
        "v3-seminar-dm",
        "v3-mitsteuern-anfrage",
        "v3-mitsteuern-aktiv-student",
        "v3-mitsteuern-aktiv-leitung",
    ] {
        let path = format!("{dir}/{scene}.png");
        headless::shot(&path, (1440, 900), 1.0, Some(scene.into())).expect("PNG");
        println!("{path}");
    }
}
