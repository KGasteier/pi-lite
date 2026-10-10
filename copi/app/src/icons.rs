//! Vektor-Icons wie die SVGs in pi-lite (viewBox 16×16), direkt gemalt.
use crate::theme::*;
use egui::{epaint::PathShape, pos2, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};

pub fn lock(p: &Painter, rect: Rect, color: Color32, open: bool) {
    let m = Map::new(rect, 16.0, 16.0);
    p.rect_stroke(Rect::from_min_max(m.p(3.0, 7.0), m.p(13.0, 15.0)), 2, Stroke::new(1.4, color), egui::StrokeKind::Inside);
    let x = if open { 10.0 } else { 4.5 };
    line(p, &m, &[(x,7.0),(x,4.0),(x+1.0,2.0),(x+4.0,2.0),(x+5.0,4.0),(x+5.0,7.0)], color, 1.4);
    line(p, &m, &[(8.0,10.0),(8.0,12.0)], color, 1.4);
}

/// Abbildung von viewBox-Koordinaten in ein Rechteck.
pub struct Map {
    rect: Rect,
    w: f32,
    h: f32,
}

impl Map {
    pub fn new(rect: Rect, w: f32, h: f32) -> Self {
        Self { rect, w, h }
    }
    pub fn p(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.rect.left() + x / self.w * self.rect.width(), self.rect.top() + y / self.h * self.rect.height())
    }
    pub fn s(&self, v: f32) -> f32 {
        v / self.w * self.rect.width()
    }
}

pub fn line(p: &Painter, m: &Map, pts: &[(f32, f32)], color: Color32, width: f32) {
    let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| m.p(x, y)).collect();
    p.add(Shape::line(pts, Stroke::new(m.s(width).max(1.0), color)));
}

pub fn closed(p: &Painter, m: &Map, pts: &[(f32, f32)], color: Color32, width: f32) {
    let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| m.p(x, y)).collect();
    p.add(PathShape::closed_line(pts, Stroke::new(m.s(width).max(1.0), color)));
}

/// Fächer-Füllung ab Punkt `fan` (für einfache konkave Formen wie Lesezeichen).
pub fn fill_fan(p: &Painter, m: &Map, fan: (f32, f32), pts: &[(f32, f32)], color: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(m.p(fan.0, fan.1), color);
    for &(x, y) in pts {
        mesh.colored_vertex(m.p(x, y), color);
    }
    for i in 1..pts.len() as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    p.add(Shape::mesh(mesh));
}

/// Kreisbogen als Polylinie: Sehne senkrecht bei `x` von `y0` bis `y1`, nach rechts gewölbt.
fn arc(x: f32, y0: f32, y1: f32, r: f32) -> Vec<(f32, f32)> {
    let h = (y1 - y0) / 2.0;
    let cy = y0 + h;
    let cx = x - (r * r - h * h).max(0.0).sqrt();
    let a = (h / r).asin();
    (0..=12).map(|i| {
        let t = -a + 2.0 * a * i as f32 / 12.0;
        (cx + r * t.cos(), cy + r * t.sin())
    }).collect()
}

pub fn plus(p: &Painter, r: Rect, c: Color32) {
    let m = Map::new(r, 16.0, 16.0);
    line(p, &m, &[(8.0, 3.0), (8.0, 13.0)], c, 1.6);
    line(p, &m, &[(3.0, 8.0), (13.0, 8.0)], c, 1.6);
}
pub fn up(p: &Painter, r: Rect, c: Color32) {
    line(p, &Map::new(r, 16.0, 16.0), &[(4.0, 10.0), (8.0, 6.0), (12.0, 10.0)], c, 1.6);
}
pub fn down(p: &Painter, r: Rect, c: Color32) {
    line(p, &Map::new(r, 16.0, 16.0), &[(4.0, 6.0), (8.0, 10.0), (12.0, 6.0)], c, 1.6);
}
pub fn expand(p: &Painter, r: Rect, c: Color32) {
    let m = Map::new(r, 16.0, 16.0);
    line(p, &m, &[(9.0, 3.0), (13.0, 3.0), (13.0, 7.0)], c, 1.6);
    line(p, &m, &[(13.0, 3.0), (9.0, 7.0)], c, 1.6);
    line(p, &m, &[(7.0, 13.0), (3.0, 13.0), (3.0, 9.0)], c, 1.6);
    line(p, &m, &[(3.0, 13.0), (7.0, 9.0)], c, 1.6);
}
pub fn external(p: &Painter, r: Rect, c: Color32) {
    let m = Map::new(r, 16.0, 16.0);
    line(p, &m, &[(9.0, 2.0), (14.0, 2.0), (14.0, 7.0)], c, 1.5);
    line(p, &m, &[(14.0, 2.0), (7.0, 9.0)], c, 1.5);
    line(p, &m, &[(6.0, 3.0), (2.0, 3.0), (2.0, 14.0), (13.0, 14.0), (13.0, 10.0)], c, 1.5);
}
/// Lesezeichen des Filterknopfs (viewBox 16×16).
pub fn bookmark_small(p: &Painter, r: Rect, c: Color32, filled: bool) {
    let m = Map::new(r, 16.0, 16.0);
    let pts = [(4.5, 2.5), (11.5, 2.5), (11.5, 13.5), (8.0, 11.0), (4.5, 13.5)];
    if filled {
        fill_fan(p, &m, (8.0, 11.0), &[(11.5, 13.5), (11.5, 2.5), (4.5, 2.5), (4.5, 13.5)], c);
    }
    closed(p, &m, &pts, c, 1.6);
}
/// Lesezeichen auf Session-Karten (viewBox 16×20).
pub fn bookmark_card(p: &Painter, r: Rect, c: Color32, filled: bool) {
    let m = Map::new(r, 16.0, 20.0);
    if filled {
        fill_fan(p, &m, (8.0, 14.0), &[(13.0, 18.0), (13.0, 2.0), (3.0, 2.0), (3.0, 18.0)], c);
    }
    closed(p, &m, &[(3.0, 2.0), (13.0, 2.0), (13.0, 18.0), (8.0, 14.0), (3.0, 18.0)], c, 1.5);
}
pub fn pencil(p: &Painter, r: Rect, c: Color32) {
    let m = Map::new(r, 16.0, 16.0);
    closed(p, &m, &[(3.0, 13.0), (4.0, 9.5), (10.5, 3.0), (13.0, 5.5), (6.5, 12.0)], c, 1.4);
    line(p, &m, &[(9.5, 4.0), (12.0, 6.5)], c, 1.4);
}
pub fn trash(p: &Painter, r: Rect, c: Color32) {
    let m = Map::new(r, 16.0, 16.0);
    line(p, &m, &[(3.5, 4.5), (12.5, 4.5)], c, 1.4);
    line(p, &m, &[(6.5, 4.5), (6.5, 3.0), (9.5, 3.0), (9.5, 4.5)], c, 1.4);
    line(p, &m, &[(5.0, 4.5), (5.6, 13.0), (10.4, 13.0), (11.0, 4.5)], c, 1.4);
}
pub fn speaker(p: &Painter, r: Rect, c: Color32, on: bool) {
    let m = Map::new(r, 16.0, 16.0);
    closed(p, &m, &[(2.5, 6.0), (4.7, 6.0), (8.0, 3.5), (8.0, 12.5), (4.7, 10.0), (2.5, 10.0)], c, 1.6);
    if on {
        line(p, &m, &arc(10.5, 6.0, 10.0, 2.8), c, 1.6);
        line(p, &m, &arc(12.3, 4.2, 11.8, 5.4), c, 1.6);
    } else {
        line(p, &m, &[(11.0, 6.0), (14.0, 10.0)], c, 1.6);
        line(p, &m, &[(14.0, 6.0), (11.0, 10.0)], c, 1.6);
    }
}
pub fn stop(p: &Painter, r: Rect, c: Color32) {
    let m = Map::new(r, 16.0, 16.0);
    p.rect_filled(Rect::from_min_max(m.p(4.5, 4.5), m.p(11.5, 11.5)), m.s(1.0), c);
}
/// Aufklapp-Pfeil der Auswahlknöpfe (10×6, rechts 9 px).
pub fn chevron(p: &Painter, button: Rect, c: Color32) {
    let r = Rect::from_min_size(pos2(button.right() - 9.0 - 10.0, button.center().y - 3.0), Vec2::new(10.0, 6.0));
    line(p, &Map::new(r, 10.0, 6.0), &[(1.0, 1.0), (5.0, 5.0), (9.0, 1.0)], c, 1.6);
}
/// ⓘ (Glyphe fehlt in den Standardschriften).
pub fn info(p: &Painter, r: Rect, c: Color32) {
    let ctr = r.center();
    let rad = r.width().min(r.height()) / 2.0 - 1.0;
    p.circle_stroke(ctr, rad, Stroke::new(1.0, c));
    p.circle_filled(pos2(ctr.x, ctr.y - rad * 0.45), 0.9, c);
    p.line_segment([pos2(ctr.x, ctr.y - rad * 0.1), pos2(ctr.x, ctr.y + rad * 0.5)], Stroke::new(1.2, c));
}
/// pi-Logo (viewBox 12×12.5).
pub fn pi_logo(p: &Painter, r: Rect) {
    let m = Map::new(r, 12.0, 12.5);
    let rect = |x: f32, y: f32, w: f32, h: f32, c: Color32| p.rect_filled(Rect::from_min_max(m.p(x, y), m.p(x + w, y + h)), 0.0, c);
    let red = Color32::from_rgb(0xe1, 0x88, 0x78);
    let blue = Color32::from_rgb(0x4e, 0x8c, 0xb0);
    let yellow = Color32::from_rgb(0xe7, 0xb3, 0x5c);
    rect(0.0, 0.0, 9.0, 3.0, red);
    rect(6.0, 3.0, 3.0, 3.0, red);
    rect(0.0, 3.0, 3.0, 9.5, blue);
    rect(3.0, 6.0, 3.0, 3.0, blue);
    rect(9.0, 6.0, 3.0, 6.5, yellow);
}

/// Darstellung eines Icon-Knopfs.
#[derive(Clone, Copy)]
pub struct IconStyle {
    /// Hintergrund (None = transparent, Hover #526781).
    pub bg: Option<Color32>,
    pub fg: Color32,
    pub hover_fg: Color32,
    pub icon: f32,
    pub radius: u8,
}

impl Default for IconStyle {
    fn default() -> Self {
        Self { bg: Some(BTN), fg: FG, hover_fg: FG, icon: 15.0, radius: 6 }
    }
}

/// Knopf mit gemaltem Icon; `draw(painter, icon_rect, farbe)`.
pub fn icon_button(ui: &mut Ui, size: Vec2, enabled: bool, st: IconStyle, tip: &str, draw: impl FnOnce(&Painter, Rect, Color32)) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    paint_icon_button(ui, rect, &resp, enabled, st, draw);
    resp.on_hover_text(tip)
}

pub fn paint_icon_button(ui: &Ui, rect: Rect, resp: &Response, enabled: bool, st: IconStyle, draw: impl FnOnce(&Painter, Rect, Color32)) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let hovered = enabled && resp.hovered();
    let p = ui.painter();
    match st.bg {
        Some(bg) => {
            let bg = if hovered { brighten(bg, 1.35) } else { bg };
            p.rect_filled(rect, st.radius, if enabled { bg } else { alpha(bg, 0.55) });
        }
        None if hovered => {
            p.rect_filled(rect, st.radius, PICK_OPEN);
        }
        None => {}
    }
    if resp.has_focus() {
        p.rect_stroke(rect.expand(2.0), st.radius + 2, Stroke::new(2.0, ACC), egui::StrokeKind::Outside);
    }
    let fg = if hovered { st.hover_fg } else { st.fg };
    let fg = if enabled { fg } else { alpha(fg, 0.55) };
    let icon = Rect::from_center_size(rect.center(), Vec2::new(st.icon, st.icon));
    draw(p, icon, fg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}
