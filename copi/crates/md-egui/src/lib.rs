//! md-egui – rendert die IR aus `md-core` mit egui.
//!
//! * Text ist auswählbar (egui-Labels; Auswahl auch über mehrere Labels hinweg).
//! * Links sind im Fließtext klickbar (Treffer per Galley-Cursor), ohne die Auswahl zu stören.
//! * Codeblöcke bekommen einen Rahmen, Sprach-Hinweis, Kopier-Knopf und leichtes Highlighting.
//! * Optik orientiert sich an pi-lite (`.md`-CSS, hljs „GitHub Dark“).
//!
//! Fett benötigt eine eigene Schriftfamilie [`BOLD_FAMILY`] (egui kennt keine Schriftstärken).
//! Ist sie nicht registriert, wird auf die normale Proportional-Schrift zurückgefallen.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, RichText, Sense, Stroke, Ui};
use md_core::highlight::{highlight, Tok};
use md_core::{Align, Block, Doc, Span};

/// Name der Schriftfamilie für Fettdruck (vom Host-Programm zu registrieren).
pub const BOLD_FAMILY: &str = "bold";

/// Farben und Größen des Renderers.
#[derive(Clone, Debug)]
pub struct Theme {
    pub size: f32,
    pub code_scale: f32,
    pub fg: Color32,
    pub dim: Color32,
    pub link: Color32,
    pub code_bg: Color32,
    pub border: Color32,
    pub heading: Color32,
    pub check: Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            size: 15.0,
            code_scale: 0.87,
            fg: Color32::from_rgb(0xe6, 0xed, 0xf3),
            dim: Color32::from_rgb(0x8b, 0x94, 0x9e),
            link: Color32::from_rgb(0x80, 0x95, 0xff),
            code_bg: Color32::from_rgb(0x16, 0x1b, 0x22),
            border: Color32::from_rgb(0x3a, 0x40, 0x46),
            heading: Color32::from_rgb(0xe6, 0xed, 0xf3),
            check: Color32::from_rgb(0x18, 0xa9, 0x99),
        }
    }
}

impl Theme {
    fn code_size(&self) -> f32 {
        (self.size * self.code_scale).round()
    }
}

/// hljs „GitHub Dark“-Farben je Token-Art.
pub fn token_color(t: Tok) -> Color32 {
    match t {
        Tok::Plain => Color32::from_rgb(0xc9, 0xd1, 0xd9),
        Tok::Keyword => Color32::from_rgb(0xff, 0x7b, 0x72),
        Tok::Literal | Tok::Number => Color32::from_rgb(0x79, 0xc0, 0xff),
        Tok::String => Color32::from_rgb(0xa5, 0xd6, 0xff),
        Tok::Comment => Color32::from_rgb(0x8b, 0x94, 0x9e),
        Tok::Function => Color32::from_rgb(0xd2, 0xa8, 0xff),
        Tok::Type => Color32::from_rgb(0xff, 0xa6, 0x57),
        Tok::Added => Color32::from_rgb(0xaf, 0xf5, 0xb4),
        Tok::Removed => Color32::from_rgb(0xff, 0xdc, 0xd7),
        Tok::Section => Color32::from_rgb(0x1f, 0x6f, 0xeb),
    }
}

/// Ergebnis einer Darstellung: ggf. angeklickter Link.
#[derive(Default)]
pub struct Output {
    pub clicked_link: Option<String>,
}

/// Kompletten Markdown-Text darstellen (parst bei jedem Aufruf – für lange Texte
/// besser [`show_doc`] mit einem zwischengespeicherten [`Doc`] nutzen).
pub fn show_markdown(ui: &mut Ui, theme: &Theme, src: &str) -> Output {
    show_doc(ui, theme, &md_core::parse(src), "md")
}

/// Geparstes Dokument darstellen. `id_salt` muss pro Dokument eindeutig sein.
pub fn show_doc(ui: &mut Ui, theme: &Theme, doc: &Doc, id_salt: impl std::hash::Hash + std::fmt::Debug) -> Output {
    let mut out = Output::default();
    ui.push_id(id_salt, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        blocks(ui, theme, &doc.blocks, &mut out, 0);
    });
    out
}

fn gap(ui: &mut Ui, theme: &Theme) {
    ui.add_space(theme.size * 0.6);
}

fn blocks(ui: &mut Ui, theme: &Theme, bs: &[Block], out: &mut Output, depth: usize) {
    for (i, b) in bs.iter().enumerate() {
        if i > 0 {
            gap(ui, theme);
        }
        ui.push_id(i, |ui| block(ui, theme, b, out, depth));
    }
}

fn block(ui: &mut Ui, theme: &Theme, b: &Block, out: &mut Output, depth: usize) {
    match b {
        Block::Paragraph(s) => inline_label(ui, theme, s, theme.size, theme.fg, false, out),
        Block::Heading { level, text } => {
            let scale = match level {
                1 => 1.6,
                2 => 1.35,
                3 => 1.17,
                _ => 1.0,
            };
            ui.add_space(theme.size * 0.2);
            inline_label(ui, theme, text, theme.size * scale, theme.heading, true, out);
        }
        Block::Code { lang, text } => code_block(ui, theme, lang, text),
        Block::Rule => {
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(egui::vec2(w, 9.0), Sense::hover());
            ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, theme.border));
        }
        Block::Quote(inner) => {
            let mut t = theme.clone();
            t.fg = theme.dim;
            t.heading = theme.dim;
            let resp = egui::Frame::NONE.inner_margin(Margin { left: 15, right: 0, top: 0, bottom: 0 }).show(ui, |ui| {
                blocks(ui, &t, inner, out, depth);
            });
            let r = resp.response.rect;
            ui.painter().vline(r.left() + 1.5, r.y_range(), Stroke::new(3.0, theme.border));
        }
        Block::List { start, items } => {
            for (n, it) in items.iter().enumerate() {
                if n > 0 {
                    ui.add_space(theme.size * 0.25);
                }
                let marker = match (it.task, start) {
                    (Some(true), _) => "☑".to_string(),
                    (Some(false), _) => "☐".to_string(),
                    (None, Some(s)) => format!("{}.", s + n as u64),
                    (None, None) => ["•", "◦", "▪"][depth % 3].to_string(),
                };
                let mcol = if it.task == Some(true) { theme.check } else { theme.dim };
                ui.push_id(n, |ui| {
                    ui.horizontal_top(|ui| {
                        let w = theme.size * 1.5;
                        ui.allocate_ui_with_layout(egui::vec2(w, theme.size), egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            ui.add_space(theme.size * 0.35);
                            ui.add(egui::Label::new(RichText::new(marker).size(theme.size).color(mcol)).selectable(false));
                        });
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for (k, b) in it.blocks.iter().enumerate() {
                                if k > 0 {
                                    ui.add_space(theme.size * 0.3);
                                }
                                ui.push_id(k, |ui| block(ui, theme, b, out, depth + 1));
                            }
                        });
                    });
                });
            }
        }
        Block::Table { align, head, rows } => table(ui, theme, align, head, rows, out),
    }
}

/// Fallback-Hervorhebung für Fettdruck, wenn `BOLD_FAMILY` nicht registriert ist.
///
/// Statt die Textfarbe hart auf Weiß zu setzen (was auf hellen Themes unlesbar
/// wäre), wird die ursprüngliche Farbe Richtung Weiß aufgehellt – so bleibt der
/// Farbton erhalten (z. B. Link-Blau wird helleres Blau statt reinem Weiß) und
/// der Kontrast ist auf dunklen wie hellen Hintergründen zumindest grob richtig.
/// Reines Schwarz/sehr dunkle Farben werden am stärksten angehoben, damit ein
/// sichtbarer Unterschied zu nicht-fettem Text entsteht.
fn brighten(c: Color32) -> Color32 {
    // Einfache, portable Mischung Richtung Weiß (kein Gamma-korrektes
    // Color-Management nötig – reicht für einen UI-Fallback).
    let mix = |v: u8| -> u8 { (v as f32 + (255.0 - v as f32) * 0.55).round() as u8 };
    Color32::from_rgba_unmultiplied(mix(c.r()), mix(c.g()), mix(c.b()), c.a())
}

/// Erzeugt einen LayoutJob aus Inline-Spans. Gibt auch die Linkbereiche (Zeichenindex) zurück.
pub fn inline_job(theme: &Theme, spans: &[Span], size: f32, color: Color32, all_bold: bool, ui: &Ui) -> (LayoutJob, Vec<(std::ops::Range<usize>, String)>) {
    let mut job = LayoutJob::default();
    let mut links = Vec::new();
    let mut chars = 0usize;
    let bold_family = FontFamily::Name(BOLD_FAMILY.into());
    let has_bold = ui.fonts(|f| f.families().contains(&bold_family));
    for s in spans {
        let st = &s.style;
        let family = if st.code {
            FontFamily::Monospace
        } else if (st.bold || all_bold) && has_bold {
            bold_family.clone()
        } else {
            FontFamily::Proportional
        };
        let fsize = if st.code { (size * theme.code_scale).round() } else { size };
        let mut fmt = TextFormat { font_id: FontId::new(fsize, family), color, italics: st.italic, ..Default::default() };
        if st.code {
            fmt.background = theme.code_bg;
            fmt.expand_bg = 2.0;
        }
        if st.strike {
            fmt.strikethrough = Stroke::new(1.0, color);
        }
        if (st.bold || all_bold) && !has_bold {
            fmt.color = brighten(color);
        }
        let n = s.text.chars().count();
        if let Some(url) = &st.link {
            fmt.color = theme.link;
            links.push((chars..chars + n, url.clone()));
        }
        // Inline-Code mit schmalem Innenabstand (wie `padding:1px 5px`)
        if st.code {
            job.append("\u{2009}", 0.0, TextFormat { font_id: FontId::new(fsize, FontFamily::Monospace), background: theme.code_bg, ..Default::default() });
            chars += 1;
            if let Some(l) = links.last_mut().filter(|_| st.link.is_some()) {
                l.0 = l.0.start + 1..l.0.end + 1;
            }
        }
        job.append(&s.text, 0.0, fmt.clone());
        chars += n;
        if st.code {
            job.append("\u{2009}", 0.0, TextFormat { font_id: FontId::new(fsize, FontFamily::Monospace), background: theme.code_bg, ..Default::default() });
            chars += 1;
        }
    }
    job.wrap.max_width = ui.available_width();
    (job, links)
}

fn inline_label(ui: &mut Ui, theme: &Theme, spans: &[Span], size: f32, color: Color32, bold: bool, out: &mut Output) {
    let (job, links) = inline_job(theme, spans, size, color, bold, ui);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let resp = ui.add(egui::Label::new(galley.clone()).selectable(true));
    if links.is_empty() {
        return;
    }
    // Link unter dem Mauszeiger bestimmen
    let Some(pos) = resp.hover_pos() else { return };
    let local = pos - resp.rect.min;
    let cur = galley.cursor_from_pos(local);
    let idx = cur.index.0;
    // Nur echte Treffer (Cursor-Zeichen liegt innerhalb der Glyphe, nicht hinter der Zeile)
    let hit = links.iter().find(|(r, _)| {
        if !(r.start <= idx && idx <= r.end) {
            return false;
        }
        let a = galley.pos_from_cursor(egui::text::CCursor::new(r.start));
        let b = galley.pos_from_cursor(egui::text::CCursor::new(r.end));
        let within_row = local.y >= a.min.y.min(b.min.y) && local.y <= a.max.y.max(b.max.y);
        within_row && (a.min.y != b.min.y || (local.x >= a.min.x && local.x <= b.max.x))
    });
    if let Some((_, url)) = hit {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        let resp = resp.on_hover_text_at_pointer(url.as_str());
        // Klick ohne Ziehen = Link öffnen (Ziehen bleibt Textauswahl)
        if resp.clicked() {
            out.clicked_link = Some(url.clone());
        }
    }
}

/// Codeblock mit Rahmen, Sprache und Kopier-Knopf.
pub fn code_block(ui: &mut Ui, theme: &Theme, lang: &str, text: &str) {
    let size = theme.code_size();
    egui::Frame::NONE.fill(theme.code_bg).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 20.0), Sense::click());
                let response = response.on_hover_text("Code / Terminalbefehl kopieren");
                let color = if response.hovered() { theme.fg } else { theme.dim };
                let stroke = egui::Stroke::new(1.1, color);
                let center = rect.center();
                ui.painter().rect_stroke(egui::Rect::from_min_size(center + egui::vec2(-5.0, -6.0), egui::vec2(7.0, 9.0)), 1, stroke, egui::StrokeKind::Inside);
                ui.painter().rect_stroke(egui::Rect::from_min_size(center + egui::vec2(-2.0, -3.0), egui::vec2(7.0, 9.0)), 1, stroke, egui::StrokeKind::Inside);
                if response.clicked() {
                    ui.ctx().copy_text(text.to_string());
                }
                if !lang.is_empty() { ui.label(egui::RichText::new(lang).size(11.0).color(theme.dim)); }
            });
        });
        let job = code_job(lang, text, size);
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        let w = galley.size().x;
        egui::ScrollArea::horizontal().id_salt("code").auto_shrink([false, true]).show(ui, |ui| {
            ui.add(egui::Label::new(galley).selectable(true).wrap_mode(egui::TextWrapMode::Extend));
            ui.set_min_width(w);
        });
    });
}

/// LayoutJob für hervorgehobenen Code.
pub fn code_job(lang: &str, text: &str, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(size);
    for (r, t) in highlight(lang, text) {
        let mut fmt = TextFormat { font_id: font.clone(), color: token_color(t), ..Default::default() };
        if t == Tok::Comment {
            fmt.italics = true;
        }
        if t == Tok::Added {
            fmt.background = Color32::from_rgb(0x03, 0x3a, 0x16);
        }
        if t == Tok::Removed {
            fmt.background = Color32::from_rgb(0x67, 0x06, 0x0c);
        }
        job.append(&text[r], 0.0, fmt);
    }
    job.wrap.max_width = f32::INFINITY;
    job
}

fn table(ui: &mut Ui, theme: &Theme, align: &[Align], head: &[md_core::Inlines], rows: &[Vec<md_core::Inlines>], out: &mut Output) {
    let cols = head.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    egui::ScrollArea::horizontal().id_salt("table").auto_shrink([false, true]).show(ui, |ui| {
        egui::Frame::NONE.stroke(Stroke::new(1.0, theme.border)).show(ui, |ui| {
            egui::Grid::new("grid").striped(false).spacing(egui::vec2(16.0, 6.0)).min_col_width(24.0).show(ui, |ui| {
                let cell = |ui: &mut Ui, s: &[Span], bold: bool, a: Align, out: &mut Output| {
                    let layout = match a {
                        Align::Right => egui::Layout::right_to_left(egui::Align::Center),
                        Align::Center => egui::Layout::top_down(egui::Align::Center),
                        _ => egui::Layout::left_to_right(egui::Align::Center),
                    };
                    ui.with_layout(layout, |ui| {
                        ui.set_max_width(theme.size * 28.0);
                        inline_label(ui, theme, s, theme.size, theme.fg, bold, out);
                    });
                };
                for c in 0..cols {
                    let a = align.get(c).copied().unwrap_or(Align::None);
                    let empty = Vec::new();
                    ui.push_id(("h", c), |ui| cell(ui, head.get(c).unwrap_or(&empty), true, a, out));
                }
                ui.end_row();
                for (ri, r) in rows.iter().enumerate() {
                    for c in 0..cols {
                        let a = align.get(c).copied().unwrap_or(Align::None);
                        let empty = Vec::new();
                        ui.push_id((ri, c), |ui| cell(ui, r.get(c).unwrap_or(&empty), false, a, out));
                    }
                    ui.end_row();
                }
            });
        });
    });
}

/// Zwischenspeicher für geparste Dokumente (z. B. eine Nachricht je Eintrag).
///
/// Wird z. B. pro Chat-Nachricht einmal gehalten und bei jedem Redraw mit
/// [`DocCache::get`] abgefragt. Neu geparst wird nur, wenn sich der Quelltext
/// tatsächlich geändert hat – unverändertes Material (der häufigste Fall: eine
/// bereits fertige Nachricht wird jeden Frame erneut gezeichnet) kostet dann
/// nur noch einen Längenvergleich plus – falls die Länge zufällig gleich blieb –
/// einen einzigen Byte-Vergleich, statt bei jedem Frame den kompletten Text neu
/// zu hashen und zu parsen.
///
/// Bewusst **kein** Hashing: Ein eigener Hash wäre mit Kollisionsrisiko
/// behaftet und spart gegenüber einem (von `memcmp` beschleunigten)
/// direkten Gleichheitsvergleich nichts ein. Stattdessen:
/// 1. Längen unterschiedlich? → garantiert geändert, sofort neu parsen
///    (kein Vergleich nötig – das deckt den häufigsten Fall beim Streamen
///    ab, wo Text per `push_str` wächst).
/// 2. Länge gleich? → vollständiger Gleichheitsvergleich (für den seltenen
///    Fall einer Bearbeitung an gleicher Länge, z. B. nachträgliche
///    Korrektur). Nur bei tatsächlichem Unterschied wird neu geparst.
#[derive(Default)]
pub struct DocCache {
    src: String,
    doc: Doc,
    /// `false` vor dem ersten `get`-Aufruf – erzwingt den initialen Parse
    /// auch dann, wenn der erste `src` zufällig leer ist (sonst wäre er von
    /// der ebenfalls leeren Default-`src` nicht zu unterscheiden).
    parsed: bool,
    /// Zähler erfolgter Neu-Parses – nur für Tests/Diagnose gedacht.
    parses: u64,
}

impl DocCache {
    /// Aktuelles Dokument liefern, bei Bedarf neu parsen.
    ///
    /// Günstig für wiederholte Aufrufe mit unverändertem `src` (z. B. einmal
    /// pro UI-Frame für dieselbe Nachricht).
    pub fn get(&mut self, src: &str) -> &Doc {
        if !self.parsed || self.src.len() != src.len() || self.src != src {
            self.doc = md_core::parse(src);
            self.src.clear();
            self.src.push_str(src);
            self.parsed = true;
            self.parses += 1;
        }
        &self.doc
    }

    /// Anzahl bisher durchgeführter Neu-Parses (für Tests/Diagnose).
    pub fn parses(&self) -> u64 {
        self.parses
    }
}

#[cfg(test)]
mod doc_cache_tests {
    use super::*;

    #[test]
    fn unveraenderter_text_parst_nicht_erneut() {
        let mut c = DocCache::default();
        c.get("# Hallo");
        assert_eq!(c.parses(), 1);
        c.get("# Hallo");
        c.get("# Hallo");
        assert_eq!(c.parses(), 1, "gleicher Text darf nicht erneut geparst werden");
    }

    #[test]
    fn wachsender_text_parst_bei_jeder_aenderung() {
        let mut c = DocCache::default();
        c.get("Hallo");
        c.get("Hallo W");
        c.get("Hallo Welt");
        assert_eq!(c.parses(), 3);
        assert_eq!(c.get("Hallo Welt").blocks.len(), 1);
    }

    #[test]
    fn gleich_lange_bearbeitung_wird_erkannt() {
        // Gleiche Länge, aber anderer Inhalt (z. B. nachträgliche Korrektur) –
        // muss trotzdem als Änderung erkannt werden (keine falsche Cache-Treffer).
        let mut c = DocCache::default();
        c.get("# Hallo");
        assert_eq!(c.parses(), 1);
        c.get("# Hillo");
        assert_eq!(c.parses(), 2, "inhaltliche Änderung bei gleicher Länge muss erkannt werden");
        let md_core::Block::Heading { text, .. } = &c.get("# Hillo").blocks[0] else { panic!() };
        assert_eq!(md_core::inline_text(text), "Hillo");
    }

    #[test]
    fn leerer_text() {
        let mut c = DocCache::default();
        assert!(c.get("").blocks.is_empty());
        assert_eq!(c.parses(), 1);
        c.get("");
        assert_eq!(c.parses(), 1);
    }
}
