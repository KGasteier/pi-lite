//! copi – native Oberfläche für pi (Rust + egui).
mod app;
mod commands;
mod headless;
mod icons;
mod store;
mod theme;
mod view;

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, Stroke};

fn load_font(defs: &mut FontDefinitions, name: &str, path: &str) -> bool {
    match std::fs::read(path) {
        Ok(b) => {
            defs.font_data.insert(name.into(), Arc::new(FontData::from_owned(b)));
            true
        }
        Err(_) => false,
    }
}

/// Schriften und dunkles Farbschema (für Fenster und Screenshot gleich).
pub fn setup_ctx(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let base = "/usr/share/fonts";
    let sans = load_font(&mut defs, "sans", &format!("{base}/liberation/LiberationSans-Regular.ttf"));
    let bold = load_font(&mut defs, "sans-bold", &format!("{base}/liberation/LiberationSans-Bold.ttf"));
    let mono = load_font(&mut defs, "mono", &format!("{base}/Adwaita/AdwaitaMono-Regular.ttf"));
    let sym = load_font(&mut defs, "sym2", &format!("{base}/noto/NotoSansSymbols2-Regular.ttf"));
    let sym1 = load_font(&mut defs, "sym1", &format!("{base}/noto/NotoSansSymbols-Regular.ttf"));
    let defaults_p = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let defaults_m = defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let mut fallback: Vec<String> = Vec::new();
    if mono {
        fallback.push("mono".into());
    }
    if sym {
        fallback.push("sym2".into());
    }
    if sym1 {
        fallback.push("sym1".into());
    }
    let mut prop = Vec::new();
    if sans {
        prop.push("sans".to_string());
    }
    prop.extend(defaults_p.clone());
    prop.extend(fallback.clone());
    let mut mon = Vec::new();
    if mono {
        mon.push("mono".to_string());
    }
    mon.extend(defaults_m);
    mon.extend(fallback.iter().filter(|f| *f != "mono").cloned());
    let mut b = Vec::new();
    if bold {
        b.push("sans-bold".to_string());
    }
    b.extend(prop.clone());
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
