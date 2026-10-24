//! Darstellung (entspricht index.html/style.css in pi-lite).
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    pos2, vec2, Align, Align2, Color32, Context, CornerRadius, Event, FontId, Frame, Id, Key, Label, Layout, Margin, Modifiers, Order, Pos2, Rect, Response, RichText, ScrollArea, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};
use serde_json::{json, Value};
use std::time::Duration;

use crate::app::*;
use crate::commands::*;
use crate::icons::{self, IconStyle};
use crate::run_activity;
use crate::theme::*;
use crate::virtual_log;

type Act = Box<dyn FnOnce(&mut App)>;

const KEYHELP: [&str; 5] = [
    "Tab / Shift+Tab: nächstes / vorheriges Bedienelement · Enter/Leertaste: aktivieren · Esc: schließen/zurück",
    "Alt+1: Eingabe · Alt+2: Seitenliste · Alt+3: Chat · Alt+4: Kopfzeile · Alt+5: Vorschau\nAlt+N: neue Session · Alt+A: abbrechen · Alt+F: Fork · Alt+M: Modell · Alt+T: Thinking\nAlt+− / Alt++: Ausgabeschrift · F1: diese Hilfe · Strg/Cmd+B: Sessions · Strg/Cmd+Shift+B: Netz-Seitenleiste",
    "Listen: ↑/↓, Home/End · ←: Ansichtsauswahl · →: Vorschau\nSessions: Enter öffnen · R/F2 umbenennen · Backspace löschen · M/Shift+F10 Menü\nChat: ↑/↓ scrollen · Home/End Anfang/Ende · Tab erreicht Links und Tool-Blöcke",
    "Eingabe: ↑/↓ an erster/letzter Zeile: History; sonst Cursor · Alt+↑/↓ oder Pfeilknöpfe: History\nEnter sendet · Option/Alt+Enter: neue Zeile · Editor: Strg/Cmd+S speichert\nEsc unterbricht den Agenten · !cmd Shell (!!cmd ohne Kontext) · pi update u. a. pi-CLI-Befehle direkt\n/ öffnet Befehle · Tippen filtert Namen · Hover/↑↓ markieren · Klick/Enter ausführen · Tab vervollständigen · Esc schließen",
    "Dialoge: Tab bleibt im Dialog; Pfeile wechseln zwischen Knöpfen, nicht innerhalb von Textfeldern.",
];
/// Texte der Netz-Seitenleiste (identisch in ui/index.html, #netpanel).
pub const NET_PLACEHOLDER: [&str; 3] = [
    "Hier entsteht der Chat mit Menschen: Kurs, Projekt oder Direktnachricht.",
    "Noch ohne Funktion – es wird nichts gesendet, empfangen oder gespeichert.",
    "Geplant: Präsenz, Dateien, Code und Prompts als Karten. Prompts landen nur nach ausdrücklicher Übernahme in der Eingabe.",
];
const PLACEHOLDER: &str = "Nachricht, /Befehl, !Shell oder pi update … (Enter = senden, Alt+Enter = Zeilenumbruch)";

fn prompt_id() -> Id {
    Id::new("copi-prompt")
}

/// Nimmt ein Tastenereignis aus der Warteschlange (plus das Text-Echo bei Alt/Leertaste).
fn take(ctx: &Context, f: impl Fn(Key, &Modifiers) -> bool) -> bool {
    ctx.input_mut(|i| {
        let Some(p) = i.events.iter().position(|e| matches!(e, Event::Key { key, pressed: true, modifiers, .. } if f(*key, modifiers))) else {
            return false;
        };
        let ev = i.events.remove(p);
        let swallow = matches!(ev, Event::Key { key, modifiers, .. } if modifiers.alt || key == Key::Space);
        if swallow && matches!(i.events.get(p), Some(Event::Text(_))) {
            i.events.remove(p);
        }
        true
    })
}

/// Exakter Modifikator-Vergleich (alt, strg/cmd, shift). Der `ctrl`-Parameter bedeutet
/// „Strg bzw. Cmd gedrückt“ und vergleicht gegen `Modifiers::command`, das egui unter Windows/
/// Linux auf `ctrl` und unter macOS auf `mac_cmd` (⌘) setzt – dadurch funktionieren dieselben
/// Tastenkürzel (z. B. Strg/Cmd+S) auf allen drei Plattformen ohne separate Mac-Zweige.
fn k(ctx: &Context, key: Key, alt: bool, ctrl: bool, shift: bool) -> bool {
    take(ctx, |kk, m| kk == key && m.alt == alt && m.command == ctrl && m.shift == shift)
}

fn plain(ctx: &Context, key: Key) -> bool {
    k(ctx, key, false, false, false)
}

pub fn txt(s: impl Into<String>, size: f32, c: Color32) -> RichText {
    RichText::new(s.into()).size(size).color(c)
}

/// Gemalter Knopf (CSS `button`): bg #444a52, Radius 6, Hover #555c66; primär = Akzent mit weißer Schrift.
pub fn btn(ui: &mut Ui, text: &str, size: f32, pad: Vec2, min: Vec2, primary: bool, enabled: bool) -> Response {
    let fg = if primary { Color32::WHITE } else { FG };
    let family = if primary { egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()) } else { egui::FontFamily::Proportional };
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::new(size, family), fg);
    let want = vec2((galley.size().x + 2.0 * pad.x).max(min.x), (galley.size().y + 2.0 * pad.y).max(min.y));
    let (rect, resp) = ui.allocate_exact_size(want, if enabled { Sense::click() } else { Sense::hover() });
    if ui.is_rect_visible(rect) {
        let base = if primary { ACC } else { BTN };
        let hov = enabled && resp.hovered();
        let bg = if hov {
            if primary {
                brighten(ACC, 1.12)
            } else {
                Color32::from_rgb(0x55, 0x5c, 0x66)
            }
        } else {
            base
        };
        let p = ui.painter();
        p.rect_filled(rect, 6, if enabled { bg } else { alpha(bg, 0.55) });
        if resp.has_focus() {
            p.rect_stroke(rect.expand(2.0), 8, Stroke::new(2.0, ACC), StrokeKind::Outside);
        }
        let pos = rect.center() - galley.size() / 2.0;
        p.galley_with_override_text_color(pos, galley, if enabled { fg } else { alpha(fg, 0.55) });
        if hov {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    resp
}

/// Pixel-Spinner: 2×4 Punkte, 3 px, Abstand 1,5 px, gestaffelt.
fn spinner(ui: &mut Ui) {
    snake(ui, false);
}

/// Pixel-Schlange; `mirror` lässt sie gespiegelt (gegenläufig) laufen – so flankieren
/// zwei Schlangen den Status-Chip symmetrisch.
fn snake(ui: &mut Ui, mirror: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(7.5, 16.5), Sense::hover());
    let t = ui.ctx().input(|i| i.time) as f32;
    let order = [0, 1, 3, 5, 7, 6, 4, 2];
    for (n, &cell) in order.iter().enumerate() {
        let cx = if mirror { 1 - cell % 2 } else { cell % 2 } as f32;
        let cy = (cell / 2) as f32;
        let phase = ((t - n as f32 * 0.1).rem_euclid(0.8)) / 0.8;
        let a = if phase < 0.375 { 1.0 } else { 0.18 };
        let r = Rect::from_min_size(rect.min + vec2(cx * 4.5, cy * 4.5), vec2(3.0, 3.0));
        ui.painter().rect_filled(r, 0, alpha(ACC, a));
    }
}

fn tri(p: &egui::Painter, c: Pos2, open: bool, col: Color32) {
    let pts = if open {
        vec![c + vec2(-4.0, -2.0), c + vec2(4.0, -2.0), c + vec2(0.0, 3.0)]
    } else {
        vec![c + vec2(-2.0, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.0, 4.0)]
    };
    p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
}

impl App {
    pub fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.pump();
        self.keys(&ctx);

        if self.store.side_visible {
            egui::Panel::left("side")
                .exact_size(240.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::new().fill(PANEL).inner_margin(Margin { left: 8, right: 8, top: 10, bottom: 8 }).stroke(Stroke::new(1.0, BORDER)))
                .show(ui, |ui| self.sidebar(ui));
        }
        egui::Panel::top("head")
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(12, 8)))
            .show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().hline(r.x_range(), r.bottom() + 7.0, Stroke::new(1.0, BORDER));
                self.header(ui);
            });
        egui::Panel::bottom("usage")
            .exact_size(30.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(12, 4)))
            .show(ui, |ui| self.usage_bar(ui));
        let t = self.ext_status.iter().filter(|(key, _)| key != "vorlesen").map(|(_, v)| v.as_str()).collect::<Vec<_>>().join(" · ");
        if !t.is_empty() {
            egui::Panel::bottom("extstatus")
                .show_separator_line(false)
                .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(12, 3)))
                .show(ui, |ui| {
                    ui.add(Label::new(txt(t, 12.0, DIM)).truncate());
                });
        }
        self.widget_panel(ui, "belowEditor");
        if !self.material.items.is_empty() || !self.material.status.is_empty() {
            egui::Panel::bottom("material")
                .show_separator_line(false)
                .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(12, 6)))
                .show(ui, |ui| self.material_bar(ui));
        }
        egui::Panel::bottom("footer")
            .exact_size(92.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(12, 8)))
            .show(ui, |ui| self.footer(ui));
        egui::Panel::bottom("statusline")
            .exact_size(30.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(12, 0)))
            .show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, BORDER));
                self.status_line(ui);
            });
        self.widget_panel(ui, "aboveEditor");
        if self.store.net_visible {
            // Unterhalb der Kopfzeile, oberhalb der Eingabezone; schmale Fenster behalten
            // mindestens ~60 % für den Agentenchat.
            let free = ui.available_width();
            egui::Panel::right("net")
                .exact_size(320.0f32.min(free * 0.4).max(200.0))
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::new().fill(PANEL).inner_margin(Margin::same(12)))
                .show(ui, |ui| {
                    let r = ui.max_rect().expand(12.0);
                    ui.painter().vline(r.left(), r.y_range(), Stroke::new(1.0, BORDER));
                    self.net_bar(ui);
                    #[cfg(feature = "cocopi")]
                    {
                        ui.separator();
                        self.collaboration_panel(ui);
                    }
                });
        }
        if self.store.net_visible && self.net.grid {
            egui::CentralPanel::default().frame(Frame::new().fill(BG)).show(ui, |ui| self.net_grid(ui));
        } else {
            egui::CentralPanel::default().frame(Frame::new().fill(BG)).show(ui, |ui| self.log(ui));
        }
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let over_preview = pointer.is_some_and(|p| self.preview_rect.contains(p) || self.preview_anchor.contains(p));
        if over_preview { self.preview_hover_until = std::time::Instant::now() + Duration::from_millis(300); }
        if self.preview.is_some() && !over_preview && self.dialogs.is_empty() && !self.editor_open && std::time::Instant::now() > self.preview_hover_until {
            self.hide_preview();
        }
        if self.preview.is_some() {
            let preview_area = egui::Area::new(Id::new("floating-preview"))
                .order(Order::Foreground).fade_in(false)
                .fixed_pos(Pos2::new(if self.preview_anchor.is_finite() { self.preview_anchor.right() + 10.0 } else { 248.0 }, if self.preview_anchor.is_finite() { self.preview_anchor.top() } else { 64.0 }))
                .constrain(true)
                .show(&ctx, |ui| {
                    Frame::new().fill(PANEL).corner_radius(12)
                        .stroke(Stroke::new(1.0, match self.side_view { 1 => CAT_EXT, 2 => CAT_SKILLS, 3 => CAT_AGENTS, 4 => CAT_HARNESS, _ => CAT })).inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(350.0f32.min(ctx.content_rect().width() - 40.0));
                            ui.set_max_height((ctx.content_rect().height() - 150.0).max(150.0));
                            self.preview_panel(ui);
                        });
                });
            self.preview_rect = preview_area.response.rect;
            ctx.request_repaint_after(Duration::from_millis(300));
        }

        self.picker_overlay(&ctx);
        self.slash_overlay(&ctx);
        self.dialog_overlay(&ctx);
        self.net_overlays(&ctx);
        self.keyhelp_overlay(&ctx);
        self.editor_overlay(&ctx);
        self.list_menu_overlay(&ctx);
        self.toast_overlay(&ctx);

        self.drain();
        if self.title != self.sent_title {
            self.sent_title = self.title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title.clone()));
        }
        if self.busy || self.active_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn widget_panel(&mut self, ui: &mut Ui, place: &str) {
        let lines: Vec<String> = self.widgets.iter().filter(|w| w.2 == place && w.0 != "vorlesen").flat_map(|w| w.1.clone()).collect();
        if lines.is_empty() {
            return;
        }
        egui::Panel::bottom(Id::new(("widgets", place.to_string())))
            .show_separator_line(false)
            .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(12, 4)))
            .show(ui, |ui| {
                for l in lines {
                    ui.add(Label::new(txt(strip_ansi(&l), 12.0, DIM)).truncate());
                }
            });
    }

    // ---------- Tastatur ----------

    fn keys(&mut self, ctx: &Context) {
        // 0. Dialoge der Backend-Leiste (eigene Modale, Tasten dort)
        if self.net.form.is_some() || self.net.new_inst.is_some() {
            ctx.memory_mut(|m| m.surrender_focus(prompt_id()));
            if plain(ctx, Key::Escape) {
                self.net.form = None;
                self.net.new_inst = None;
                self.focus_prompt = true;
            }
            return;
        }
        // 1. Dialoge
        if let Some(kind) = self.dialogs.front().map(|d| d.kind) {
            ctx.memory_mut(|m| m.surrender_focus(prompt_id()));
            if plain(ctx, Key::Escape) {
                self.close_dialog(Some(json!({"cancelled": true})));
                return;
            }
            let Some(d) = self.dialogs.front_mut() else { return };
            match kind {
                DialogKind::Select => {
                    let n = d.options.len();
                    let mut moved = true;
                    if plain(ctx, Key::ArrowDown) {
                        d.cursor = (d.cursor + 1).min(n.saturating_sub(1));
                    } else if plain(ctx, Key::ArrowUp) {
                        d.cursor = d.cursor.saturating_sub(1);
                    } else if plain(ctx, Key::Home) {
                        d.cursor = 0;
                    } else if plain(ctx, Key::End) {
                        d.cursor = n.saturating_sub(1);
                    } else if plain(ctx, Key::PageDown) {
                        d.cursor = (d.cursor + 8).min(n.saturating_sub(1));
                    } else if plain(ctx, Key::PageUp) {
                        d.cursor = d.cursor.saturating_sub(8);
                    } else {
                        moved = false;
                    }
                    if moved {
                        d.scroll = true;
                    }
                    if plain(ctx, Key::Enter) {
                        if let Some(o) = d.options.get(d.cursor).cloned() {
                            self.close_dialog(Some(json!({"value": o})));
                        }
                    }
                }
                DialogKind::Confirm => {
                    if plain(ctx, Key::ArrowLeft) || plain(ctx, Key::ArrowRight) || plain(ctx, Key::ArrowUp) || plain(ctx, Key::ArrowDown) {
                        d.cursor = 1 - d.cursor.min(1);
                    }
                    if plain(ctx, Key::Enter) {
                        let confirmed = d.cursor == 0;
                        self.close_dialog(Some(json!({"confirmed": confirmed})));
                    }
                }
                DialogKind::Input => {
                    if plain(ctx, Key::Enter) {
                        let v = d.text.clone();
                        self.close_dialog(Some(json!({"value": v})));
                    }
                }
                DialogKind::Editor => {
                    if k(ctx, Key::Enter, false, true, false) {
                        let v = d.text.clone();
                        self.close_dialog(Some(json!({"value": v})));
                    }
                }
                DialogKind::Folder => {
                    let n = d.folders.len();
                    let mut moved = true;
                    if plain(ctx, Key::ArrowDown) {
                        d.cursor = (d.cursor + 1).min(n.saturating_sub(1));
                    } else if plain(ctx, Key::ArrowUp) {
                        d.cursor = d.cursor.saturating_sub(1);
                    } else if plain(ctx, Key::Home) {
                        d.cursor = 0;
                    } else if plain(ctx, Key::End) {
                        d.cursor = n.saturating_sub(1);
                    } else {
                        moved = false;
                    }
                    if moved {
                        d.scroll = true;
                    }
                    if k(ctx, Key::Enter, false, true, false) {
                        // Strg+Enter: aktuellen Pfad bestätigen.
                        let v = d.text.clone();
                        self.close_dialog(Some(json!({"path": v})));
                    } else if plain(ctx, Key::Enter) {
                        // Enter: zum ausgewählten Unterordner wechseln (oder, falls keine
                        // Auswahl, den eingegebenen Pfad neu durchsuchen).
                        if let Some(l) = d.folders.get(d.cursor).cloned() {
                            let p = l.path;
                            self.browse_folder(p);
                        } else {
                            let p = d.text.clone();
                            self.browse_folder(p);
                        }
                    } else if k(ctx, Key::Backspace, false, false, false) {
                        if let Some(parent) = d.parent.clone() {
                            self.browse_folder(parent);
                        }
                    }
                }
                DialogKind::Material => {
                    let n = d.entries.len();
                    let mut moved = true;
                    if plain(ctx, Key::ArrowDown) {
                        d.cursor = (d.cursor + 1).min(n.saturating_sub(1));
                    } else if plain(ctx, Key::ArrowUp) {
                        d.cursor = d.cursor.saturating_sub(1);
                    } else {
                        moved = false;
                    }
                    if moved {
                        d.scroll = true;
                    }
                    if k(ctx, Key::Enter, false, true, false) {
                        // Strg+Enter: Auswahl bestätigen.
                        let v = d.selected.clone();
                        self.close_dialog(Some(json!({"paths": v})));
                    } else if plain(ctx, Key::Enter) {
                        if let Some(e) = d.entries.get(d.cursor).cloned() {
                            self.toggle_material_entry(e.path, e.is_dir);
                        }
                    } else if k(ctx, Key::Backspace, false, false, false) {
                        if let Some(parent) = d.parent.clone() {
                            self.browse_material(parent);
                        }
                    }
                }
            }
            return;
        }
        // 2. Tastenhilfe
        if self.keyhelp {
            if plain(ctx, Key::Escape) || plain(ctx, Key::F1) {
                self.keyhelp = false;
                self.focus_prompt = true;
            }
            return;
        }
        // 3. Großer Editor
        if self.editor_open {
            if plain(ctx, Key::Escape) {
                self.editor_open = false;
                self.editor_target = EditorTarget::Prompt;
                self.focus_prompt = true;
            } else if k(ctx, Key::S, false, true, false) {
                self.apply_editor();
            }
            return;
        }
        // 4. Auswahlliste
        if let Some(kind) = self.picker.as_ref().map(|p| p.kind) {
            let n = self.picker_rows(kind).0.len();
            let p = self.picker.as_mut().unwrap();
            let mut moved = true;
            if plain(ctx, Key::ArrowDown) {
                p.cursor = (p.cursor + 1).min(n.saturating_sub(1));
            } else if plain(ctx, Key::ArrowUp) {
                p.cursor = p.cursor.saturating_sub(1);
            } else if plain(ctx, Key::Home) {
                p.cursor = 0;
            } else if plain(ctx, Key::End) {
                p.cursor = n.saturating_sub(1);
            } else if plain(ctx, Key::PageDown) {
                p.cursor = (p.cursor + 8).min(n.saturating_sub(1));
            } else if plain(ctx, Key::PageUp) {
                p.cursor = p.cursor.saturating_sub(8);
            } else {
                moved = false;
            }
            if moved {
                p.scroll = true;
            }
            let cur = p.cursor;
            if plain(ctx, Key::Enter) || plain(ctx, Key::Space) {
                self.choose_pick(kind, cur);
                self.focus_prompt = true;
            } else if plain(ctx, Key::Escape) || plain(ctx, Key::Tab) {
                self.picker = None;
                self.focus_prompt = true;
            }
            return;
        }
        // 4b. Sessions-/Inventarliste in der Seitenleiste (Tastaturnavigation, nur wenn die
        // Eingabe nicht fokussiert ist und keine Auswahlliste/Hilfe offen ist).
        let prompt_focused_now = ctx.memory(|m| m.has_focus(prompt_id()));
        if self.store.side_visible && !prompt_focused_now && !ctx.memory(|m| m.focused().is_some()) && !egui::Popup::is_any_open(ctx) {
            if self.list_menu {
                if plain(ctx, Key::Escape) {
                    self.list_menu = false;
                }
                return;
            }
            let n = if self.side_view == 0 { self.visible_sessions().len() } else { self.visible_inventory().len() };
            if n > 0 {
                let mut moved = true;
                let cur = self.list_cursor.unwrap_or(0).min(n - 1);
                let mut cur = cur;
                if plain(ctx, Key::ArrowDown) {
                    cur = (cur + 1).min(n - 1);
                } else if plain(ctx, Key::ArrowUp) {
                    cur = cur.saturating_sub(1);
                } else if plain(ctx, Key::Home) {
                    cur = 0;
                } else if plain(ctx, Key::End) {
                    cur = n - 1;
                } else {
                    moved = false;
                }
                if moved {
                    self.list_cursor = Some(cur);
                    self.list_scroll = true;
                    return;
                }
                if self.side_view == 0 {
                    if plain(ctx, Key::Enter) {
                        if let Some(s) = self.visible_sessions().get(cur).cloned() {
                            self.open_session(&s, None);
                        }
                        return;
                    }
                    if plain(ctx, Key::R) || plain(ctx, Key::F2) {
                        if let Some(s) = self.visible_sessions().get(cur).cloned() {
                            self.rename_session(&s);
                        }
                        return;
                    }
                    if plain(ctx, Key::Backspace) || plain(ctx, Key::Delete) {
                        if let Some(s) = self.visible_sessions().get(cur).cloned() {
                            self.remove_session(&s);
                        }
                        return;
                    }
                    if plain(ctx, Key::M) || take(ctx, |kk, m| kk == Key::F10 && m.shift) {
                        self.list_cursor = Some(cur);
                        self.list_menu = true;
                        return;
                    }
                } else if plain(ctx, Key::Enter) {
                    if let Some(e) = self.visible_inventory().get(cur).map(|e| e.path.clone()) {
                        self.open_inventory_file(&e);
                    }
                    return;
                }
            }
        }
        // 5. Eingabefeld
        let focused = ctx.memory(|m| m.has_focus(prompt_id()));
        ctx.input(|i| {
            for event in &i.events {
                match event {
                    Event::Ime(egui::ImeEvent::Preedit { text, .. }) => self.prompt_ime_active = !text.is_empty(),
                    Event::Ime(egui::ImeEvent::Commit(_)) => self.prompt_ime_active = false,
                    #[allow(deprecated)]
                    Event::Ime(egui::ImeEvent::Disabled) => self.prompt_ime_active = false,
                    _ => {}
                }
            }
        });
        let ime = self.prompt_ime_active;
        if focused && !ime {
            if let Some(list) = self.slash_list() {
                let n = list.len();
                if plain(ctx, Key::ArrowDown) {
                    self.slash_cursor = (self.slash_cursor + 1) % n;
                    self.slash_scroll = true;
                    return;
                }
                if plain(ctx, Key::ArrowUp) {
                    self.slash_cursor = (self.slash_cursor + n - 1) % n;
                    self.slash_scroll = true;
                    return;
                }
                let pick = list[self.slash_cursor.min(n - 1)].name.clone();
                if plain(ctx, Key::Enter) {
                    self.prompt = format!("/{pick}");
                    self.submit();
                    return;
                }
                if plain(ctx, Key::Tab) {
                    self.set_prompt(format!("/{pick} "));
                    return;
                }
                if plain(ctx, Key::Escape) {
                    self.slash_dismissed = Some(self.prompt.clone());
                    return;
                }
            }
            if plain(ctx, Key::Enter) {
                self.submit();
                return;
            }
            let before_nl = self.prompt.chars().take(self.prompt_cursor).any(|c| c == '\n');
            let after_nl = self.prompt.chars().skip(self.prompt_cursor).any(|c| c == '\n');
            if k(ctx, Key::ArrowUp, true, false, false) || (!before_nl && plain(ctx, Key::ArrowUp)) {
                self.recall(-1);
                return;
            }
            if k(ctx, Key::ArrowDown, true, false, false) || (!after_nl && plain(ctx, Key::ArrowDown)) {
                self.recall(1);
                return;
            }
        }
        // 6. Globale Kürzel
        if plain(ctx, Key::Escape) {
            if self.busy || self.active_job.is_some() {
                self.abort_current();
            }
        } else if plain(ctx, Key::F1) {
            self.keyhelp = true;
        } else if k(ctx, Key::Num1, true, false, false) {
            self.focus_prompt = true;
        } else if k(ctx, Key::Num2, true, false, false) || k(ctx, Key::B, false, true, false) {
            self.store.side_visible = !self.store.side_visible;
            self.save_store();
        } else if k(ctx, Key::B, false, true, true) {
            self.toggle_net();
        } else if k(ctx, Key::N, true, false, false) {
            self.new_session();
        } else if k(ctx, Key::A, true, false, false) {
            self.abort_current();
        } else if k(ctx, Key::F, true, false, false) {
            self.fork_from_here();
        } else if k(ctx, Key::M, true, false, false) {
            self.open_picker(PickerKind::Model);
        } else if k(ctx, Key::T, true, false, false) {
            self.open_picker(PickerKind::Thinking);
        } else if k(ctx, Key::Num5, true, false, false) {
            if self.preview.is_some() {
                self.hide_preview();
            } else if self.side_view == 0 {
                let picked = self.list_cursor.and_then(|i| self.visible_sessions().get(i).cloned());
                if let Some(s) = picked {
                    self.show_preview(&s.path, &s.name, false);
                }
            } else {
                let picked = self.list_cursor.and_then(|i| self.visible_inventory().get(i).map(|e| (*e).clone()));
                if let Some(e) = picked {
                    self.show_preview(&e.path, &e.name, false);
                }
            }
        } else if take(ctx, |kk, m| matches!(kk, Key::Plus | Key::Equals) && m.alt && !m.ctrl) {
            self.resize_output(1);
        } else if take(ctx, |kk, m| kk == Key::Minus && m.alt && !m.ctrl) {
            self.resize_output(-1);
        }
    }

    pub fn slash_list(&self) -> Option<Vec<Cmd>> {
        let visible = self.prompt_focused || ctx_pointer_in(&self.ctx, self.slash_rect);
        if !visible || self.slash_dismissed.as_deref() == Some(self.prompt.as_str()) || self.prompt_cursor != self.prompt.chars().count() {
            return None;
        }
        slash_matches(&self.commands, &self.prompt).filter(|l| !l.is_empty())
    }

    // ---------- Seitenleiste ----------

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(5.0, 6.0);
        let w = ui.available_width();
        let side_label = SIDE_VIEWS[self.side_view.min(SIDE_VIEWS.len() - 1)];
        self.pick_button(ui, PickerKind::SideView, side_label, w, "Ansicht wählen");
        if self.side_view == 0 {
            ui.horizontal(|ui| {
                ui.set_height(28.0);
                if btn(ui, "+ Neu", 12.0, vec2(9.0, 3.0), vec2(0.0, 28.0), false, true).on_hover_text("Neue Session (Alt+N)").clicked() {
                    self.new_session();
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let only = self.store.only_bookmarks;
                    let label = if !only { "Alle" } else if self.store.colored_bookmarks { "Farben" } else { "Bookmarks" };
                    if btn(ui, label, 12.0, vec2(7.0,3.0), vec2(0.0,28.0), only, true).on_hover_text("Alle → nur Bookmarks (Zeit) → nur Bookmarks (Farbkategorie/Zeit)").clicked() {
                        if !only { self.store.only_bookmarks = true; self.store.colored_bookmarks = false; }
                        else if !self.store.colored_bookmarks { self.store.colored_bookmarks = true; }
                        else { self.store.only_bookmarks = false; self.store.colored_bookmarks = false; }
                        self.save_store();
                    }
                });
            });
        } else if self.inventory_loading {
            ui.add(Label::new(txt("Lädt…", 12.0, DIM)));
        }
        ui.add_space(4.0);
        let sep = ui.available_rect_before_wrap();
        ui.painter().hline(sep.x_range(), sep.top(), Stroke::new(1.0, BORDER));
        ui.add_space(4.0);
        if self.side_view == 0 {
            let list = self.visible_sessions();
            let active = self.session_file();
            let cursor = self.list_cursor;
            let scroll = std::mem::take(&mut self.list_scroll);
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 5.0;
                if list.is_empty() && self.store.only_bookmarks {
                    ui.add(Label::new(txt("Keine Bookmarks. Lesezeichen-Knopf oben zeigt wieder alle Sessions.", 12.0, DIM)).wrap());
                }
                for (i, s) in list.iter().enumerate() {
                    let r = self.session_card(ui, s, s.path == active, cursor == Some(i));
                    if cursor == Some(i) && scroll {
                        r.scroll_to_me(None);
                    }
                }
            });
        } else {
            let entries: Vec<pi_backend::inventory::Entry> = self.visible_inventory().into_iter().cloned().collect();
            let cursor = self.list_cursor;
            let scroll = std::mem::take(&mut self.list_scroll);
            let cat_color = match self.side_view {
                1 => CAT_EXT,
                2 => CAT_SKILLS,
                3 => CAT_AGENTS,
                _ => CAT_HARNESS,
            };
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 5.0;
                if entries.is_empty() && !self.inventory_loading {
                    ui.add(Label::new(txt("Keine Einträge gefunden.", 12.0, DIM)).wrap());
                }
                for (i, e) in entries.iter().enumerate() {
                    let r = self.inventory_card(ui, e, cursor == Some(i), cat_color);
                    if cursor == Some(i) && scroll {
                        r.scroll_to_me(None);
                    }
                }
            });
        }
    }

    fn inventory_card(&mut self, ui: &mut Ui, e: &pi_backend::inventory::Entry, selected: bool, cat_color: Color32) -> Response {
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 44.0), Sense::hover());
        let resp = ui.interact(rect, Id::new(("invcard", &e.path)), Sense::click());
        let t = if selected { 0.22 } else if resp.hovered() { 0.17 } else { 0.09 };
        let p = ui.painter();
        p.rect_filled(rect, 8, mix(cat_color, t, CARD));
        let border = if selected { cat_color } else { mix(cat_color, 0.32, CARD) };
        p.rect_stroke(rect, 8, Stroke::new(1.0, border), StrokeKind::Inside);
        let tw = rect.width() - 20.0;
        let mut job = LayoutJob::simple_singleline(e.name.clone(), FontId::proportional(13.0), FG);
        job.wrap = TextWrapping::truncate_at_width(tw);
        p.galley(rect.min + vec2(10.0, 6.0), p.layout_job(job), FG);
        let meta = if e.harness.is_empty() { pi_backend::tilde(&e.path) } else { format!("{} · {}", e.harness, pi_backend::tilde(&e.path)) };
        let mut job = LayoutJob::simple_singleline(meta, FontId::proportional(11.0), DIM);
        job.wrap = TextWrapping::truncate_at_width(tw);
        p.galley(rect.min + vec2(10.0, 24.0), p.layout_job(job), DIM);
        if resp.hovered() {
            self.preview_anchor = rect;
            self.preview_hover_until = std::time::Instant::now() + Duration::from_millis(300);
        }
        if resp.clicked() || (resp.hovered() && resp.hover_pos().is_some() && self.preview.as_ref().is_none_or(|p| p.path != e.path)) {
            self.show_preview(&e.path, &e.name, false);
        }
        if resp.double_clicked() {
            self.open_inventory_file(&e.path);
        }
        resp.context_menu(|ui| {
            ui.set_min_width(170.0);
            if ui.button("Vorschau").clicked() {
                self.show_preview(&e.path, &e.name, false);
                ui.close();
            }
            if ui.button("Bearbeiten").clicked() {
                self.open_inventory_file(&e.path);
                ui.close();
            }
        });
        resp
    }

    /// Vorschau-Spalte (Stufe 3, Alt+5): lokale Kurzinfo immer; KI-Zweckerklärung nur nach
    /// ausdrücklicher Zustimmung (Datenschutz/Kosten), vereinfacht als feste Spalte statt
    /// Hover-Tooltip wie im Original.
    fn preview_panel(&mut self, ui: &mut Ui) {
        let Some(p) = &self.preview else { return };
        let (name, path, text, method, loading) = (p.name.clone(), p.path.clone(), p.text.clone(), p.method.clone(), p.loading);
        let (ai_text, ai_method, ai_loading, ai_visible, ask_consent) = (p.ai_text.clone(), p.ai_method.clone(), p.ai_loading, p.ai_visible, p.ask_consent);
        ui.horizontal(|ui| {
            ui.add(Label::new(RichText::new("Vorschau").size(14.0).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if btn(ui, "×", 13.0, vec2(8.0, 2.0), Vec2::ZERO, false, true).on_hover_text("Vorschau schließen (Alt+5)").clicked() {
                    self.hide_preview();
                }
                if btn(ui, "Cache leeren", 10.5, vec2(6.0, 2.0), Vec2::ZERO, false, true).on_hover_text("Zwischengespeicherte Kurzinfos und KI-Zweckerklärungen löschen (fragt vorher nach)").clicked() {
                    self.confirm_clear_preview_cache();
                }
            });
        });
        ui.add_space(4.0);
        ui.add(Label::new(txt(name, 13.0, FG)).wrap());
        ui.add(Label::new(txt(pi_backend::tilde(&path), 11.0, DIM)).truncate());
        ui.add_space(8.0);
        if !path.ends_with(".jsonl") {
            ui.horizontal(|ui| {
                if ui.button("Intern bearbeiten…").clicked() { self.open_inventory_file(&path); }
                if ui.button("VSCodium ↗").clicked() { self.open_editor(Some(&path)); }
            });
        }
        ScrollArea::vertical().max_height(440.0).auto_shrink([false, true]).show(ui, |ui| {
            if loading {
                spinner(ui);
            }
            ui.add(Label::new(txt(text, 12.5, FG)).wrap());
            if !method.is_empty() {
                ui.add(Label::new(txt(format!("Quelle: {method}"), 10.5, DIM)).wrap());
            }
            ui.add_space(10.0);
            let sep = ui.available_rect_before_wrap();
            ui.painter().hline(sep.x_range(), sep.top(), Stroke::new(1.0, BORDER));
            ui.add_space(8.0);
            if ask_consent {
                ui.add(Label::new(txt(
                    "KI-Zweckerklärung nutzt ein Hilfsmodell und sendet dazu einen Dateiausschnitt. Fortfahren?",
                    12.0,
                    FG,
                )).wrap());
                ui.horizontal(|ui| {
                    if btn(ui, "Zustimmen & erklären", 12.0, vec2(10.0, 5.0), Vec2::ZERO, true, true).clicked() {
                        self.allow_ai_explain();
                    }
                    if btn(ui, "Abbrechen", 12.0, vec2(10.0, 5.0), Vec2::ZERO, false, true).clicked() {
                        self.deny_ai_explain();
                    }
                });
            } else if ai_visible {
                ui.add(Label::new(RichText::new("KI-Zweck").size(12.0).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))));
                if ai_loading {
                    spinner(ui);
                }
                Frame::new().fill(mix(ACC, 0.10, PANEL)).corner_radius(8).inner_margin(Margin::same(10)).show(ui, |ui| {
                    ui.add(Label::new(txt(ai_text, 12.5, FG)).wrap());
                    if !ai_method.is_empty() { ui.add(Label::new(txt(format!("LLM: {ai_method}"), 9.0, DIM)).wrap()); }
                });
            }
            ui.horizontal_wrapped(|ui| {
                if btn(ui, "Von KI erklären lassen…", 12.0, vec2(8.0, 4.0), Vec2::ZERO, false, !ai_loading).clicked() {
                    self.request_ai_explain(ai_visible);
                }
                if btn(ui, "Hilfsmodell…", 10.5, vec2(6.0, 3.0), Vec2::ZERO, false, true).on_hover_text("KI-Hilfsmodell auswählen").clicked() {
                    self.configure_preview_helper();
                }
            });
        });
    }

    /// Vereinfachtes, fest positioniertes Kontextmenü für den per Tastatur markierten
    /// Sessioneintrag (M / Umschalt+F10) – anstelle eines pro-Zeile positionierten Menüs.
    fn list_menu_overlay(&mut self, ctx: &Context) {
        if !self.list_menu {
            return;
        }
        let Some(idx) = self.list_cursor else {
            self.list_menu = false;
            return;
        };
        let Some(s) = self.visible_sessions().get(idx).cloned() else {
            self.list_menu = false;
            return;
        };
        let active = s.path == self.session_file();
        let bm = self.store.is_bookmark(&s.path);
        let mut close = true;
        egui::Modal::new(Id::new("list-menu"))
            .backdrop_color(Color32::from_black_alpha(0x55))
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(10).inner_margin(Margin::same(10)))
            .show(ctx, |ui| {
                ui.set_width(220.0);
                ui.add(Label::new(RichText::new("Session-Menü").size(13.0).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))));
                ui.add_space(4.0);
                close = false;
                if !active && ui.button("Öffnen").clicked() {
                    self.open_session(&s, None);
                    close = true;
                }
                if ui.button(if bm { "Bookmark entfernen" } else { "Als Bookmark markieren" }).clicked() {
                    self.toggle_bookmark(&s.path);
                }
                if ui.button("Umbenennen").clicked() {
                    self.rename_session(&s);
                    close = true;
                }
                if !active && ui.button("Löschen …").clicked() {
                    self.remove_session(&s);
                    close = true;
                }
                if ui.button("Abbrechen").clicked() {
                    close = true;
                }
            });
        if close {
            self.list_menu = false;
        }
    }

    fn session_card(&mut self, ui: &mut Ui, s: &pi_backend::sessions::SessionInfo, active: bool, selected: bool) -> Response {
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 52.0), Sense::hover());
        let resp = ui.interact(rect, Id::new(("card", &s.path)), Sense::click());
        let bm = self.store.is_bookmark(&s.path);
        let t = if active || selected { 0.22 } else if resp.hovered() { 0.17 } else { 0.09 };
        let p = ui.painter();
        p.rect_filled(rect, 8, mix(CAT, t, CARD));
        let border = if active || selected { CAT } else { mix(CAT, 0.32, CARD) };
        p.rect_stroke(rect, 8, Stroke::new(1.0, border), StrokeKind::Inside);
        let title = [&s.name, &s.title].into_iter().find(|t| !t.is_empty()).cloned().unwrap_or("(leer)".into());
        let tw = rect.width() - 24.0;
        let mut job = LayoutJob::simple_singleline(title.clone(), FontId::proportional(13.0), FG);
        job.wrap = TextWrapping::truncate_at_width(tw);
        p.galley(rect.min + vec2(12.0, 8.0), p.layout_job(job), FG);
        let meta = format!("{} · {}", fmt_date(s.modified), pi_backend::tilde(&s.cwd));
        let mut job = LayoutJob::simple_singleline(meta, FontId::proportional(11.0), DIM);
        job.wrap = TextWrapping::truncate_at_width((tw - 102.0).max(10.0));
        p.galley(rect.min + vec2(12.0, 29.0), p.layout_job(job), DIM);

        if resp.hovered() {
            self.preview_anchor = rect;
            self.preview_hover_until = std::time::Instant::now() + Duration::from_millis(300);
        }
        if resp.hovered() && self.preview.as_ref().is_none_or(|p| p.path != s.path) {
            self.show_preview(&s.path, &title, false);
        }
        let bm_rect = Rect::from_min_size(rect.right_bottom() - vec2(6.0 + 23.0, 5.0 + 23.0), vec2(23.0, 23.0));
        let color_rect = bm_rect.translate(vec2(-25.0, 0.0));
        let act_rect = bm_rect.translate(vec2(-75.0, 0.0));
        let delete_rect = bm_rect.translate(vec2(-50.0, 0.0));
        let delete_resp = ui.interact(delete_rect, Id::new(("card-delete", &s.path)), Sense::click()).on_hover_text("Session löschen");
        icons::paint_icon_button(ui, delete_rect, &delete_resp, !active, IconStyle { bg: None, fg: DIM, hover_fg: FG, icon: 13.0, radius: 5 }, icons::trash);
        if !active && delete_resp.clicked() { self.remove_session(s); }
        let bm_resp = ui.interact(bm_rect, Id::new(("cardbm", &s.path)), Sense::click()).on_hover_text(if bm { "Bookmark entfernen" } else { "Als Bookmark markieren" });
        let st = IconStyle { bg: None, fg: if bm { crate::bookmark_palette::color(self.store.bookmark_color(&s.path)) } else { DIM }, hover_fg: FG, icon: 13.0, radius: 5 };
        icons::paint_icon_button(ui, bm_rect, &bm_resp, true, st, |p, r, c| icons::bookmark_card(p, r, c, bm));
        let act_resp = ui
            .interact(act_rect, Id::new(("cardact", &s.path)), Sense::click())
            .on_hover_text("Session umbenennen (F2)");
        {
            let st = IconStyle { bg: None, fg: DIM, hover_fg: FG, icon: 13.0, radius: 5 };
            icons::paint_icon_button(ui, act_rect, &act_resp, true, st, icons::pencil);
        }
        // Farbmenü muss Klicks und Textbearbeitung innen erlauben.
        let color_resp = ui.interact(color_rect, Id::new(("bookmark-color", &s.path)), Sense::click()).on_hover_text("Bookmark-Farbe · acht Standardfarben");
        if bm {
            ui.painter().circle_filled(color_rect.center(), 5.0, crate::bookmark_palette::color(self.store.bookmark_color(&s.path)));
            egui::Popup::menu(&color_resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                for i in 0..8 {
                    ui.horizontal(|ui| {
                        let (r, response) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 8.0, crate::bookmark_palette::color(i));
                        if response.on_hover_text(self.store.category_name(i)).clicked() {
                            self.store.set_bookmark_color(&s.path, i);
                            self.save_store();
                        }
                        let mut name = self.store.category_name(i);
                        let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(140.0).char_limit(48));
                        if edit.changed() {
                            self.store.set_category_name(i, &name);
                            self.save_store();
                        }
                        if (edit.has_focus() || edit.lost_focus()) && ui.input(|input| input.key_pressed(Key::Enter)) {
                            self.store.set_category_name(i, &name);
                            self.save_store();
                            ui.close();
                        }
                    });
                }
            });
        }
        if bm_resp.clicked() {
            self.toggle_bookmark(&s.path);
        } else if act_resp.clicked() {
            self.rename_session(s);
        } else if resp.clicked() && !active {
            self.open_session(s, None);
        }
        resp.context_menu(|ui| {
            ui.set_min_width(190.0);
            if !active && ui.button("Öffnen   ↵").clicked() {
                self.open_session(s, None);
                ui.close();
            }
            if ui.button(if bm { "Bookmark entfernen" } else { "Als Bookmark markieren" }).clicked() {
                self.toggle_bookmark(&s.path);
                ui.close();
            }
            if ui.button("Session umbenennen   R").clicked() {
                self.rename_session(s);
                ui.close();
            }
            if !active && ui.button("Löschen …   ⌫").clicked() {
                self.remove_session(s);
                ui.close();
            }
        });
        resp
    }

    // ---------- Kopfzeile ----------

    fn pick_button(&mut self, ui: &mut Ui, kind: PickerKind, label: &str, max_w: f32, tip: &str) {
        let mut job = LayoutJob::simple_singleline(label.to_string(), FontId::proportional(13.0), FG);
        job.wrap = TextWrapping::truncate_at_width((max_w - 34.0).max(20.0));
        let galley = ui.painter().layout_job(job);
        let size = vec2(if matches!(kind, PickerKind::SideView | PickerKind::Workspace) { max_w } else { (galley.size().x + 34.0).min(max_w) }, if kind == PickerKind::SideView { 30.0 } else { 28.0 });
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let open = self.picker.as_ref().is_some_and(|p| p.kind == kind);
        let bg = if open {
            PICK_OPEN
        } else if resp.hovered() {
            SELECT_HOVER
        } else {
            BTN
        };
        let p = ui.painter();
        p.rect_filled(rect, 6, bg);
        p.galley(pos_left(rect, 10.0, galley.size().y), galley, FG);
        icons::chevron(p, rect, FG);
        self.anchors.insert(kind, rect);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.on_hover_text(tip).clicked() {
            self.open_picker(kind);
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.allocate_ui_with_layout(vec2(ui.available_width(), 34.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_height(30.0);
            let side_on = self.store.side_visible;
            if icons::icon_button(ui, vec2(32.0, 30.0), true, IconStyle { icon: 28.0, fg: ACC, ..Default::default() }, "Seitenleiste ein-/ausblenden", |p, r, c| {
                let box_rect = r.shrink(0.5);
                p.rect_stroke(box_rect, 2, Stroke::new(1.2, c), StrokeKind::Inside);
                let x = box_rect.left() + box_rect.width() * 0.3;
                p.line_segment([pos2(x, box_rect.top()), pos2(x, box_rect.bottom())], Stroke::new(1.2, c));
                if side_on { p.rect_filled(Rect::from_min_max(box_rect.min, pos2(x, box_rect.bottom())).shrink(1.5), 1, c); }
            }).clicked() {
                self.store.side_visible = !self.store.side_visible;
                self.save_store();
            }
            let commands_button = btn(ui, "/ Befehle", 13.0, vec2(10.0, 4.0), vec2(0.0, 28.0), false, true)
                .on_hover_text("Alle Slash-Befehle · Auswahl in die Eingabe übernehmen");
            egui::Popup::menu(&commands_button).show(|ui| {
                ui.set_max_height(360.0);
                ScrollArea::vertical().show(ui, |ui| {
                    let mut commands = self.commands.clone();
                    commands.sort_by_key(|c| c.name.to_lowercase());
                    let mut entries: Vec<String> = commands.iter().filter(|c| !c.name.starts_with("skill:")).map(|c| c.name.clone()).collect();
                    entries.push("skill:".into()); entries.sort_by_key(|s| s.to_lowercase()); entries.dedup();
                    for entry in entries {
                        if entry == "skill:" {
                            ui.menu_button(RichText::new("/skill:").italics().color(ACC), |ui| {
                                for command in commands.iter().filter(|c| c.name.starts_with("skill:")) {
                                    if ui.button(format!("/{}", command.name)).clicked() { self.set_prompt(format!("/{} ", command.name)); ui.close(); }
                                }
                                if !commands.iter().any(|c| c.name.starts_with("skill:")) { ui.label("Keine Skills von pi gemeldet"); }
                            });
                        } else if ui.button(format!("/{}", entry)).clicked() {
                            self.set_prompt(format!("/{} ", entry)); ui.close();
                        }
                    }
                });
            });
            let ws = format!("Workspace: {}", if self.cwd.is_empty() { "—".into() } else { pi_backend::tilde(&self.cwd) });
            let w = (ui.available_width() - 32.0) * 0.30;
            Frame::new().stroke(Stroke::new(1.0, BORDER)).corner_radius(7).inner_margin(Margin::same(2)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                self.pick_button(ui, PickerKind::Workspace, &ws, w.max(100.0), "Arbeitsordner wählen");
                let st = IconStyle { bg: None, fg: DIM, hover_fg: FG, icon: 14.0, radius: 5 };
                if icons::icon_button(ui, vec2(26.0, 26.0), true, st, "Workspace in VSCodium öffnen", icons::external).clicked() {
                    self.open_editor(None);
                }
            });
            let ml = self.model_label();
            self.pick_button(ui, PickerKind::Model, &ml, 260.0, "Modell wählen (Alt+M)");
            let tl = self.thinking_level();
            self.pick_button(ui, PickerKind::Thinking, &format!("Denken: {}", if tl.is_empty() { "—" } else { &tl }), 160.0, "Denkstufe wählen (Alt+T)");
            if btn(ui, "⑂ Fork", 13.0, vec2(10.0, 4.0), vec2(0.0, 28.0), false, true).on_hover_text("Ab einer früheren Nachricht abzweigen (Alt+F)").clicked() {
                self.fork_from_here();
            }
            ui.spacing_mut().item_spacing.x = 2.0;
            if btn(ui, "A−", 13.0, vec2(7.0, 4.0), vec2(0.0, 28.0), false, true).on_hover_text("Ausgabeschrift kleiner (Alt+−)").clicked() {
                self.resize_output(-1);
            }
            let font_size = btn(ui, &format!("{} px", self.store.output_size as i32), 12.0, vec2(7.0, 4.0), vec2(0.0, 28.0), false, true).on_hover_text("Schriftgröße zurücksetzen");
            ui.painter().rect_stroke(font_size.rect, 6, Stroke::new(1.0, BORDER), StrokeKind::Inside);
            if font_size.clicked() { self.resize_output(0); }
            if btn(ui, "A+", 13.0, vec2(7.0, 4.0), vec2(0.0, 28.0), false, true).on_hover_text("Ausgabeschrift größer (Alt++)").clicked() {
                self.resize_output(1);
            }
            ui.spacing_mut().item_spacing.x = 8.0;
            if btn(ui, "?", 13.0, vec2(9.0, 4.0), vec2(28.0, 28.0), false, true).on_hover_text("Tastatursteuerung (F1)").clicked() {
                self.keyhelp = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let net_on = self.store.net_visible;
                let tip = if net_on { "Netz-Seitenleiste ausblenden" } else { "Netz-Seitenleiste einblenden (menschlicher Chat, in Vorbereitung)" };
                if icons::icon_button(ui, vec2(32.0, 30.0), true, IconStyle { icon: 28.0, fg: OK, ..Default::default() }, tip, |p, r, c| {
                    let box_rect = r.shrink(0.5);
                    p.rect_stroke(box_rect, 2, Stroke::new(1.2, c), StrokeKind::Inside);
                    let x = box_rect.right() - box_rect.width() * 0.3;
                    p.line_segment([pos2(x, box_rect.top()), pos2(x, box_rect.bottom())], Stroke::new(1.2, c));
                    if net_on { p.rect_filled(Rect::from_min_max(pos2(x, box_rect.top()), box_rect.max).shrink(1.5), 1, c); }
                }).clicked() {
                    self.toggle_net();
                }
            });
        });
    }

    pub(crate) fn toggle_net(&mut self) {
        self.store.net_visible = !self.store.net_visible;
        self.save_store();
    }

    /// Statuszeile direkt über der Eingabe: links Zähler bzw. letzter Lauf, Mitte der
    /// Status-Chip (Phase · Laufzeit · Schätzung) zwischen zwei Pixel-Schlangen, rechts
    /// laufende Shell-/pi-Befehle und die Warteschlange.
    fn status_line(&mut self, ui: &mut Ui) {
        let r = ui.max_rect();
        let busy = self.busy;
        let job = match &self.active_job {
            Some(Job::Bash(_)) => Some("Shell-Befehl läuft"),
            Some(Job::Cli(_)) => Some("pi-Befehl läuft"),
            None => None,
        };
        let moving = busy || job.is_some();
        let chip_w: f32 = if busy { 320.0 } else { 200.0 };
        let total: f32 = chip_w + if moving { 2.0 * (7.5 + 10.0) } else { 0.0 };
        let center = Rect::from_center_size(r.center(), vec2(total.min(r.width()), r.height()));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(center).layout(Layout::left_to_right(Align::Center)));
        c.spacing_mut().item_spacing.x = 10.0;
        if moving { snake(&mut c, false); }
        let phase = if busy { self.activity.phase_label(&self.status) } else { job.map(String::from).unwrap_or_else(|| self.status.clone()) };
        let (fill, fg) = if busy { (ACC, BG) } else if job.is_some() { (mix(ACC, 0.18, PANEL), FG) } else { (PANEL, DIM) };
        let chip_inner = (chip_w - 2.0 * 8.0).min(c.available_width() - if moving { 17.5 } else { 0.0 }).max(40.0);
        Frame::new().fill(fill).stroke(Stroke::new(1.0, if busy { ACC } else { BORDER })).corner_radius(6).inner_margin(Margin::symmetric(8, 2)).show(&mut c, |ui| {
            ui.style_mut().override_font_id = Some(egui::FontId::new(12.0, egui::FontFamily::Name("status-regular".into())));
            ui.set_width(chip_inner);
            ui.set_height(18.0);
            ui.spacing_mut().item_spacing.x = 6.0;
            if !busy {
                ui.add_sized(vec2(chip_inner, 18.0), Label::new(txt(phase, 12.5, fg).family(egui::FontFamily::Name("status-regular".into()))).truncate());
                return;
            }
            let sec = self.busy_since.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
            let (pct, overdue, est) = self.store.duration_percent(sec);
            let sep = |ui: &mut Ui| {
                let (s, _) = ui.allocate_exact_size(vec2(1.0, 14.0), Sense::hover());
                ui.painter().vline(s.center().x, s.y_range(), Stroke::new(1.0, BG.gamma_multiply(0.3)));
            };
            let phase_w = (chip_inner - 44.0 - 58.0 - 2.0 * (1.0 + 2.0 * 6.0)).max(30.0);
            ui.add_sized(vec2(phase_w, 18.0), Label::new(txt(phase, 12.5, fg).family(egui::FontFamily::Name("status-regular".into()))).truncate())
                .on_hover_text("Aktuelle Phase des Agentenlaufs (aus den Ereignissen von pi)");
            sep(ui);
            ui.add_sized(vec2(44.0, 18.0), Label::new(txt(run_activity::clock(sec), 12.0, fg).family(egui::FontFamily::Name("status-regular".into()))).truncate())
                .on_hover_text("Laufzeit des Agentenlaufs. Die Restdauer ist unbekannt; Tool-Aufrufe und weitere Schritte können sie beliebig verlängern.");
            sep(ui);
            let label = format!("≈{:.0}%{}", pct, if overdue { " +" } else { "" });
            let label = if overdue { RichText::new(label).italics() } else { RichText::new(label) };
            ui.add_sized(vec2(58.0, 18.0), Label::new(label.size(12.0).color(fg).family(egui::FontFamily::Name("status-regular".into()))).truncate())
                .on_hover_text(format!("Lokale Laufzeitschätzung, kein gemessener Fortschritt. Basis: Median letzter abgeschlossener Läufe, anfangs 180 s. Erwartung: {:.0} s. {}Restdauer unbekannt.", est, if overdue { "Länger als üblich. " } else { "" }));
        });
        if moving { snake(&mut c, true); }
        // Seitentext nur, wenn neben dem Chip Platz bleibt; keine Werkzeug-Rohdetails.
        let left = Rect::from_min_max(r.min, pos2(center.left() - 12.0, r.max.y));
        let counts = self.activity.counts_label();
        if left.width() > 60.0 && !counts.is_empty() {
            let mut l = ui.new_child(egui::UiBuilder::new().max_rect(left).layout(Layout::left_to_right(Align::Center)));
            l.add(Label::new(txt(counts, 12.0, DIM)).truncate()).on_hover_text(if self.activity.active {
                "Dieser Lauf: Runden (Modellantworten), abgeschlossene Werkzeugaufrufe, Fehler und wartende Nachrichten"
            } else {
                "Zusammenfassung des letzten abgeschlossenen Agentenlaufs"
            });
        }
    }

    // ---------- Fußzeile ----------

    fn usage_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            let usage = ui.add(Label::new(txt(self.usage_text.clone(), 12.0, DIM)).truncate());
            usage.on_hover_ui(|ui| {
                ui.set_width(390.0);
                ui.heading("Tokens & Kontext");
                ui.label(RichText::new(&self.usage_text).size(15.0).color(FG));
                ui.separator();
                egui::Grid::new("usage-details").spacing(vec2(18.0, 10.0)).show(ui, |ui| {
                    for (key, desc) in [("↑ Eingabe", "An das Modell gesendete Tokens"), ("↓ Ausgabe", "Vom Modell erzeugte Tokens"), ("R Cache", "Bereits verarbeitete, erneut gelesene Tokens"), ("CH Treffer", "Anteil aus dem Prompt-Cache"), ("Kontext %", "Belegung des verfügbaren Kontextfensters"), ("auto", "Automatische Kontextkomprimierung") ] {
                        ui.label(RichText::new(key).strong()); ui.label(desc); ui.end_row();
                    }
                });
            });
            let st = IconStyle { bg: None, fg: DIM, hover_fg: FG, icon: 14.0, radius: 5 };
            if icons::icon_button(ui, vec2(20.0, 20.0), true, st, "Tokenanzeige erklären", icons::info).clicked() {
                let details = self.usage_details.clone();
                self.ask(DialogKind::Editor, "Tokens & Kontext", "", vec![], &details, |_, _| {});
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let en = self.speech_available;
                let st = IconStyle { bg: None, fg: if self.speech_on { ACC } else { DIM }, hover_fg: FG, icon: 14.0, radius: 5 };
                let tip = if en { "Vorlesen stoppen" } else { "Keine Vorlesen-Extension geladen" };
                if icons::icon_button(ui, vec2(26.0, 22.0), en, IconStyle { fg: DIM, ..st }, tip, icons::stop).clicked() {
                    self.speech_command("stopp");
                }
                let on = self.speech_on;
                let tip = if en { "Sprachausgabe ein-/ausschalten" } else { "Keine Vorlesen-Extension geladen" };
                if icons::icon_button(ui, vec2(26.0, 22.0), en, st, tip, |p, r, c| icons::speaker(p, r, c, on)).clicked() {
                    self.speech_command(if on { "aus" } else { "an" });
                }
            });
        });
    }

    /// Leiste mit Material-Chips (eine Datei/URL je Chip) und Status, über dem Eingabefeld.
    /// Entspricht `#materialchips`/`#materialstatus` aus `ui/app.js`.
    fn material_bar(&mut self, ui: &mut Ui) {
        if !self.material.status.is_empty() {
            ui.add(Label::new(txt(self.material.status.clone(), 12.0, DIM)).truncate());
        }
        if self.material.items.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for item in self.material.items.clone() {
                let label = format!("{} {} ×", if item.url { "URL:" } else { "Datei:" }, item.value.rsplit('/').next().unwrap_or(&item.value));
                let resp = btn(ui, &label, 12.0, vec2(8.0, 4.0), Vec2::ZERO, false, !self.material.preparing).on_hover_text(item.value.clone());
                if resp.clicked() {
                    self.remove_material(item);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!self.material.preparing, |ui| {
                let refs = self.material.mode == crate::material_ui::MaterialMode::ReferencesOnly;
                let mut checked = refs;
                if ui.checkbox(&mut checked, "Nur Dateiverweise").changed() {
                    self.material.mode = if checked { crate::material_ui::MaterialMode::ReferencesOnly } else { crate::material_ui::MaterialMode::Auto };
                }
                if !self.material.routes.is_empty() {
                    egui::ComboBox::from_id_salt("material-route")
                        .selected_text(if self.material.route.is_empty() { "Medienroute: automatisch".to_string() } else { self.material.route.clone() })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.material.route, String::new(), "automatisch – bestätigte Route");
                            for r in self.material.routes.clone() {
                                let label = format!("{} · {}{}", r.label, r.modalities.join("/"), if r.available { "" } else { " · Zugang fehlt" });
                                ui.selectable_value(&mut self.material.route, r.id.clone(), label);
                            }
                        });
                }
            });
        });
    }

    fn footer(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        let gap = 8.0;
        let tools = Rect::from_min_size(full.min, vec2(32.0, full.height()));
        let send = Rect::from_min_max(Pos2::new(full.right() - 96.0, full.top()), full.max);
        let prompt = Rect::from_min_max(Pos2::new(tools.right() + gap, full.top()), Pos2::new(send.left() - gap, full.bottom()));
        ui.allocate_rect(full, Sense::hover());

        // Werkzeugspalte
        let h = (full.height() - 8.0) / 3.0;
        let st = IconStyle { icon: 14.0, ..Default::default() };
        let row = |i: f32| Rect::from_min_size(tools.min + vec2(0.0, i * (h + 4.0)), vec2(32.0, h));
        let r0 = ui.interact(row(2.0), Id::new("t-add"), Sense::click()).on_hover_text("Material hinzufügen");
        icons::paint_icon_button(ui, row(2.0), &r0, true, IconStyle { fg: Color32::WHITE, bg: Some(ACC), ..st }, icons::plus);
        let r1 = ui.interact(row(0.0), Id::new("t-up"), Sense::click()).on_hover_text("Ältere Eingabe (Alt+↑)");
        icons::paint_icon_button(ui, row(0.0), &r1, true, st, icons::up);
        let r2 = ui.interact(row(1.0), Id::new("t-down"), Sense::click()).on_hover_text("Neuere Eingabe (Alt+↓)");
        icons::paint_icon_button(ui, row(1.0), &r2, true, st, icons::down);
        if r0.clicked() {
            self.choose_material();
        }
        if r1.clicked() {
            self.recall(-1);
        }
        if r2.clicked() {
            self.recall(1);
        }

        // Eingabefeld
        self.prompt_rect = prompt;
        let focused = ui.memory(|m| m.has_focus(prompt_id()));
        ui.painter().rect_filled(prompt, 6, BG);
        ui.painter().rect_stroke(prompt, 6, Stroke::new(1.0, if focused { ACC } else { BORDER }), StrokeKind::Inside);
        let inner = prompt.shrink2(vec2(8.0, 6.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(Layout::top_down(Align::Min)));
        let size = self.store.output_size.min(16.0);
        let out = ScrollArea::vertical().id_salt("prompt-scroll").auto_shrink([false, false]).max_height(inner.height()).show(&mut child, |ui| {
            egui::TextEdit::multiline(&mut self.prompt)
                .id(prompt_id())
                .frame(Frame::NONE)
                .hint_text(txt(PLACEHOLDER, size, DIM))
                .font(FontId::proportional(size))
                .text_color(FG)
                .desired_width(inner.width() - 26.0)
                .desired_rows(2)
                .lock_focus(true)
                .return_key(Some(egui::KeyboardShortcut::new(Modifiers::ALT, Key::Enter)))
                .show(ui)
        });
        let out = out.inner;
        if let Some(c) = self.pending_cursor.take() {
            let mut st = out.state.clone();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(c))));
            st.store(ui.ctx(), prompt_id());
            self.prompt_cursor = c;
        } else {
            self.prompt_cursor = out.cursor_range.map(|r| r.primary.index.0).unwrap_or(self.prompt.chars().count());
        }
        if out.response.changed() {
            self.history.reset();
            self.slash_cursor = 0;
        }
        self.prompt_focused = out.response.has_focus();
        if self.focus_prompt && self.dialogs.is_empty() && !self.editor_open && !self.keyhelp {
            ui.memory_mut(|m| m.request_focus(prompt_id()));
            self.focus_prompt = false;
        }
        let ex = Rect::from_min_size(prompt.right_top() + vec2(-26.0, 4.0), vec2(22.0, 22.0));
        let er = ui.interact(ex, Id::new("t-expand"), Sense::click()).on_hover_text("Im großen Fenster bearbeiten");
        icons::paint_icon_button(ui, ex, &er, true, IconStyle { bg: None, fg: DIM, hover_fg: FG, icon: 13.0, radius: 5 }, icons::expand);
        if er.clicked() {
            self.editor_text = self.prompt.clone();
            self.editor_target = EditorTarget::Prompt;
            self.editor_open = true;
            self.editor_focus = true;
        }

        // Senden / Abbrechen
        let sh = (full.height() - 4.0) * 0.6;
        let sr = Rect::from_min_size(send.min, vec2(96.0, sh));
        let ar = Rect::from_min_max(Pos2::new(send.left(), sr.bottom() + 4.0), send.max);
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(sr));
        if btn(&mut c, "Senden", 14.0, vec2(8.0, 2.0), sr.size(), true, true).on_hover_text("Senden (Enter)").clicked() {
            self.submit();
        }
        let running = self.busy || self.active_job.is_some();
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(ar));
        if btn(&mut c, "Abbrechen", 13.0, vec2(8.0, 2.0), ar.size(), false, running).on_hover_text("Abbrechen (Esc)").clicked() {
            self.abort_current();
        }
    }

    // ---------- Chat ----------

    fn log(&mut self, ui: &mut Ui) {
        let size = self.store.output_size;
        let epoch = self.log_epoch;
        let mut acts: Vec<Act> = Vec::new();
        let force = std::mem::take(&mut self.force_scroll);
        let th = md_egui::Theme { size, ..Default::default() };
        let items = &mut self.items;
        let n = items.len();
        // Kein eigener Busy-Eintrag mehr im Verlauf: die Statuszeile über der Eingabe
        // zeigt Aktivität (Pixel-Schlangen) bereits an.
        let total = n;

        // Breite auf ein Raster runden, damit kleine Schwankungen (z. B. durch
        // ein auftauchendes/verschwindendes Scrollbar-Pixel) nicht bei jedem
        // Frame den kompletten Höhen-Cache verwerfen.
        let width_bucket = (ui.available_width() / 8.0).round() * 8.0;
        let cache_id = Id::new("copi-log-heights");
        let mut cache = virtual_log::load(ui, cache_id);
        cache.sync(total, width_bucket, size, epoch);

        let out = ScrollArea::vertical().id_salt("log").stick_to_bottom(true).auto_shrink([false, false]).show_viewport(ui, |ui, viewport| {
            Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 12.0;
                let avail = ui.available_width();
                let maxw = avail.min(900.0);
                let spacing = ui.spacing().item_spacing.y;
                // Großzügiger Überstand, damit knapp außerhalb des Viewports liegende
                // Einträge schon gemountet sind, bevor sie sichtbar werden.
                const OVERSCAN: f32 = 800.0;
                const ESTIMATE: f32 = 90.0;
                let mut pl = virtual_log::plan(&cache, total, viewport, OVERSCAN, spacing, ESTIMATE);
                if force {
                    // Nur das Ende mounten, statt bei langen Sitzungen tausende
                    // Markdown-/Tool-Blöcke in einem einzigen Frame zu layouten.
                    let bottom = pl.total_height;
                    let end_viewport = Rect::from_min_max(
                        pos2(viewport.min.x, (bottom - viewport.height()).max(0.0)),
                        pos2(viewport.max.x, bottom),
                    );
                    pl = virtual_log::plan(&cache, total, end_viewport, OVERSCAN, spacing, ESTIMATE);
                }
                if pl.lead_space > 0.0 {
                    ui.allocate_exact_size(vec2(1.0, pl.lead_space), Sense::hover());
                }
                for i in pl.first..pl.last.min(total) {
                    let salt = (epoch, i);
                    let measured = ui.scope(|ui| {
                        match &mut items[i] {
                            Item::User(text) => {
                                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                    let fr = Frame::new().fill(ACC).corner_radius(10).inner_margin(Margin { left: 14, right: 30, top: 10, bottom: 10 }).show(ui, |ui| {
                                        ui.set_max_width(maxw - 44.0);
                                        ui.add(Label::new(txt(text.clone(), size, BG)).wrap());
                                    });
                                    let r = fr.response.rect;
                                    let br = Rect::from_min_size(r.right_top() + vec2(-24.0, 4.0), vec2(20.0, 20.0));
                                    let resp = ui.interact(br, Id::new(("redo", salt)), Sense::click()).on_hover_text("Anfrage in die Eingabe übernehmen");
                                    let col = if resp.hovered() { Color32::WHITE } else { alpha(BG, 0.7) };
                                    ui.painter().text(br.center(), Align2::CENTER_CENTER, "↻", FontId::proportional(14.0), col);
                                    if resp.clicked() {
                                        let t = text.clone();
                                        acts.push(Box::new(move |a| a.set_prompt(t)));
                                    }
                                });
                            }
                            Item::Assistant { doc, text, done } => {
                                let fr = Frame::new().fill(PANEL).corner_radius(10).inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
                                    ui.set_width(maxw - 28.0);
                                    md_egui::show_doc(ui, &th, doc.get(text), ("a", salt))
                                });
                                if let Some(link) = fr.inner.clicked_link {
                                    acts.push(Box::new(move |a| a.handle_markdown_link(&link)));
                                }
                                if *done {
                                    let r = fr.response.rect;
                                    let br = Rect::from_min_size(r.right_top() + vec2(-26.0, 4.0), vec2(22.0, 22.0));
                                    let resp = ui.interact(br, Id::new(("copy", salt)), Sense::click()).on_hover_text("Diese Textausgabe als Markdown kopieren");
                                    if resp.hovered() || fr.response.hovered() || ui.rect_contains_pointer(r) {
                                        let col = if resp.hovered() { FG } else { DIM };
                                        ui.painter().text(br.center(), Align2::CENTER_CENTER, "⧉", FontId::proportional(14.0), col);
                                    }
                                    if resp.clicked() {
                                        let t = text.clone();
                                        acts.push(Box::new(move |a| {
                                            a.ctx.copy_text(t);
                                            a.toast("Markdown kopiert.", ToastKind::Info);
                                        }));
                                    }
                                }
                            }
                            Item::Thinking(t) => {
                                ui.add(Label::new(txt(t.clone(), 13.0, DIM).italics()).wrap());
                            }
                            Item::Note { text, err, block } => {
                                let c = if *err { ERR } else { DIM };
                                if *block {
                                    Frame::new().fill(PANEL).corner_radius(8).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                                        ui.set_max_width(maxw - 24.0);
                                        ui.add(Label::new(txt(text.clone(), 13.0, FG)).wrap());
                                    });
                                } else {
                                    ui.vertical_centered(|ui| {
                                        ui.add(Label::new(txt(text.clone(), 13.0, c)).wrap());
                                    });
                                }
                            }
                            Item::Tool { name, args, out, open, err, path, .. } => {
                                let stripe = if *err { ERR } else { ACC };
                                let title = format!("{name} {args}");
                                let p2 = path.clone();
                                block(ui, maxw, stripe, salt, open, &title, "", p2.as_deref(), out, &mut acts);
                            }
                            Item::Shell { title, out, state, open, err } => {
                                let stripe = if *err { ERR } else if state.starts_with("läuft") { WARN } else { OK };
                                block(ui, maxw, stripe, salt, open, title, state, None, out, &mut acts);
                            }
                            Item::StartCard(card) => start_card(ui, maxw, card, salt, &mut acts),
                        }
                    });
                    cache.set(i, measured.response.rect.height());
                }
                if pl.trail_space > 0.0 {
                    ui.allocate_exact_size(vec2(1.0, pl.trail_space), Sense::hover());
                }
                if force {
                    ui.scroll_to_cursor(Some(Align::BOTTOM));
                }
            });
        });
        let _ = out;
        virtual_log::save(ui, cache_id, cache);
        for a in acts {
            a(self);
        }
    }

    // ---------- Überlagerungen ----------

    fn picker_overlay(&mut self, ctx: &Context) {
        let Some(kind) = self.picker.as_ref().map(|p| p.kind) else {
            self.picker_rect = Rect::NOTHING;
            return;
        };
        let anchor = self.anchors.get(&kind).copied().unwrap_or(Rect::from_min_size(Pos2::new(100.0, 12.0), vec2(200.0, 28.0)));
        let pressed = ctx.input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None });
        if let Some(pos) = pressed {
            if !self.picker_rect.contains(pos) && !anchor.contains(pos) && self.picker_rect != Rect::NOTHING {
                self.picker = None;
                self.picker_rect = Rect::NOTHING;
                return;
            }
        }
        let (rows, _) = self.picker_rows(kind);
        let screen = ctx.content_rect();
        let width = match kind {
            PickerKind::Thinking | PickerKind::Sort => anchor.width().max(160.0),
            _ => anchor.width().max(560.0f32.min(screen.width() * 0.8)),
        };
        let maxh = screen.height() * 0.55;
        let moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO);
        let p = self.picker.as_mut().unwrap();
        let mut chosen = None;
        let area = egui::Area::new(Id::new("picker"))
            .order(Order::Foreground)
            .fade_in(false)
            .fixed_pos(anchor.left_bottom() + vec2(0.0, 4.0))
            .constrain(true)
            .show(ctx, |ui| {
                Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(6).inner_margin(Margin::same(4)).show(ui, |ui| {
                    ui.set_min_size(vec2(width, (rows.len() as f32 * 34.0).min(maxh)));
                    ScrollArea::vertical().max_height(maxh).auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, r) in rows.iter().enumerate() {
                            let (rect, resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
                            if resp.hovered() && moved {
                                p.cursor = i;
                            }
                            let sel = i == p.cursor;
                            if sel {
                                ui.painter().rect_filled(rect, 4, ACC);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(rect, 4, PICK_OPEN);
                            }
                            let mut job = LayoutJob::simple_singleline(r.clone(), FontId::proportional(13.0), FG);
                            job.wrap = TextWrapping::truncate_at_width(width - 16.0);
                            let g = ui.painter().layout_job(job);
                            ui.painter().galley(pos_left(rect, 8.0, g.size().y), g, if sel { Color32::WHITE } else { FG });
                            if sel && p.scroll {
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                chosen = Some(i);
                            }
                        }
                    });
                });
            });
        p.scroll = false;
        self.picker_rect = area.response.rect;
        if let Some(i) = chosen {
            self.choose_pick(kind, i);
            self.focus_prompt = true;
        }
    }

    fn slash_overlay(&mut self, ctx: &Context) {
        let Some(list) = self.slash_list() else {
            self.slash_rect = Rect::NOTHING;
            return;
        };
        let n = list.len();
        self.slash_cursor = self.slash_cursor.min(n - 1);
        let width = self.prompt_rect.width();
        let moved = ctx.input(|i| i.pointer.delta() != Vec2::ZERO);
        let mut chosen = None;
        let (mut cursor, scroll) = (self.slash_cursor, std::mem::take(&mut self.slash_scroll));
        let area = egui::Area::new(Id::new("slash"))
            .order(Order::Foreground)
            .fade_in(false)
            .pivot(Align2::LEFT_BOTTOM)
            .fixed_pos(self.prompt_rect.left_top() - vec2(0.0, 4.0))
            .show(ctx, |ui| {
                Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(6).inner_margin(Margin::same(4)).show(ui, |ui| {
                    ui.set_width(width - 8.0);
                    ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, c) in list.iter().enumerate() {
                            let (rect, resp) = ui.allocate_exact_size(vec2(width - 8.0, 28.0), Sense::click());
                            if resp.hovered() && moved {
                                cursor = i;
                            }
                            let sel = i == cursor;
                            if sel {
                                ui.painter().rect_filled(rect, 4, ACC);
                            }
                            let fg = if sel { Color32::WHITE } else { FG };
                            let pnt = ui.painter();
                            let name = pnt.layout_no_wrap(format!("/{}", c.name), FontId::new(13.0, egui::FontFamily::Name(md_egui::BOLD_FAMILY.into())), fg);
                            let nw = name.size().x;
                            pnt.galley(pos_left(rect, 8.0, name.size().y), name, fg);
                            let src = pnt.layout_no_wrap(source_label(&c.source).to_string(), FontId::proportional(11.0), if sel { Color32::WHITE } else { DIM });
                            let sw = src.size().x;
                            pnt.galley(Pos2::new(rect.right() - 8.0 - sw, rect.center().y - src.size().y / 2.0), src, DIM);
                            let mut job = LayoutJob::simple_singleline(c.description.clone(), FontId::proportional(12.0), if sel { Color32::WHITE } else { DIM });
                            job.wrap = TextWrapping::truncate_at_width((rect.width() - nw - sw - 40.0).max(10.0));
                            let d = pnt.layout_job(job);
                            pnt.galley(pos_left(rect, 8.0 + nw + 12.0, d.size().y), d, DIM);
                            if sel && scroll {
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                chosen = Some(c.name.clone());
                            }
                        }
                    });
                    ui.add(Label::new(txt("Tippen filtert Namen · ↑↓ / Hover wählen · Klick/Enter ausführen · Tab vervollständigen · Esc schließen", 11.0, DIM)).truncate());
                });
            });
        self.slash_cursor = cursor;
        self.slash_rect = area.response.rect;
        if let Some(name) = chosen {
            self.prompt = format!("/{name}");
            self.slash_rect = Rect::NOTHING;
            self.submit();
            self.focus_prompt = true;
        }
    }

    fn dialog_overlay(&mut self, ctx: &Context) {
        let Some(d) = self.dialogs.front_mut() else { return };
        let mut answer: Option<Value> = None;
        let mut explain_tokens = false;
        let mut select_helper = false;
        let mut navigate: Option<String> = None;
        let mut navigate_material: Option<String> = None;
        let mut toggle_material: Option<(String, bool)> = None;
        let mut preview_material: Option<String> = None;
        let screen = ctx.content_rect();
        let width = 560.0f32.min(screen.width() - 24.0);
        let modal = egui::Modal::new(Id::new("dialog"))
            .backdrop_color(Color32::from_black_alpha(0xaa))
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(12).inner_margin(Margin::same(16)))
            .show(ctx, |ui| {
                ui.set_width(width - 32.0);
                ui.spacing_mut().item_spacing.y = 10.0;
                if !d.title.is_empty() {
                    ui.add(Label::new(RichText::new(d.title.clone()).size(16.0).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))).wrap());
                }
                if !d.message.is_empty() {
                    ui.add(Label::new(txt(d.message.clone(), 14.0, FG)).wrap());
                }
                match d.kind {
                    DialogKind::Select => {
                        let moved = ui.input(|i| i.pointer.delta() != Vec2::ZERO);
                        ScrollArea::vertical().max_height(screen.height() * 0.6).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for (i, o) in d.options.iter().enumerate() {
                                let (rect, resp) = ui.allocate_exact_size(vec2(width - 32.0, 30.0), Sense::click());
                                if resp.hovered() && moved {
                                    d.cursor = i;
                                }
                                let sel = i == d.cursor;
                                ui.painter().rect_filled(rect, 6, if sel { ACC } else { BTN });
                                let mut job = LayoutJob::simple_singleline(o.clone(), FontId::proportional(13.0), FG);
                                job.wrap = TextWrapping::truncate_at_width(rect.width() - 20.0);
                                let g = ui.painter().layout_job(job);
                                ui.painter().galley(pos_left(rect, 10.0, g.size().y), g, if sel { Color32::WHITE } else { FG });
                                if sel && d.scroll {
                                    resp.scroll_to_me(None);
                                }
                                if resp.clicked() {
                                    answer = Some(json!({"value": o}));
                                }
                            }
                        });
                        d.scroll = false;
                        let hint = format!("{} / {} Einträge · ↑↓ wählen · Home/End · Bild↑/↓ · Enter bestätigen · Esc abbrechen", d.cursor + 1, d.options.len());
                        ui.add(Label::new(txt(hint, 12.0, DIM)).wrap());
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if btn(ui, "Abbrechen", 13.0, vec2(14.0, 6.0), Vec2::ZERO, false, true).clicked() {
                                answer = Some(json!({"cancelled": true}));
                            }
                        });
                    }
                    DialogKind::Confirm => {
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if btn(ui, "Ja", 13.0, vec2(14.0, 6.0), vec2(64.0, 0.0), d.cursor == 0, true).clicked() {
                                answer = Some(json!({"confirmed": true}));
                            }
                            if btn(ui, "Nein", 13.0, vec2(14.0, 6.0), vec2(64.0, 0.0), d.cursor == 1, true).clicked() {
                                answer = Some(json!({"confirmed": false}));
                            }
                        });
                    }
                    DialogKind::Input | DialogKind::Editor => {
                        let id = Id::new("dialog-text");
                        let editor = d.kind == DialogKind::Editor;
                        let info_only = d.title.starts_with("Tokens & Kontext");
                        let te = if editor { egui::TextEdit::multiline(&mut d.text).desired_rows(14).interactive(!info_only) } else { egui::TextEdit::singleline(&mut d.text) };
                        let r = Frame::new().fill(BG).corner_radius(6).inner_margin(Margin::same(8)).show(ui, |ui| {
                            let te = te.id(id).frame(Frame::NONE).hint_text(d.placeholder.clone()).desired_width(f32::INFINITY).font(FontId::proportional(14.0));
                            if editor {
                                ScrollArea::vertical().max_height(screen.height() * 0.5).show(ui, |ui| {
                                    ui.set_min_height(180.0);
                                    ui.add(te)
                                })
                                .inner
                            } else {
                                ui.add(te)
                            }
                        });
                        if d.focus {
                            r.inner.request_focus();
                            d.focus = false;
                        }
                        if info_only {
                            ui.horizontal(|ui| {
                                explain_tokens = ui.button("Von KI erklären lassen…").clicked();
                                select_helper = ui.small_button("Hilfsmodell…").clicked();
                            });
                        }
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            let ok_label = if info_only { "Schließen" } else if editor { "Übernehmen (Strg+Enter)" } else { "OK" };
                            if btn(ui, ok_label, 13.0, vec2(14.0, 6.0), vec2(64.0, 0.0), true, true).clicked() {
                                answer = Some(json!({"value": d.text.clone()}));
                            }
                            if btn(ui, "Abbrechen", 13.0, vec2(14.0, 6.0), Vec2::ZERO, false, true).clicked() {
                                answer = Some(json!({"cancelled": true}));
                            }
                        });
                    }
                    DialogKind::Folder => {
                        let id = Id::new("dialog-folder-text");
                        let mut enter_pressed = false;
                        ui.horizontal(|ui| {
                            let r = Frame::new().fill(BG).corner_radius(6).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut d.text).id(id).frame(Frame::NONE).desired_width(ui.available_width() - 70.0).font(FontId::proportional(14.0)),
                                );
                                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                    enter_pressed = true;
                                }
                                resp
                            });
                            if d.focus {
                                r.inner.request_focus();
                                d.focus = false;
                            }
                            if btn(ui, "Hoch", 12.0, vec2(10.0, 6.0), Vec2::ZERO, false, d.parent.is_some()).clicked() {
                                if let Some(p) = d.parent.clone() {
                                    navigate = Some(p);
                                }
                            }
                        });
                        if enter_pressed {
                            navigate = Some(d.text.clone());
                        }
                        let moved = ui.input(|i| i.pointer.delta() != Vec2::ZERO);
                        ScrollArea::vertical().max_height(screen.height() * 0.45).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for (i, l) in d.folders.iter().enumerate() {
                                let (rect, resp) = ui.allocate_exact_size(vec2(width - 32.0, 28.0), Sense::click());
                                if resp.hovered() && moved {
                                    d.cursor = i;
                                }
                                let sel = i == d.cursor;
                                ui.painter().rect_filled(rect, 6, if sel { ACC } else { BTN });
                                let mut job = LayoutJob::simple_singleline(l.name.clone(), FontId::proportional(13.0), FG);
                                job.wrap = TextWrapping::truncate_at_width(rect.width() - 20.0);
                                let g = ui.painter().layout_job(job);
                                ui.painter().galley(pos_left(rect, 10.0, g.size().y), g, if sel { Color32::WHITE } else { FG });
                                if sel && d.scroll {
                                    resp.scroll_to_me(None);
                                }
                                if resp.double_clicked() {
                                    navigate = Some(l.path.clone());
                                } else if resp.clicked() {
                                    d.cursor = i;
                                }
                            }
                        });
                        d.scroll = false;
                        if d.folders.is_empty() {
                            ui.add(Label::new(txt("Keine Unterordner.", 12.0, DIM)).wrap());
                        }
                        ui.add(Label::new(txt("Enter im Textfeld: Ordner wechseln · Doppelklick: Unterordner öffnen · Strg+Enter/Knopf: auswählen · Backspace: Hoch", 11.0, DIM)).wrap());
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if btn(ui, "Ordner wählen", 13.0, vec2(14.0, 6.0), Vec2::ZERO, true, true).clicked() {
                                answer = Some(json!({"path": d.text.clone()}));
                            }
                            if btn(ui, "Abbrechen", 13.0, vec2(14.0, 6.0), Vec2::ZERO, false, true).clicked() {
                                answer = Some(json!({"cancelled": true}));
                            }
                        });
                    }
                    DialogKind::Material => {
                        ui.horizontal(|ui| {
                            ui.label("Sortierung:");
                            if ui.selectable_label(!self.material.sort_date, "Name").clicked() { self.material.sort_date = false; navigate_material = Some(d.text.clone()); }
                            if ui.selectable_label(self.material.sort_date, "Datum ↓").clicked() { self.material.sort_date = true; navigate_material = Some(d.text.clone()); }
                        });
                        let id = Id::new("dialog-material-text");
                        let mut enter_pressed = false;
                        ui.horizontal(|ui| {
                            let r = Frame::new().fill(BG).corner_radius(6).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut d.text).id(id).frame(Frame::NONE).desired_width(ui.available_width() - 70.0).font(FontId::proportional(14.0)),
                                );
                                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                    enter_pressed = true;
                                }
                                resp
                            });
                            if d.focus {
                                r.inner.request_focus();
                                d.focus = false;
                            }
                            if btn(ui, "Hoch", 12.0, vec2(10.0, 6.0), Vec2::ZERO, false, d.parent.is_some()).clicked() {
                                if let Some(p) = d.parent.clone() {
                                    navigate_material = Some(p);
                                }
                            }
                        });
                        if enter_pressed {
                            navigate_material = Some(d.text.clone());
                        }
                        let moved = ui.input(|i| i.pointer.delta() != Vec2::ZERO);
                        ScrollArea::vertical().max_height(screen.height() * 0.45).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for (i, e) in d.entries.iter().enumerate() {
                                let (rect, resp) = ui.allocate_exact_size(vec2(width - 32.0, 28.0), Sense::click());
                                if resp.hovered() && moved {
                                    d.cursor = i;
                                }
                                let sel = i == d.cursor;
                                let picked = d.selected.contains(&e.path);
                                ui.painter().rect_filled(rect, 6, if sel { ACC } else if picked { mix(BTN, 0.3, OK) } else { BTN });
                                let prefix = if e.is_dir { "\u{1F4C1} " } else if picked { "\u{2611} " } else { "\u{2610} " };
                                let mut job = LayoutJob::simple_singleline(format!("{prefix}{}", e.name), FontId::proportional(13.0), FG);
                                job.wrap = TextWrapping::truncate_at_width(rect.width() - 20.0);
                                let g = ui.painter().layout_job(job);
                                ui.painter().galley(pos_left(rect, 10.0, g.size().y), g, if sel { Color32::WHITE } else { FG });
                                if sel && d.scroll {
                                    resp.scroll_to_me(None);
                                }
                                if e.is_dir && resp.double_clicked() {
                                    navigate_material = Some(e.path.clone());
                                } else if resp.clicked() {
                                    d.cursor = i;
                                    if !e.is_dir {
                                        toggle_material = Some((e.path.clone(), false));
                                    }
                                }
                            }
                        });
                        d.scroll = false;
                        if d.entries.is_empty() {
                            ui.add(Label::new(txt("Keine Eintr\u{e4}ge.", 12.0, DIM)).wrap());
                        }
                        if let Some(entry) = d.entries.get(d.cursor).filter(|e| !e.is_dir) {
                            if ui.button("Lokale Vorschau …").clicked() { preview_material = Some(entry.path.clone()); }
                        }
                        if !self.material.preview_text.is_empty() {
                            ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                                let th = md_egui::Theme::default();
                                md_egui::show_markdown(ui, &th, &self.material.preview_text);
                            });
                            if let Some(texture) = &self.material.preview_texture {
                                ui.add(egui::Image::new(texture).max_size(vec2(width - 40.0, 240.0)).maintain_aspect_ratio(true));
                            }
                        }
                        let count = d.selected.len();
                        ui.add(Label::new(txt(format!("{count} ausgew\u{e4}hlt \u{b7} Klick: Datei an/abw\u{e4}hlen \u{b7} Doppelklick: Ordner \u{f6}ffnen \u{b7} Backspace: Hoch"), 11.0, DIM)).wrap());
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if btn(ui, "Auswahl \u{fc}bernehmen", 13.0, vec2(14.0, 6.0), Vec2::ZERO, true, true).clicked() {
                                answer = Some(json!({"paths": d.selected.clone()}));
                            }
                            if btn(ui, "Abbrechen", 13.0, vec2(14.0, 6.0), Vec2::ZERO, false, true).clicked() {
                                answer = Some(json!({"cancelled": true}));
                            }
                        });
                    }
                }
            });
        if let Some(p) = navigate {
            self.browse_folder(p);
        }
        if let Some(path) = preview_material { self.preview_material(path); }
        if let Some(p) = navigate_material {
            self.browse_material(p);
        }
        if let Some((path, is_dir)) = toggle_material {
            self.toggle_material_entry(path, is_dir);
        }
        // Hintergrundklick ist absichtlich kein Abbruch (versehentliche Datenverluste vermeiden).
        let _ = modal;
        if explain_tokens {
            self.close_dialog(None);
            self.explain_token_info();
        } else if select_helper {
            self.close_dialog(None);
            self.configure_preview_helper();
        } else if let Some(a) = answer {
            self.close_dialog(Some(a));
        }
    }

    fn keyhelp_overlay(&mut self, ctx: &Context) {
        if !self.keyhelp {
            return;
        }
        let mut close = false;
        let width = 860.0f32.min(ctx.content_rect().width() - 24.0);
        let m = egui::Modal::new(Id::new("keyhelp"))
            .backdrop_color(Color32::from_black_alpha(0xaa))
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(1.0, ACC)).corner_radius(12).inner_margin(Margin::same(16)))
            .show(ctx, |ui| {
                ui.set_width(width - 32.0);
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.add(Label::new(RichText::new("Tastatursteuerung").size(16.0).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))));
                ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.7).show(ui, |ui| {
                    for (i, p) in KEYHELP.iter().enumerate() {
                        Frame::new().fill(BG).corner_radius(8).inner_margin(Margin::same(12)).show(ui, |ui| {
                            ui.set_width(width - 64.0);
                            ui.label(RichText::new(format!("{}", ["Navigation", "Eingabe & Befehle", "Weitere Shortcuts"].get(i).unwrap_or(&"Bedienung"))).size(14.0).color(ACC));
                            for line in p.split('\n') { ui.add(Label::new(txt(line, 14.0, FG)).wrap()); }
                        });
                        ui.add_space(8.0);
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if btn(ui, "Schließen (Esc)", 13.0, vec2(14.0, 6.0), Vec2::ZERO, false, true).clicked() {
                        close = true;
                    }
                });
            });
        if close || m.backdrop_response.clicked() {
            self.keyhelp = false;
            self.focus_prompt = true;
        }
    }

    fn editor_overlay(&mut self, ctx: &Context) {
        if !self.editor_open {
            return;
        }
        let screen = ctx.content_rect();
        let (w, h) = (screen.width() * 0.92, screen.height() * 0.92);
        let (mut apply, mut close) = (false, false);
        let focus = std::mem::take(&mut self.editor_focus);
        let mut editable = !matches!(self.editor_target, EditorTarget::InventoryFile { .. }) || self.editor_unlocked;
        let text = &mut self.editor_text;
        let size = self.store.output_size;
        let title = match &self.editor_target {
            EditorTarget::Prompt => "Eingabe bearbeiten".to_string(),
            EditorTarget::InventoryFile { path, .. } => format!("Datei bearbeiten: {path}"),
        };
        let apply_label = match &self.editor_target {
            EditorTarget::Prompt => "Übernehmen (Strg+S)",
            EditorTarget::InventoryFile { .. } => "Speichern (Strg+S)",
        };
        egui::Modal::new(Id::new("prompt-editor"))
            .backdrop_color(Color32::from_black_alpha(0xaa))
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(12).inner_margin(Margin::same(16)))
            .show(ctx, |ui| {
                ui.set_width(w - 32.0);
                ui.set_height(h - 32.0);
                ui.add(Label::new(RichText::new(title).size(16.0).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))));
                let eh = ui.available_height() - 48.0;
                Frame::new().fill(BG).corner_radius(6).inner_margin(Margin::same(8)).show(ui, |ui| {
                    ScrollArea::vertical().max_height(eh).auto_shrink([false, false]).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::multiline(text)
                                .id(Id::new("prompt-editor-text"))
                                .frame(Frame::NONE)
                                .desired_width(f32::INFINITY)
                                .desired_rows(20)
                                .interactive(editable)
                                .font(FontId::proportional(size))
                                .lock_focus(true),
                        );
                        if focus {
                            r.request_focus();
                        }
                    });
                });
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    apply = btn(ui, apply_label, 13.0, vec2(14.0, 6.0), Vec2::ZERO, true, editable).clicked();
                    close = btn(ui, "Schließen", 13.0, vec2(14.0, 6.0), Vec2::ZERO, false, true).clicked();
                    if matches!(self.editor_target, EditorTarget::InventoryFile { .. }) {
                        let st = IconStyle { bg: None, fg: if editable { ACC } else { DIM }, hover_fg: FG, icon: 18.0, radius: 5 };
                        if icons::icon_button(ui, vec2(32.0, 30.0), true, st, if editable { "Schloss schließen · Schreibschutz" } else { "Schloss öffnen · Bearbeitung erlauben" }, |p, r, c| icons::lock(p, r, c, editable)).clicked() { editable = !editable; }
                        ui.label(txt(if editable { "Bearbeitung erlaubt" } else { "Schreibgeschützt" }, 12.0, DIM));
                    }
                });
            });
        self.editor_unlocked = editable;
        if apply {
            self.apply_editor();
        } else if close {
            self.editor_open = false;
            self.editor_target = EditorTarget::Prompt;
            self.focus_prompt = true;
        }
    }

    fn toast_overlay(&mut self, ctx: &Context) {
        if self.toasts.is_empty() {
            return;
        }
        let screen = ctx.content_rect();
        let maxw = 420.0f32.min(screen.width() * 0.9);
        egui::Area::new(Id::new("toasts"))
            .order(Order::Tooltip)
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(screen.right_bottom() - vec2(12.0, 90.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                    for t in self.toasts.iter().rev() {
                        let c = match t.kind {
                            ToastKind::Error => ERR,
                            ToastKind::Warn => WARN,
                            ToastKind::Info => OK,
                        };
                        let r = Frame::new().fill(KBD_BG).corner_radius(8).inner_margin(Margin { left: 15, right: 12, top: 8, bottom: 8 }).show(ui, |ui| {
                            ui.set_max_width(maxw - 27.0);
                            ui.add(Label::new(txt(t.text.clone(), 13.0, FG)).wrap());
                        });
                        let rr = r.response.rect;
                        ui.painter().rect_filled(Rect::from_min_size(rr.min, vec2(3.0, rr.height())), CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 }, c);
                    }
                });
            });
    }
}

fn ctx_pointer_in(ctx: &Context, r: Rect) -> bool {
    r != Rect::NOTHING && ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| r.contains(p))
}

fn pos_left(rect: Rect, pad: f32, h: f32) -> Pos2 {
    Pos2::new(rect.left() + pad, rect.center().y - h / 2.0)
}

/// Tool- oder Shell-Block mit 3-px-Streifen und aufklappbarem Inhalt.
#[allow(clippy::too_many_arguments)]
fn block(ui: &mut Ui, maxw: f32, stripe: Color32, salt: (u64, usize), open: &mut bool, title: &str, state: &str, path: Option<&str>, out: &str, acts: &mut Vec<Act>) {
    let fr = Frame::new().fill(TOOL_BG).corner_radius(6).inner_margin(Margin { left: 13, right: 10, top: 6, bottom: 6 }).show(ui, |ui| {
        ui.set_width(maxw - 23.0);
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
        let resp = ui.interact(rect, Id::new(("blk", salt)), Sense::click());
        let p = ui.painter();
        tri(p, Pos2::new(rect.left() + 6.0, rect.center().y), *open, if resp.hovered() { FG } else { DIM });
        let mut right = rect.right();
        let mut ed: Option<Response> = None;
        if let Some(path) = path {
            let g = p.layout_no_wrap("↗ VSCodium".into(), FontId::proportional(12.0), DIM);
            let br = Rect::from_min_size(Pos2::new(right - g.size().x - 12.0, rect.top()), vec2(g.size().x + 12.0, 22.0));
            let r = ui.interact(br, Id::new(("blked", salt)), Sense::click()).on_hover_text(format!("In VSCodium öffnen: {path}"));
            ui.painter().rect_filled(br, 5, if r.hovered() { PICK_OPEN } else { BTN });
            ui.painter().galley(pos_left(br, 6.0, g.size().y), g, FG);
            right = br.left() - 8.0;
            ed = Some(r);
        }
        if !state.is_empty() {
            let g = ui.painter().layout_no_wrap(state.to_string(), FontId::proportional(12.0), DIM);
            right -= g.size().x;
            ui.painter().galley(Pos2::new(right, rect.center().y - g.size().y / 2.0), g, DIM);
            right -= 8.0;
        }
        let mut job = LayoutJob::simple_singleline(title.to_string(), FontId::monospace(13.0), FG);
        job.wrap = TextWrapping::truncate_at_width((right - rect.left() - 18.0).max(10.0));
        let g = ui.painter().layout_job(job);
        ui.painter().galley(pos_left(rect, 16.0, g.size().y), g, FG);
        let ed_click = ed.as_ref().is_some_and(|r| r.clicked());
        if ed_click {
            let p = path.unwrap_or_default().to_string();
            acts.push(Box::new(move |a| a.open_editor(Some(&p))));
        } else if resp.clicked() {
            *open = !*open;
        }
        if *open && !out.is_empty() {
            ScrollArea::vertical().id_salt(("blkout", salt)).max_height(300.0).stick_to_bottom(true).show(ui, |ui| {
                ui.add(Label::new(txt(out.trim_end().to_string(), 13.0, FG).family(egui::FontFamily::Monospace)).wrap());
            });
        }
    });
    let r = fr.response.rect;
    ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 }, stripe);
}

fn start_card(ui: &mut Ui, maxw: f32, card: &mut StartCard, salt: (u64, usize), acts: &mut Vec<Act>) {
    Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(12).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(maxw - 32.0);
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(38.0, 40.0), Sense::hover());
            icons::pi_logo(ui.painter(), r);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let v = if card.version.is_empty() { "pi".to_string() } else { format!("pi {}", card.version) };
                ui.add(Label::new(RichText::new(v).size(18.0).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))));
                ui.horizontal_wrapped(|ui| {
                    for (k, t) in [("Enter", "senden"), ("/", "Befehle"), ("!", "Shell"), ("Alt+N", "neue Session"), ("F1", "Hilfe")] {
                        Frame::new().fill(KBD_BG).corner_radius(4).inner_margin(Margin::symmetric(5, 1)).show(ui, |ui| {
                            ui.add(Label::new(txt(k, 12.0, FG).family(egui::FontFamily::Monospace)));
                        });
                        ui.add(Label::new(txt(t, 12.0, DIM)));
                    }
                });
            });
        });
        if let Some(u) = &card.update {
            let fr = Frame::new().fill(mix(WARN, 0.12, PANEL)).corner_radius(6).inner_margin(Margin { left: 12, right: 10, top: 6, bottom: 6 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(Label::new(txt(format!("Neue pi-Version {u} verfügbar."), 13.0, FG)));
                    if btn(ui, "pi update", 12.0, vec2(9.0, 3.0), Vec2::ZERO, false, true).clicked() {
                        acts.push(Box::new(|a| a.run_cli(vec!["update".into()])));
                    }
                });
            });
            let r = fr.response.rect;
            ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(4.0, r.height())), CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 }, WARN);
        }
        if card.groups.is_empty() {
            return;
        }
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
        let resp = ui.interact(rect, Id::new(("cardtoggle", salt)), Sense::click());
        tri(ui.painter(), Pos2::new(rect.left() + 5.0, rect.center().y), card.open, DIM);
        ui.painter().text(Pos2::new(rect.left() + 16.0, rect.center().y), Align2::LEFT_CENTER, "Geladen", FontId::proportional(13.0), if resp.hovered() { FG } else { DIM });
        if resp.clicked() {
            card.open = !card.open;
        }
        if !card.open {
            return;
        }
        for (gi, (name, color, chips)) in card.groups.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.add(Label::new(txt(name.clone(), 12.0, *color)));
                for (ci, c) in chips.iter().enumerate() {
                    let g = ui.painter().layout_no_wrap(c.label.clone(), FontId::proportional(12.0), FG);
                    let (r, _) = ui.allocate_exact_size(g.size() + vec2(16.0, 6.0), Sense::hover());
                    let sense = if matches!(c.action, ChipAction::None) { Sense::hover() } else { Sense::click() };
                    let resp = ui.interact(r, Id::new(("chip", salt, gi, ci)), sense);
                    let bg = mix(*color, if resp.hovered() && sense == Sense::click() { 0.25 } else { 0.12 }, PANEL);
                    ui.painter().rect_filled(r, 10, bg);
                    ui.painter().rect_stroke(r, 10, Stroke::new(1.0, mix(*color, 0.45, PANEL)), StrokeKind::Inside);
                    ui.painter().galley(r.min + vec2(8.0, 3.0), g, FG);
                    let resp = if c.title.is_empty() { resp } else { resp.on_hover_text(c.title.clone()) };
                    if resp.clicked() {
                        match &c.action {
                            ChipAction::Insert(t) => {
                                let t = t.clone();
                                acts.push(Box::new(move |a| a.set_prompt(t)));
                            }
                            ChipAction::Open(p) => {
                                let p = p.clone();
                                acts.push(Box::new(move |a| a.open_editor(Some(&p))));
                            }
                            ChipAction::None => {}
                        }
                    }
                }
            });
        }
    });
}
