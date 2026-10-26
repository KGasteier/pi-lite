//! Farben und Maße wie in pi-lite (ui/index.html).
use egui::Color32;

pub const BG: Color32 = Color32::from_rgb(0x1e, 0x22, 0x27);
pub const PANEL: Color32 = Color32::from_rgb(0x26, 0x2b, 0x30);
pub const FG: Color32 = Color32::from_rgb(0xe6, 0xed, 0xf3);
pub const DIM: Color32 = Color32::from_rgb(0x8b, 0x94, 0x9e);
pub const ACC: Color32 = Color32::from_rgb(0xe4, 0x54, 0xc4);
pub const ERR: Color32 = Color32::from_rgb(0xef, 0x47, 0x6f);
pub const OK: Color32 = Color32::from_rgb(0x18, 0xa9, 0x99);
pub const WARN: Color32 = Color32::from_rgb(0xe0, 0xa0, 0x30);
pub const BORDER: Color32 = Color32::from_rgb(0x3a, 0x40, 0x46);
pub const TOOL_BG: Color32 = Color32::from_rgb(0x1a, 0x1f, 0x24);
pub const KBD_BG: Color32 = Color32::from_rgb(0x32, 0x39, 0x40);
pub const BTN: Color32 = Color32::from_rgb(0x44, 0x4a, 0x52);
pub const SELECT_HOVER: Color32 = Color32::from_rgb(0x66, 0x7b, 0x96);
pub const PICK_OPEN: Color32 = Color32::from_rgb(0x52, 0x67, 0x81);
pub const CARD: Color32 = Color32::from_rgb(0x2b, 0x31, 0x37);
pub const CAT_SESSIONS: Color32 = Color32::from_rgb(0x6a, 0xa5, 0xe8);
pub const CAT_EXT: Color32 = Color32::from_rgb(0x5e, 0xc2, 0xa0);
pub const CAT_SKILLS: Color32 = Color32::from_rgb(0xe0, 0xb2, 0x5a);
pub const CAT_AGENTS: Color32 = Color32::from_rgb(0xb0, 0x8c, 0xf0);
pub const CAT_HARNESS: Color32 = Color32::from_rgb(0xe6, 0x8f, 0x6e);

/// `color-mix(in srgb, a t, b)` – Anteil `t` von `a`.
pub fn mix(a: Color32, t: f32, b: Color32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 * t + y as f32 * (1.0 - t)).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// Farbe mit Deckkraft `a` (0..1).
pub fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0).round() as u8)
}

/// `filter: brightness(f)` für Hover-Zustände.
pub fn brighten(c: Color32, f: f32) -> Color32 {
    let g = |x: u8| ((x as f32) * f).min(255.0) as u8;
    Color32::from_rgba_premultiplied(g(c.r()), g(c.g()), g(c.b()), c.a())
}

/// Remote-Sessions (Server-Backends): Rand, Streifen und Badge der Karten.
pub const REMOTE: Color32 = Color32::from_rgb(0x4f, 0xb8, 0xd8);

/// Kategorie-Farbe (Seitenleiste).
pub const CAT: Color32 = CAT_SESSIONS;
