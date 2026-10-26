//! Darstellung der Multi-Backend-Ansicht: Backend-/Instanzleiste (rechte Seitenleiste),
//! Raster der gewählten Instanzen (statt des Einzel-Chats) und die Dialoge „Backend
//! hinzufügen/bearbeiten“ und „Neue Instanz“. Modell und Logik: `net.rs`.
use egui::{vec2, Align, Color32, Context, FontId, Frame, Id, Label, Layout, Margin, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui};
use pi_backend::instances::State;
use serde_json::json;

use crate::app::{App, ToastKind};
use crate::net::{short_cwd, AskKind, InstKey, NetItem};
use crate::theme::*;
use crate::view::{btn, txt, NET_PLACEHOLDER};

/// Aktionen aus der Zeichnung; werden nach dem Zeichnen angewandt (Borrow-frei).
enum Act {
    Select(InstKey, bool),
    Deselect(InstKey),
    Grid(bool),
    Enable(usize, bool),
    NewInst(String),
    EditBackend(Option<usize>),
    Send(InstKey),
    Abort(InstKey),
    Stop(InstKey),
    Restart(InstKey),
    Fetch(InstKey),
    Answer(InstKey, String, serde_json::Value),
    Hide,
}

fn bold(s: impl Into<String>, size: f32, c: Color32) -> RichText {
    RichText::new(s.into()).size(size).color(c).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))
}

/// Farbe und Text der Zustands-Pille.
pub fn state_look(s: State) -> (Color32, &'static str) {
    match s {
        State::Starting => (DIM, "startet"),
        State::Idle => (OK, "bereit"),
        State::Busy => (ACC, "arbeitet"),
        State::Waiting => (WARN, "wartet"),
        State::Exited => (ERR, "beendet"),
    }
}

fn pill(ui: &mut Ui, s: State) {
    let (c, t) = state_look(s);
    Frame::new().stroke(Stroke::new(1.0, c)).fill(alpha(c, 0.12)).corner_radius(8).inner_margin(Margin::symmetric(6, 0)).show(ui, |ui| {
        ui.label(txt(t, 11.0, c));
    });
}

fn badge(ui: &mut Ui, n: usize) {
    if n == 0 {
        return;
    }
    Frame::new().fill(WARN).corner_radius(8).inner_margin(Margin::symmetric(6, 0)).show(ui, |ui| {
        ui.label(bold(n.to_string(), 11.0, BG));
    })
    .response
    .on_hover_text(if n == 1 { "1 offene Rückfrage".to_string() } else { format!("{n} offene Rückfragen") });
}

fn link_dot(ui: &mut Ui, link: Option<bool>, err: bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    let c = match (link, err) {
        (Some(true), _) => OK,
        (Some(false), _) | (None, true) => ERR,
        (None, false) => DIM,
    };
    ui.painter().circle_filled(r.center(), 4.0, c);
    resp.on_hover_text(match link {
        Some(true) => "verbunden",
        Some(false) => "getrennt – Wiederaufbau läuft",
        None if err => "nicht verbunden",
        None => "verbindet …",
    });
}

impl App {
    /// Rechte Seitenleiste: Backends mit ihren Instanzen.
    pub(crate) fn net_bar(&mut self, ui: &mut Ui) {
        let mut acts: Vec<Act> = Vec::new();
        ui.horizontal(|ui| {
            ui.add(Label::new(bold("Backends", 14.0, FG)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if btn(ui, "×", 13.0, vec2(6.0, 2.0), vec2(24.0, 22.0), false, true).on_hover_text("Leiste ausblenden (Strg/Cmd+Shift+B)").clicked() {
                    acts.push(Act::Hide);
                }
            });
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let grid = self.net.grid;
            if btn(ui, "Einzel-Chat", 12.0, vec2(8.0, 3.0), Vec2Zero::z(), !grid, true).on_hover_text("Bisheriger Chat mit dem lokalen pi").clicked() {
                acts.push(Act::Grid(false));
            }
            if btn(ui, "Raster", 12.0, vec2(8.0, 3.0), Vec2Zero::z(), grid, true).on_hover_text("Gewählte Instanzen nebeneinander").clicked() {
                acts.push(Act::Grid(true));
            }
        });
        ui.add_space(4.0);
        ui.add(Label::new(txt("Klick wählt · Strg/Cmd+Klick: mehrere fürs Raster", 11.0, DIM)).wrap());
        ui.add_space(6.0);
        let avail = ui.available_height() - 64.0;
        ScrollArea::vertical().id_salt("net-bar").max_height(avail.max(80.0)).auto_shrink([false, true]).show(ui, |ui| {
            if self.net.backends.is_empty() {
                ui.add(Label::new(txt(if self.demo { "Demo-Modus: keine Backends verbunden." } else { "Backends werden geladen …" }, 12.0, DIM)).wrap());
            }
            let w = ui.available_width();
            for (bi, b) in self.net.backends.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    link_dot(ui, if b.cfg.enabled { b.link } else { None }, b.error.is_some());
                    ui.add(Label::new(bold(b.cfg.label.clone(), 13.0, if b.cfg.enabled { FG } else { DIM })).truncate());
                    ui.label(txt(if b.remote() { "remote" } else { "lokal" }, 10.5, DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_add = b.cfg.enabled && b.api.is_some();
                        if btn(ui, "+", 14.0, vec2(5.0, 1.0), vec2(24.0, 20.0), false, can_add).on_hover_text("Neue Instanz").clicked() {
                            acts.push(Act::NewInst(b.cfg.id.clone()));
                        }
                        let mut on = b.cfg.enabled;
                        if ui.checkbox(&mut on, "").on_hover_text(if on { "Eingeschaltet – klicken zum Trennen" } else { "Ausgeschaltet – klicken zum Verbinden" }).changed() {
                            acts.push(Act::Enable(bi, on));
                        }
                        if b.remote() && btn(ui, "✎", 12.0, vec2(5.0, 1.0), vec2(22.0, 20.0), false, true).on_hover_text("Backend bearbeiten").clicked() {
                            acts.push(Act::EditBackend(Some(bi)));
                        }
                    });
                });
                if b.cfg.enabled {
                    if let Some(e) = &b.error {
                        ui.add(Label::new(txt(e.clone(), 11.5, ERR)).wrap());
                    } else if b.link == Some(false) {
                        ui.add(Label::new(txt("Verbindung getrennt – Wiederaufbau läuft …", 11.5, WARN)).wrap());
                    }
                    if b.insts.is_empty() && b.error.is_none() {
                        ui.label(txt("keine Instanzen", 11.5, DIM));
                    }
                }
                for inst in &b.insts {
                    let key: InstKey = (b.cfg.id.clone(), inst.info.id.clone());
                    let sel = self.net.selected.contains(&key);
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 54.0), Sense::click());
                    let fill = if sel { mix(ACC, 0.22, PANEL) } else if resp.hovered() { CARD } else { PANEL };
                    ui.painter().rect_filled(rect, 6, fill);
                    if sel {
                        ui.painter().rect_stroke(rect, 6, Stroke::new(1.0, ACC), StrokeKind::Inside);
                    }
                    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(8.0, 4.0))).layout(Layout::top_down(Align::Min)));
                    c.spacing_mut().item_spacing.y = 1.0;
                    c.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            pill(ui, inst.info.state);
                            badge(ui, inst.info.asks);
                            if inst.info.harness != "pi" {
                                ui.label(txt(inst.info.harness.clone(), 10.5, DIM));
                            }
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.add(Label::new(bold(inst.info.name.clone(), 12.5, FG)).truncate());
                            });
                        });
                    });
                    c.add(Label::new(txt(short_cwd(&inst.info.cwd), 11.0, DIM)).truncate());
                    let last = inst.info.last.replace('\n', " ");
                    c.add(Label::new(txt(if last.is_empty() { "–".into() } else { last }, 11.0, alpha(FG, 0.75))).truncate());
                    let resp = resp.on_hover_text(format!("{}\n{}", inst.info.cwd, if sel { "Strg/Cmd+Klick: aus dem Raster nehmen" } else { "Klick: anzeigen · Strg/Cmd+Klick: zum Raster hinzufügen" }));
                    if resp.clicked() {
                        let multi = ui.input(|i| i.modifiers.command);
                        acts.push(Act::Select(key.clone(), multi));
                    }
                    let exited = inst.info.state == State::Exited;
                    let remote_backend = b.cfg.kind == "ssh";
                    resp.context_menu(|ui| {
                        if ui.button("Stoppen (Instanz entfernen)").clicked() {
                            acts.push(Act::Stop(key.clone()));
                            ui.close();
                        }
                        if remote_backend && ui.button("Arbeitsordner hierher holen (rsync)").on_hover_text("Ohne Build-Cache; x86-64-Programme werden übersprungen. Ziel: ~/pi-abruf").clicked() {
                            acts.push(Act::Fetch(key.clone()));
                            ui.close();
                        }
                        if ui.add_enabled(exited, egui::Button::new("Neu starten")).on_disabled_hover_text("Nur für beendete Instanzen").clicked() {
                            acts.push(Act::Restart(key.clone()));
                            ui.close();
                        }
                    });
                    ui.add_space(3.0);
                }
                ui.add_space(10.0);
            }
        });
        ui.add_space(4.0);
        if btn(ui, "+ Backend hinzufügen", 12.5, vec2(10.0, 4.0), Vec2Zero::z(), false, true).on_hover_text("pi-hub-Server mit URL und Token eintragen").clicked() {
            acts.push(Act::EditBackend(None));
        }
        ui.add_space(6.0);
        ui.label(txt("Chat mit Menschen: in Vorbereitung", 11.0, DIM)).on_hover_text(NET_PLACEHOLDER.join("\n\n"));
        self.net_apply(acts);
    }

    /// Raster der gewählten Instanzen (ersetzt den Einzel-Chat, solange aktiv).
    pub(crate) fn net_grid(&mut self, ui: &mut Ui) {
        let mut acts: Vec<Act> = Vec::new();
        let keys: Vec<InstKey> = self.net.selected.iter().filter(|k| self.net.inst(k).is_some()).cloned().collect();
        let full = ui.available_rect_before_wrap().shrink(8.0);
        if keys.is_empty() {
            ui.scope_builder(egui::UiBuilder::new().max_rect(full), |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(full.height() * 0.35);
                    ui.label(txt("Keine Instanz gewählt.", 15.0, FG));
                    ui.label(txt("In der Backend-Leiste rechts eine Instanz anklicken (Strg/Cmd+Klick für mehrere).", 12.5, DIM));
                    ui.add_space(8.0);
                    if btn(ui, "Zum Einzel-Chat", 13.0, vec2(12.0, 5.0), Vec2Zero::z(), false, true).clicked() {
                        acts.push(Act::Grid(false));
                    }
                });
            });
            self.net_apply(acts);
            return;
        }
        let gap = 8.0;
        let (cols, tile_h, _scroll) = crate::net::grid_dims(keys.len(), full.width(), full.height());
        let tile_w = (full.width() - gap * (cols as f32 - 1.0)) / cols as f32;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(full).layout(Layout::top_down(Align::Min)));
        ScrollArea::vertical().id_salt("net-grid").auto_shrink([false, false]).show(&mut child, |ui| {
            ui.spacing_mut().item_spacing = vec2(gap, gap);
            for row in keys.chunks(cols) {
                ui.horizontal(|ui| {
                    for k in row {
                        let (rect, _) = ui.allocate_exact_size(vec2(tile_w, tile_h - gap), Sense::hover());
                        let mut t = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)));
                        self.net_tile(&mut t, k, &mut acts);
                    }
                });
            }
        });
        self.net_apply(acts);
    }

    fn net_tile(&mut self, ui: &mut Ui, k: &InstKey, acts: &mut Vec<Act>) {
        let blabel = self.net.backend(&k.0).map(|b| b.cfg.label.clone()).unwrap_or_default();
        let linked = self.net.backend(&k.0).is_some_and(|b| b.link != Some(false) && b.api.is_some());
        let size = self.store.output_size.min(14.0);
        let th = md_egui::Theme { size, ..Default::default() };
        let rect = ui.max_rect();
        let Some(inst) = self.net.inst_mut(k) else { return };
        let state = inst.info.state;
        Frame::new().fill(PANEL).stroke(Stroke::new(1.0, if state == State::Waiting { WARN } else { BORDER })).corner_radius(10).inner_margin(Margin::same(8)).show(ui, |ui| {
            ui.set_min_size(rect.size() - vec2(16.0, 16.0));
            ui.set_max_size(rect.size() - vec2(16.0, 16.0));
            ui.spacing_mut().item_spacing.y = 4.0;
            // Kopf
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if btn(ui, "×", 12.0, vec2(5.0, 1.0), vec2(22.0, 20.0), false, true).on_hover_text("Aus dem Raster nehmen (Instanz läuft weiter)").clicked() {
                        acts.push(Act::Deselect(k.clone()));
                    }
                    if btn(ui, "Stopp", 11.5, vec2(6.0, 1.0), vec2(0.0, 20.0), false, linked).on_hover_text("Instanz beenden und entfernen").clicked() {
                        acts.push(Act::Stop(k.clone()));
                    }
                    if state == State::Exited && btn(ui, "Neustart", 11.5, vec2(6.0, 1.0), vec2(0.0, 20.0), true, linked).clicked() {
                        acts.push(Act::Restart(k.clone()));
                    }
                    pill(ui, state);
                    badge(ui, inst.info.asks);
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add(Label::new(bold(inst.info.name.clone(), 13.0, FG)).truncate()).on_hover_text(inst.info.cwd.clone());
                        ui.add(Label::new(txt(format!("{blabel} · {}", short_cwd(&inst.info.cwd)), 11.0, DIM)).truncate());
                    });
                });
            });
            let r = ui.min_rect();
            ui.painter().hline(r.x_range(), r.bottom() + 2.0, Stroke::new(1.0, BORDER));
            // Platz für Rückfragen und Eingabe unten freihalten.
            let ask_h: f32 = inst.chat.asks.iter().take(1).map(|a| if a.kind == AskKind::Select { 40.0 + 26.0 * a.options.len().min(4) as f32 } else { 76.0 }).sum();
            let hist_h = (ui.available_height() - 36.0 - ask_h).max(40.0);
            let chat = &mut inst.chat;
            ScrollArea::vertical().id_salt(("net-hist", &k.0, &k.1)).max_height(hist_h).min_scrolled_height(hist_h).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                let w = ui.available_width();
                if chat.items.is_empty() {
                    ui.label(txt(if chat.live { "Noch kein Verlauf." } else { "Verlauf wird geladen …" }, 12.0, DIM));
                }
                for (n, it) in chat.items.iter_mut().enumerate() {
                    match it {
                        NetItem::User(t) => {
                            Frame::new().fill(mix(ACC, 0.16, PANEL)).corner_radius(8).inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
                                ui.set_width(w - 20.0);
                                ui.add(Label::new(txt(t.clone(), size, FG)).wrap());
                            });
                        }
                        NetItem::Assistant { text, doc, done } => {
                            Frame::new().fill(BG).corner_radius(8).inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
                                ui.set_width(w - 20.0);
                                md_egui::show_doc(ui, &th, doc.get(text), ("net-a", &k.0, &k.1, n));
                                if !*done && state == State::Busy {
                                    // Schreibmarke (gezeichnet; das Blockzeichen fehlt in den Schriften)
                                    let (r, _) = ui.allocate_exact_size(vec2(7.0, size), Sense::hover());
                                    ui.painter().rect_filled(r, 1, ACC);
                                }
                            });
                        }
                        NetItem::Thinking(t) => {
                            let s: String = t.chars().take(160).collect();
                            ui.add(Label::new(RichText::new(s).italics().size(11.5).color(DIM)).truncate()).on_hover_text(t.clone());
                        }
                        NetItem::Tool { name, detail, done, err, .. } => {
                            let (mark, c) = if *err { ("✗", ERR) } else if *done { ("✓", OK) } else { ("…", ACC) };
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label(txt(mark, 12.0, c));
                                ui.label(RichText::new(name.clone()).monospace().size(11.5).color(FG));
                                ui.add(Label::new(RichText::new(detail.clone()).monospace().size(11.5).color(DIM)).truncate());
                            });
                        }
                        NetItem::Note { text, err } => {
                            ui.add(Label::new(txt(text.clone(), 11.5, if *err { ERR } else { DIM })).wrap());
                        }
                    }
                }
            });
            // Rückfrage inline (älteste zuerst)
            if let Some(a) = chat.asks.first_mut() {
                let aid = a.id.clone();
                Frame::new().fill(alpha(WARN, 0.10)).stroke(Stroke::new(1.0, WARN)).corner_radius(8).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let head = if a.title.is_empty() { "Rückfrage".to_string() } else { a.title.clone() };
                    ui.add(Label::new(bold(head, 12.5, FG)).truncate());
                    if !a.message.is_empty() {
                        ui.add(Label::new(txt(a.message.clone(), 11.5, FG)).truncate()).on_hover_text(a.message.clone());
                    }
                    match a.kind {
                        AskKind::Confirm => {
                            ui.horizontal(|ui| {
                                if btn(ui, "Ja", 12.0, vec2(12.0, 3.0), vec2(48.0, 0.0), true, linked).clicked() {
                                    acts.push(Act::Answer(k.clone(), aid.clone(), json!({"confirmed": true})));
                                }
                                if btn(ui, "Nein", 12.0, vec2(12.0, 3.0), vec2(48.0, 0.0), false, linked).clicked() {
                                    acts.push(Act::Answer(k.clone(), aid.clone(), json!({"confirmed": false})));
                                }
                            });
                        }
                        AskKind::Select => {
                            ScrollArea::vertical().id_salt(("net-ask", &k.0, &k.1)).max_height(26.0 * 4.0).show(ui, |ui| {
                                for o in &a.options {
                                    if btn(ui, o, 12.0, vec2(8.0, 3.0), vec2(ui.available_width(), 22.0), false, linked).clicked() {
                                        acts.push(Act::Answer(k.clone(), aid.clone(), json!({"value": o})));
                                    }
                                }
                            });
                            if btn(ui, "Abbrechen", 11.5, vec2(8.0, 2.0), Vec2Zero::z(), false, linked).clicked() {
                                acts.push(Act::Answer(k.clone(), aid.clone(), json!({"cancelled": true})));
                            }
                        }
                        AskKind::Input => {
                            ui.horizontal(|ui| {
                                let te = ui.add(egui::TextEdit::singleline(&mut a.text).desired_width(ui.available_width() - 150.0).font(FontId::proportional(12.5)));
                                let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if btn(ui, "OK", 12.0, vec2(10.0, 3.0), Vec2Zero::z(), true, linked).clicked() || (enter && linked) {
                                    acts.push(Act::Answer(k.clone(), aid.clone(), json!({"value": a.text})));
                                }
                                if btn(ui, "Abbrechen", 12.0, vec2(8.0, 3.0), Vec2Zero::z(), false, linked).clicked() {
                                    acts.push(Act::Answer(k.clone(), aid.clone(), json!({"cancelled": true})));
                                }
                            });
                        }
                    }
                });
            }
            // Eingabe
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let can_send = linked && !matches!(state, State::Exited | State::Starting);
                let te = ui.add_enabled(
                    can_send,
                    egui::TextEdit::singleline(&mut chat.input)
                        .id(Id::new(("net-input", &k.0, &k.1)))
                        .hint_text(if state == State::Exited { "Instanz ist beendet" } else { "Nachricht …" })
                        .desired_width(ui.available_width() - if state == State::Busy || state == State::Waiting { 150.0 } else { 74.0 })
                        .font(FontId::proportional(13.0)),
                );
                let enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if btn(ui, "Senden", 12.5, vec2(10.0, 4.0), Vec2Zero::z(), true, can_send && !chat.input.trim().is_empty()).clicked() || (enter && can_send) {
                    acts.push(Act::Send(k.clone()));
                    te.request_focus();
                }
                if matches!(state, State::Busy | State::Waiting) && btn(ui, "Abbruch", 12.5, vec2(8.0, 4.0), Vec2Zero::z(), false, linked).on_hover_text("Agentenlauf abbrechen").clicked() {
                    acts.push(Act::Abort(k.clone()));
                }
            });
        });
    }

    fn net_apply(&mut self, acts: Vec<Act>) {
        for a in acts {
            match a {
                Act::Select(k, m) => self.net.select(k, m),
                Act::Deselect(k) => self.net.selected.retain(|s| *s != k),
                Act::Grid(g) => {
                    self.net.grid = g;
                    if !g {
                        self.focus_prompt = true;
                    }
                }
                Act::Enable(i, on) => self.net.set_enabled(i, on),
                Act::NewInst(b) => self.net.open_new_inst(&b),
                Act::EditBackend(i) => self.net.open_form(i),
                Act::Send(k) => self.net.send_prompt(&k),
                Act::Abort(k) => self.net.abort(&k),
                Act::Stop(k) => {
                    let name = self.net.inst(&k).map(|i| i.info.name.clone()).unwrap_or_default();
                    self.ask(crate::app::DialogKind::Confirm, "Instanz stoppen?", &format!("„{name}“ wird beendet und aus der Liste entfernt."), vec![], "", move |a, r| {
                        if r.is_some_and(|r| r["confirmed"] == true) {
                            a.net.stop(&k);
                        }
                    });
                }
                Act::Restart(k) => self.net.restart(&k),
                Act::Fetch(k) => self.net.fetch_instance_dir(&k),
                Act::Answer(k, id, r) => self.net.answer(&k, &id, r),
                Act::Hide => self.toggle_net(),
            }
        }
        for (t, err) in std::mem::take(&mut self.net.notices) {
            self.toast(t, if err { ToastKind::Error } else { ToastKind::Info });
        }
    }

    /// Dialoge „Backend hinzufügen/bearbeiten“ und „Neue Instanz“.
    pub(crate) fn net_overlays(&mut self, ctx: &Context) {
        let screen = ctx.content_rect();
        let width = 460.0f32.min(screen.width() - 24.0);
        let frame = Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(12).inner_margin(Margin::same(16));
        if self.net.form.is_some() {
            let mut close = false;
            let mut submit = false;
            let mut delete: Option<usize> = None;
            let f = self.net.form.as_mut().unwrap();
            let m = egui::Modal::new(Id::new("net-backend-form")).backdrop_color(Color32::from_black_alpha(0xaa)).frame(frame).show(ctx, |ui| {
                ui.set_width(width - 32.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.add(Label::new(bold(if f.edit.is_some() { "Backend bearbeiten" } else { "Backend hinzufügen" }, 16.0, FG)));
                if f.edit.is_none() {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut f.ssh, true, "SSH-Server");
                        ui.radio_value(&mut f.ssh, false, "pi-hub per URL");
                    });
                }
                ui.add(Label::new(txt(if f.ssh { "SSH-Server (auch Tailscale-Name): copi richtet den pi-hub dort selbst ein und verbindet über einen Tunnel. Zugang nur per Schlüssel/ssh-agent – Passwörter fragt copi nie ab." } else { "pi-hub-Server: Adresse und Zugangstoken. Das Token wird nur in backends.json (Modus 0600) gespeichert." }, 12.0, DIM)).wrap());
                let field = |ui: &mut Ui, label: &str, v: &mut String, hint: &str, pw: bool| {
                    ui.label(txt(label, 12.0, FG));
                    ui.add(egui::TextEdit::singleline(v).hint_text(hint).password(pw).desired_width(f32::INFINITY).font(FontId::proportional(13.5)))
                };
                let r = field(ui, "Name", &mut f.name, "z. B. Server Büro", false);
                if f.edit.is_none() && f.name.is_empty() && !r.has_focus() && !ui.memory(|m| m.focused().is_some()) {
                    r.request_focus();
                }
                if f.ssh {
                    field(ui, "Host", &mut f.host, "z. B. 194.95.203.142 oder mein-server.tailnet.ts.net", false);
                    ui.add(Label::new(txt("Tailscale ist optional: Einfach die 100.x-Adresse oder den MagicDNS-Namen als Host eintragen – der Zugang läuft weiter per SSH (kein Hub-Port nach außen). Die bisherige Adresse bleibt als Rückfall nutzbar.", 11.0, DIM)).wrap());
                    field(ui, "Benutzer", &mut f.user, "z. B. medienhaus", false);
                    field(ui, "Schlüsseldatei (optional)", &mut f.identity_file, "~/.ssh/id_ed25519", false);
                } else {
                    field(ui, "URL", &mut f.url, "http://127.0.0.1:8787", false);
                    let hint = if f.has_token { "gespeichert – leer lassen für unverändert" } else { "Token des Hubs" };
                    field(ui, "Token", &mut f.token, hint, true);
                    field(ui, "oder Umgebungsvariable mit dem Token", &mut f.token_env, "z. B. PI_HUB_TOKEN", false);
                }
                if let Some(e) = &f.error {
                    ui.add(Label::new(txt(e.clone(), 12.5, ERR)).wrap());
                }
                ui.horizontal(|ui| {
                    if let Some(i) = f.edit {
                        if btn(ui, "Entfernen", 13.0, vec2(12.0, 5.0), Vec2Zero::z(), false, true).clicked() {
                            delete = Some(i);
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if btn(ui, "Speichern", 13.0, vec2(14.0, 5.0), vec2(80.0, 0.0), true, true).clicked() {
                            submit = true;
                        }
                        if btn(ui, "Abbrechen", 13.0, vec2(14.0, 5.0), Vec2Zero::z(), false, true).clicked() {
                            close = true;
                        }
                    });
                });
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
            });
            if m.should_close() {
                close = true;
            }
            if let Some(i) = delete {
                self.net.delete_backend(i);
                self.net.form = None;
            } else if submit {
                self.net.submit_form();
            } else if close {
                self.net.form = None;
            }
        }
        if self.net.new_inst.is_some() {
            let mut close = false;
            let mut create = false;
            let mut go: Option<Option<String>> = None;
            let mut enter: Option<String> = None;
            let blabel = self.net.new_inst.as_ref().and_then(|n| self.net.backend(&n.backend)).map(|b| b.cfg.label.clone()).unwrap_or_default();
            let n = self.net.new_inst.as_mut().unwrap();
            let m = egui::Modal::new(Id::new("net-new-inst")).backdrop_color(Color32::from_black_alpha(0xaa)).frame(frame).show(ctx, |ui| {
                ui.set_width(width - 32.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.add(Label::new(bold(format!("Neue Instanz auf „{blabel}“"), 16.0, FG)));
                ui.label(txt("Arbeitsordner", 12.0, FG));
                ui.horizontal(|ui| {
                    if btn(ui, "↑", 13.0, vec2(8.0, 2.0), vec2(26.0, 22.0), false, n.parent.is_some() && !n.loading).on_hover_text("Elternordner").clicked() {
                        go = Some(n.parent.clone());
                    }
                    ui.add(Label::new(RichText::new(if n.path.is_empty() { "…".into() } else { n.path.clone() }).monospace().size(12.0).color(FG)).truncate());
                });
                Frame::new().fill(BG).corner_radius(6).inner_margin(Margin::same(4)).show(ui, |ui| {
                    ScrollArea::vertical().id_salt("net-dirs").max_height(200.0).min_scrolled_height(200.0).auto_shrink([false, false]).show(ui, |ui| {
                        if n.loading {
                            ui.label(txt("Ordner werden geladen …", 12.0, DIM));
                        } else if n.dirs.is_empty() && n.error.is_none() {
                            ui.label(txt("keine Unterordner", 12.0, DIM));
                        }
                        for d in &n.dirs {
                            if ui.add(egui::Button::new(txt(format!("📁 {d}"), 12.5, FG)).frame(false)).clicked() {
                                enter = Some(d.clone());
                            }
                        }
                    });
                });
                ui.label(txt("Name (optional)", 12.0, FG));
                ui.add(egui::TextEdit::singleline(&mut n.name).hint_text("Ordnername").desired_width(f32::INFINITY));
                if n.harnesses.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.label(txt("Harness", 12.0, FG));
                        for h in n.harnesses.clone() {
                            ui.radio_value(&mut n.harness, h.to_string(), h);
                        }
                    });
                }
                if let Some(e) = &n.error {
                    ui.add(Label::new(txt(e.clone(), 12.5, ERR)).wrap());
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if n.busy { "Startet …" } else { "Starten" };
                    if btn(ui, label, 13.0, vec2(14.0, 5.0), vec2(80.0, 0.0), true, !n.busy && !n.loading && !n.path.is_empty()).clicked() {
                        create = true;
                    }
                    if btn(ui, "Abbrechen", 13.0, vec2(14.0, 5.0), Vec2Zero::z(), false, true).clicked() {
                        close = true;
                    }
                });
            });
            if m.should_close() {
                close = true;
            }
            if let Some(d) = enter {
                self.net.enter_dir(&d);
            } else if let Some(p) = go {
                self.net.load_dirs(p);
            } else if create {
                self.net.create_inst();
            } else if close {
                self.net.new_inst = None;
            }
        }
        for (t, err) in std::mem::take(&mut self.net.notices) {
            self.toast(t, if err { ToastKind::Error } else { ToastKind::Info });
        }
    }
}

/// Kurzschreibweise für `Vec2::ZERO` in `btn`-Aufrufen.
struct Vec2Zero;
impl Vec2Zero {
    fn z() -> egui::Vec2 {
        egui::Vec2::ZERO
    }
}
