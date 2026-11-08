//! „Bibliothek“: Extensions/Plugins, Skills sowie Agent- und Kontextdateien in einem eigenen
//! Fenster mit fester Vorschau (statt als Seitenleisten-Ansicht). Die Seitenleiste bleibt
//! damit ganz den Sessions vorbehalten (Liste bzw. Gliederung).
use egui::{vec2, Align, Color32, Context, Frame, Id, Key, Label, Layout, Margin, RichText, ScrollArea, Stroke};

use crate::app::{App, LIBRARY_CATS};
use crate::theme::*;
use crate::view::{btn, k, plain, txt};

#[derive(Default, Clone, Debug)]
pub struct LibraryState {
    /// Index in `LIBRARY_CATS`.
    pub cat: usize,
    pub cursor: Option<usize>,
    pub scroll: bool,
    pub filter: String,
}

pub fn cat_color(cat: usize) -> Color32 {
    match cat {
        0 => CAT_EXT,
        1 => CAT_SKILLS,
        _ => CAT_AGENTS,
    }
}

impl App {
    /// Bibliothek öffnen (optional direkt in einer Kategorie) und Inventar neu laden.
    pub fn open_library(&mut self, cat: Option<usize>) {
        let keep = self.library.take().unwrap_or_default();
        self.library = Some(LibraryState { cat: cat.unwrap_or(keep.cat).min(LIBRARY_CATS.len() - 1), ..Default::default() });
        self.preview = None;
        self.load_inventory();
    }

    pub fn close_library(&mut self) {
        self.library = None;
        self.preview = None;
        self.focus_prompt = true;
    }

    pub fn library_set_cat(&mut self, cat: usize) {
        if let Some(l) = &mut self.library {
            l.cat = cat.min(LIBRARY_CATS.len() - 1);
            l.cursor = None;
        }
        self.preview = None;
    }

    /// Einträge der gewählten Kategorie, gefiltert nach Name/Pfad.
    pub fn library_entries(&self) -> Vec<pi_backend::inventory::Entry> {
        let q = self.library.as_ref().map(|l| l.filter.trim().to_lowercase()).unwrap_or_default();
        self.visible_inventory().into_iter().filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q) || e.path.to_lowercase().contains(&q)).cloned().collect()
    }

    fn library_pick(&mut self, i: usize) {
        let entries = self.library_entries();
        if let Some(e) = entries.get(i) {
            if let Some(l) = &mut self.library {
                l.cursor = Some(i);
                l.scroll = true;
            }
            self.show_preview(&e.path, &e.name, false);
        }
    }

    /// Tasten bei offener Bibliothek: Esc schließt, ↑/↓ wählen (mit Vorschau), ←/→ Kategorie,
    /// Enter bearbeitet. Liefert `true`, wenn die Bibliothek offen ist (Tasten verbraucht).
    pub(crate) fn library_keys(&mut self, ctx: &Context) -> bool {
        let Some(l) = &self.library else { return false };
        let typing = ctx.memory(|m| m.focused().is_some());
        if plain(ctx, Key::Escape) || k(ctx, Key::L, true, false, false) {
            self.close_library();
            return true;
        }
        if typing {
            if plain(ctx, Key::ArrowDown) || plain(ctx, Key::Enter) {
                ctx.memory_mut(|m| if let Some(id) = m.focused() { m.surrender_focus(id) });
                self.library_pick(0);
            }
            return true;
        }
        let n = self.library_entries().len();
        let cat = l.cat;
        let cur = l.cursor;
        if plain(ctx, Key::ArrowRight) || plain(ctx, Key::Tab) {
            self.library_set_cat((cat + 1) % LIBRARY_CATS.len());
        } else if plain(ctx, Key::ArrowLeft) {
            self.library_set_cat((cat + LIBRARY_CATS.len() - 1) % LIBRARY_CATS.len());
        } else if n > 0 && plain(ctx, Key::ArrowDown) {
            self.library_pick(cur.map_or(0, |c| (c + 1).min(n - 1)));
        } else if n > 0 && plain(ctx, Key::ArrowUp) {
            self.library_pick(cur.map_or(0, |c| c.saturating_sub(1)));
        } else if n > 0 && plain(ctx, Key::Home) {
            self.library_pick(0);
        } else if n > 0 && plain(ctx, Key::End) {
            self.library_pick(n - 1);
        } else if plain(ctx, Key::Enter) {
            if let Some(e) = cur.and_then(|c| self.library_entries().get(c).cloned()) {
                self.open_inventory_file(&e.path);
            }
        }
        true
    }

    pub(crate) fn library_overlay(&mut self, ctx: &Context) {
        let Some(state) = self.library.clone() else { return };
        let screen = ctx.content_rect();
        let (w, h) = ((screen.width() - 40.0).clamp(320.0, 1040.0), (screen.height() - 60.0).clamp(240.0, 760.0));
        let color = cat_color(state.cat);
        let entries = self.library_entries();
        let mut close = false;
        let mut set_cat = None;
        let mut filter = state.filter.clone();
        egui::Modal::new(Id::new("library"))
            .backdrop_color(Color32::from_black_alpha(0x88))
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(1.0, color)).corner_radius(12).inner_margin(Margin::same(14)))
            .show(ctx, |ui| {
                ui.set_width(w - 28.0);
                ui.set_height(h - 28.0);
                ui.horizontal(|ui| {
                    ui.add(Label::new(RichText::new("Bibliothek").size(16.0).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))));
                    ui.add_space(10.0);
                    for (i, (name, _)) in LIBRARY_CATS.iter().enumerate() {
                        if btn(ui, name, 12.0, vec2(8.0, 3.0), vec2(0.0, 26.0), i == state.cat, true).on_hover_text("←/→ wechselt die Kategorie").clicked() {
                            set_cat = Some(i);
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if btn(ui, "×", 14.0, vec2(9.0, 2.0), vec2(0.0, 26.0), false, true).on_hover_text("Schließen (Esc, Alt+L)").clicked() {
                            close = true;
                        }
                        if btn(ui, "Neu laden", 11.0, vec2(7.0, 3.0), vec2(0.0, 26.0), false, !self.inventory_loading).clicked() {
                            self.load_inventory();
                        }
                    });
                });
                ui.add_space(6.0);
                let list_w = (ui.available_width() * 0.42).clamp(220.0, 380.0);
                let body_h = ui.available_height();
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(list_w, body_h), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(list_w);
                        ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filtern …").desired_width(list_w));
                        ui.add_space(4.0);
                        if self.inventory_loading {
                            ui.add(Label::new(txt("Lädt…", 12.0, DIM)));
                        }
                        let scroll = self.library.as_mut().map(|l| std::mem::take(&mut l.scroll)).unwrap_or(false);
                        ScrollArea::vertical().id_salt("library-list").auto_shrink([false, false]).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 5.0;
                            if entries.is_empty() && !self.inventory_loading {
                                ui.add(Label::new(txt("Keine Einträge gefunden.", 12.0, DIM)).wrap());
                            }
                            for (i, e) in entries.iter().enumerate() {
                                let selected = state.cursor == Some(i) || self.preview.as_ref().is_some_and(|p| p.path == e.path);
                                let r = self.inventory_card(ui, e, selected, color);
                                if r.clicked() {
                                    if let Some(l) = &mut self.library { l.cursor = Some(i); }
                                }
                                if selected && scroll {
                                    r.scroll_to_me(None);
                                }
                            }
                        });
                    });
                    ui.add_space(8.0);
                    let rest = ui.available_width();
                    ui.allocate_ui_with_layout(vec2(rest, body_h), Layout::top_down(Align::Min), |ui| {
                        ui.set_width(rest);
                        Frame::new().fill(BG).corner_radius(10).stroke(Stroke::new(1.0, mix(color, 0.4, PANEL))).inner_margin(Margin::same(12)).show(ui, |ui| {
                            ui.set_min_size(vec2(rest - 24.0, body_h - 24.0));
                            if self.preview.is_some() {
                                self.preview_panel(ui);
                            } else {
                                ui.add(Label::new(txt("Eintrag wählen (Klick, ↑/↓) – hier erscheint die Vorschau.\nDoppelklick oder Enter: intern bearbeiten (nur .md).", 12.5, DIM)).wrap());
                            }
                        });
                    });
                });
            });
        if let Some(l) = &mut self.library {
            if l.filter != filter {
                l.filter = filter;
                l.cursor = None;
            }
        }
        if let Some(c) = set_cat {
            self.library_set_cat(c);
        }
        if close {
            self.close_library();
        }
    }
}
