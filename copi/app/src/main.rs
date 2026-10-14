//! copi – native Oberfläche für pi (Rust + egui).
mod app;
mod bookmark_palette;
mod commands;
mod headless;
mod icons;
mod material_ui;
#[path = "../../../shared/bookmarks.rs"]
mod shared_bookmarks;
mod run_activity;
mod store;
mod theme;
mod view;
mod virtual_log;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, Stroke};

/// Schriften und dunkles Farbschema (für Fenster und Screenshot gleich).
///
/// Liest absichtlich **keine** Schriftdateien vom Dateisystem (kein `/usr/share/fonts`-Zugriff
/// mehr): Das Original tat dies, wodurch das Erscheinungsbild von zufällig installierten
/// System-Schriften abhing und auf macOS/Windows (wo `Adwaita`/`noto`-Pakete unter diesen
/// Pfaden nicht existieren) ohnehin lautlos auf etwas anderes zurückfiel. Stattdessen werden
/// drei Schriftarten per `include_bytes!` fest eingebettet: Figtree (SIL Open Font License
/// 1.1, siehe `assets/fonts/OFL.txt`) für Fließtext/Fettdruck sowie Noto Sans Symbols 2
/// (Google Noto-Projekt, Apache License 2.0, siehe
/// `assets/fonts/NotoSansSymbols2-LICENSE-APACHE2.txt`) als letzter Symbol-Fallback für
/// Zeichen wie ☑☐•◦▪↑↓→←×⌘, die zwar im UI verwendet werden, aber weder von Figtree
/// noch von den mit `eframe`/`epaint` (Feature `default_fonts`) ohnehin schon
/// eingebetteten Standardschriften (`Hack`/`Ubuntu-Light`/`NotoEmoji-Regular`/
/// `emoji-icon-font`, die zwischen Figtree und Noto Sans Symbols 2 eingereiht werden)
/// abgedeckt sind. Dadurch ist das Schriftbild auf allen drei Plattformen identisch,
/// ohne Datei-I/O außerhalb des Binaries.
pub fn setup_ctx(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    defs.font_data.insert("sans".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/Figtree-Light.ttf"))));
    defs.font_data.insert("sans-regular".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/Figtree-Regular.ttf"))));
    defs.font_data.insert("sans-bold".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/Figtree-SemiBold.ttf"))));
    defs.font_data.insert("noto-symbols2".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/NotoSansSymbols2-Regular.ttf"))));
    // Von epaint/egui (Feature `default_fonts`) bereits eingebettete Standardschriften, als
    // weiterer Fallback hinter Figtree/Noto Sans Symbols 2.
    //
    // Wichtig: `noto-symbols2` wird bewusst *nach* den Standardschriften eingereiht
    // (statt davor), weil epaint die erste Schrift in der Kette, die das
    // Platzhalter-Ersatzzeichen `◻` enthaelt, intern als "Ersatzglyph-Schrift"
    // merkt. `NotoSansSymbols2-Regular.ttf` enthaelt `◻` ebenfalls; stuende es an
    // erster Stelle, wuerde epaint faelschlich *jedes* ueber diese Schrift
    // aufgeloeste Zeichen als "nicht vorhanden" (= Ersatzglyph) werten, obwohl es
    // korrekt gerendert wird (siehe `CachedFamily::new`/`find_face_for_char` in
    // epaint). Die eingebauten Standardschriften (u.a. `NotoEmoji-Regular`,
    // `Hack`) liefern bereits frueher in der Kette ein gueltiges `◻`, sodass
    // `noto-symbols2` nur als reiner Fallback fuer fehlende Symbole dient.
    let defaults_p = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let defaults_m = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let mut prop = vec!["sans".to_string()];
    prop.extend(defaults_p);
    prop.push("noto-symbols2".to_string());
    let mut mon = defaults_m;
    mon.push("noto-symbols2".to_string());
    let mut b = vec!["sans-bold".to_string()];
    b.extend(prop.clone());
    // Regular beibehalten, mathematische Zeichen wie ≈ über vorhandene Fallbacks.
    let mut regular = vec!["sans-regular".to_string()];
    regular.extend(prop.iter().filter(|name| name.as_str() != "sans").cloned());
    defs.families.insert(FontFamily::Name("status-regular".into()), regular);
    defs.families.insert(FontFamily::Proportional, prop);
    defs.families.insert(FontFamily::Monospace, mon);
    defs.families.insert(FontFamily::Name(md_egui::BOLD_FAMILY.into()), b);
    ctx.set_fonts(defs);

    ctx.all_styles_mut(|s| {
        let mut v = egui::Visuals::dark();
        v.panel_fill = theme::BG;
        v.window_fill = theme::PANEL;
        v.window_stroke = Stroke::new(1.0, theme::BORDER);
        v.extreme_bg_color = theme::BG;
        v.faint_bg_color = theme::PANEL;
        v.hyperlink_color = egui::Color32::from_rgb(0x80, 0x95, 0xff);
        v.selection.bg_fill = theme::alpha(theme::ACC, 0.45);
        v.selection.stroke = Stroke::new(1.0, theme::FG);
        v.text_cursor.stroke = Stroke::new(2.0, theme::ACC);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme::FG);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme::BORDER);
        v.widgets.inactive.bg_fill = theme::BTN;
        v.widgets.inactive.weak_bg_fill = theme::BTN;
        v.widgets.hovered.bg_fill = theme::SELECT_HOVER;
        v.widgets.hovered.weak_bg_fill = theme::SELECT_HOVER;
        v.widgets.active.bg_fill = theme::PICK_OPEN;
        v.widgets.active.weak_bg_fill = theme::PICK_OPEN;
        s.visuals = v;
        s.spacing.button_padding = egui::vec2(10.0, 6.0);
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(18.0));
    });
}

struct Main(app::App);

impl eframe::App for Main {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.draw(ui);
    }

    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(theme::BG).to_array()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let shot = arg("--shot").or_else(|| std::env::var("COPI_SHOT").ok());
    let demo_env = std::env::var("COPI_DEMO").map(|v| !v.is_empty() && v != "0").unwrap_or(false);
    if let Some(path) = shot {
        // Screenshots laufen immer im Demo-Modus: pi wird nie gestartet.
        let size = arg("--size")
            .and_then(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
            .unwrap_or((1280, 800));
        let ppp = std::env::var("COPI_PPP").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
        let scene = std::env::var("COPI_SCENE").ok().filter(|s| !s.is_empty());
        match headless::shot(&path, size, ppp, scene) {
            Ok(()) => println!("{path}"),
            Err(e) => {
                eprintln!("Screenshot fehlgeschlagen: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_app_id("copi").with_title("copi").with_inner_size([1280.0, 800.0]).with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    let r = eframe::run_native(
        "copi",
        options,
        Box::new(move |cc| {
            setup_ctx(&cc.egui_ctx);
            Ok(Box::new(Main(app::App::new(cc.egui_ctx.clone(), demo_env))))
        }),
    );
    if let Err(e) = r {
        eprintln!("copi: {e}");
        std::process::exit(1);
    }
}
