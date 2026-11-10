//! Seitenansicht „Gliederung“: virtuelle, sortier- und einrückbare Sicht auf Sessions und
//! Markdown-Dateien mit benennbaren, auf-/zuklappbaren Kategorien (Bedienung nach AUGMOST,
//! aber kompakter). Modell und Ablage: `shared/outline.rs` (gemeinsam mit pi-lite).
//! Es werden nie echte Dateien verschoben, umbenannt oder gelöscht.
use egui::text::{LayoutJob, TextWrapping};
use egui::{pos2, vec2, Context, FontId, Id, Key, Label, LayerId, Order, Pos2, Rect, Response, ScrollArea, Sense, Stroke, StrokeKind, Ui};
use pi_backend::sessions::SessionInfo;
use std::time::{Duration, Instant, SystemTime};

use crate::app::{App, DialogKind, ToastKind};
use crate::icons;
use crate::shared_outline::{self as so, Op, Outline, Ref};
use crate::theme::*;
use crate::view::{k, plain, take, txt};

/// Index der Gliederung in `SIDE_VIEWS` (Schalter „Gliederung“ im Kontextmanager).
pub const OUTLINE_VIEW: usize = 1;
/// Einrückung je Ebene (AUGMOST: 26 px; hier schmaler für die 240-px-Seitenleiste).
pub const INDENT: f32 = 14.0;
pub const ROW_H: f32 = 22.0;
/// Kleine Keynote-artige Staffelung im Inhaltsmodus; tiefe Bäume behalten Kartenbreite.
pub fn view_indent(level: usize) -> f32 { level.min(5) as f32 * 9.0 }
/// Breite der Symbolspalte (Dreieck bzw. Punkt; zugleich Ziehgriff).
const SYM: f32 = 12.0;
const LOOSE_SESSIONS: &str = "sessions";
const LOOSE_MD: &str = "md";

#[derive(Default)]
pub struct OutlineState {
    pub o: Outline,
    pub cursor: Option<RowKey>,
    pub scroll: bool,
    pub drag: Option<Drag>,
    pub view: bool,
    last_check: Option<Instant>,
    mtime: Option<SystemTime>,
}

/// Stabile Zeilenkennung (überlebt Änderungen der Gliederung; Kategorien über `id`).
#[derive(Clone, Debug)]
pub enum RowKey {
    Tree(Ref),
    Group(&'static str),
    Loose(Ref),
}
impl PartialEq for RowKey {
    fn eq(&self, o: &Self) -> bool {
        match (self, o) {
            (RowKey::Tree(a), RowKey::Tree(b)) | (RowKey::Loose(a), RowKey::Loose(b)) => a.same(b),
            (RowKey::Group(a), RowKey::Group(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Drag {
    pub src: RowKey,
    pub label: String,
    /// Abstand des Griffpunkts zur Ebenenkante der Quelle.
    pub grab_dx: f32,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub key: RowKey,
    /// Index in `Outline::items` (nur Baumzeilen).
    pub item: Option<usize>,
    pub level: usize,
    pub label: String,
    pub tip: String,
    pub has_children: bool,
    pub collapsed: bool,
    pub missing: bool,
    pub remote: bool,
    /// Anzahl Nachfahren (Kategorie) bzw. Einträge (Gruppe).
    pub count: usize,
}

fn session_label(s: &SessionInfo) -> String {
    [&s.name, &s.title].into_iter().find(|t| !t.is_empty()).cloned().unwrap_or_else(|| "(leer)".into())
}
fn file_name(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}
fn is_md(p: &str) -> bool {
    p.rsplit('.').next().is_some_and(|e| e.eq_ignore_ascii_case("md")) && p.contains('.')
}

impl App {
    /// Lokale und Remote-Sessions (unabhängig von Herkunfts- und Bookmarkfilter), neueste zuerst.
    pub fn outline_sessions(&self) -> Vec<SessionInfo> {
        let mut v: Vec<SessionInfo> = self.sessions.iter().chain(self.net.remote_sessions.iter()).cloned().collect();
        v.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.backend.cmp(&b.backend)).then(a.path.cmp(&b.path)));
        v
    }
    /// Markdown-Dateien aus dem Inventar (alle Kategorien), nach Name.
    pub fn outline_md(&self) -> Vec<pi_backend::inventory::Entry> {
        let mut v: Vec<_> = self.inventory.iter().filter(|e| is_md(&e.path)).cloned().collect();
        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.path.cmp(&b.path)));
        v
    }

    /// Sichtbare Zeilen: Baum (ohne Nachfahren zugeklappter Einträge), danach die Gruppen
    /// „Ohne Einordnung“ für Sessions und md-Dateien, die noch nicht in der Gliederung stehen.
    pub fn outline_rows(&self) -> Vec<Row> {
        let o = &self.outline.o;
        let sessions = self.outline_sessions();
        let md = self.outline_md();
        let mut rows = Vec::new();
        for i in o.visible() {
            let it = &o.items[i];
            let end = o.subtree_end(i);
            let (label, tip, missing, remote) = match &it.r {
                Ref::Cat { name, .. } => (name.clone(), String::new(), false, false),
                Ref::Session { key } => match sessions.iter().find(|s| &s.key() == key) {
                    Some(s) => (session_label(s), format!("Session · {} · {}", crate::commands::fmt_date(s.modified), if s.is_remote() { self.net.backend_label(&s.backend) } else { pi_backend::tilde(&s.cwd) }), false, s.is_remote()),
                    None => (file_name(key), format!("Session nicht (mehr) gefunden: {key}"), true, key.starts_with("remote:")),
                },
                Ref::Md { path } => match md.iter().find(|e| &e.path == path) {
                    Some(e) => (e.name.clone(), pi_backend::tilde(&e.path), false, false),
                    None => (file_name(path), format!("Datei nicht im Inventar: {}", pi_backend::tilde(path)), true, false),
                },
            };
            rows.push(Row { key: RowKey::Tree(it.r.clone()), item: Some(i), level: it.level, label, tip, has_children: end > i + 1, collapsed: it.collapsed, missing, remote, count: end - i - 1 });
        }
        let loose_s: Vec<&SessionInfo> = sessions.iter().filter(|s| !o.contains(&Ref::Session { key: s.key() })).collect();
        let loose_m: Vec<&pi_backend::inventory::Entry> = md.iter().filter(|e| !o.contains(&Ref::Md { path: e.path.clone() })).collect();
        for (group, label, n) in [(LOOSE_SESSIONS, "Sessions ohne Einordnung", loose_s.len()), (LOOSE_MD, "md-Dateien ohne Einordnung", loose_m.len())] {
            let open = o.unsorted_open(group);
            rows.push(Row { key: RowKey::Group(group), item: None, level: 0, label: label.into(), tip: String::new(), has_children: n > 0, collapsed: !open, missing: false, remote: false, count: n });
            if !open {
                continue;
            }
            if group == LOOSE_SESSIONS {
                for s in &loose_s {
                    let tip = format!("Session · {} · {}", crate::commands::fmt_date(s.modified), if s.is_remote() { self.net.backend_label(&s.backend) } else { pi_backend::tilde(&s.cwd) });
                    rows.push(Row { key: RowKey::Loose(Ref::Session { key: s.key() }), item: None, level: 1, label: session_label(s), tip, has_children: false, collapsed: false, missing: false, remote: s.is_remote(), count: 0 });
                }
            } else {
                for e in &loose_m {
                    rows.push(Row { key: RowKey::Loose(Ref::Md { path: e.path.clone() }), item: None, level: 1, label: e.name.clone(), tip: pi_backend::tilde(&e.path), has_children: false, collapsed: false, missing: false, remote: false, count: 0 });
                }
            }
        }
        rows
    }

    /// Gliederung von der Platte laden (nicht im Demo-Modus).
    pub fn outline_load(&mut self) {
        if self.demo {
            return;
        }
        self.outline.mtime = so::mtime();
        let inventory: Vec<_> = self.outline_sessions().into_iter().map(|s| Ref::Session{key:s.key()}).chain(self.outline_md().into_iter().map(|e|Ref::Md{path:e.path})).collect();
        match so::prepare(&inventory) {
            Ok(o) => {
                self.outline.o = o;
                if let Ok(b)=crate::shared_bookmarks::read() { self.store.accept_shared(&b); }
            },
            Err(e) => self.toast(e, ToastKind::Error),
        }
    }

    /// Änderungen aus anderen Fenstern (copi, cocopi, pi-lite) höchstens einmal je Sekunde übernehmen.
    pub fn outline_refresh(&mut self) {
        if self.demo || self.side_view != OUTLINE_VIEW || self.outline.drag.is_some() {
            return;
        }
        if self.outline.last_check.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
            return;
        }
        self.outline.last_check = Some(Instant::now());
        self.outline_load();
    }

    /// Operation anwenden und speichern; der Cursor folgt dem betroffenen Eintrag.
    pub fn outline_op(&mut self, op: Op) {
        self.outline_op_at(self.outline.o.rev, op);
    }

    /// Dialoge behalten ihren Ausgangsstand: fremde Änderungen dürfen Indizes nicht umdeuten.
    fn outline_op_at(&mut self, rev: u64, op: Op) {
        if self.outline.o.rev != rev {
            self.toast(so::CONFLICT, ToastKind::Info);
            return;
        }
        let res = if self.demo {
            let mut b=self.store.shared_value();
            let r = so::apply_colored_model(&mut self.outline.o, &mut b, op);
            self.store.apply_shared(&b);
            Ok(r.1)
        } else {
            match so::apply_colored(self.outline.o.rev, op) {
                Ok((o, at)) => {
                    if let Ok(b)=crate::shared_bookmarks::read() { self.store.accept_shared(&b); }
                    self.outline.o = o;
                    self.outline.mtime = so::mtime();
                    Ok(at)
                }
                Err(e) => Err(e),
            }
        };
        match res {
            Ok(Some(at)) => {
                if let Some(it) = self.outline.o.items.get(at) {
                    self.outline.cursor = Some(RowKey::Tree(it.r.clone()));
                    self.outline.scroll = true;
                }
            }
            Ok(None) => {}
            Err(e) => {
                self.outline_load();
                self.toast(e, ToastKind::Error);
            }
        }
    }

    pub(crate) fn outline_new_color(&mut self) {
        self.ask(DialogKind::Input,"Neue Farboberkategorie","Name der neuen Bookmark-Farbe",vec![],"",|a,r| {
            if let Some(name)=r.and_then(|v|v["value"].as_str().map(str::to_owned)).filter(|s|!s.trim().is_empty()) {
                a.ask(DialogKind::Input,"Farbe wählen","Hex-Farbe, z. B. #74b9ff",vec![],"#74b9ff",move |a,r| {
                    if let Some(hex)=r.and_then(|v|v["value"].as_str().map(str::to_owned)) {
                        if !crate::shared_bookmarks::valid_hex(&hex) || a.store.palette_len()>=255 { a.toast("Gültige Farbe als #RRGGBB eingeben (max. 255 Farben).",ToastKind::Error); return; }
                        let i=a.store.palette_len() as u8;
                        a.store.extra_colors.push(hex);
                        a.store.set_category_name(i,&name);
                        a.save_store(); a.outline_load();
                    }
                });
            }
        });
    }

    fn outline_row_color(&self,row:&Row)->egui::Color32 {
        let r=match &row.key { RowKey::Tree(r)|RowKey::Loose(r)=>r, _=>return DIM };
        if let Some(c)=so::color_root(r) { return c.map(|i|self.store.palette_color(i)).unwrap_or(DIM); }
        if let Some(k)=so::ref_key(r) {
            if self.store.is_bookmark(k) { return self.store.palette_color(self.store.bookmark_color(k)); }
        }
        if let Some(i)=row.item {
            if let Some(c)=(0..=i).rev().find_map(|j|so::color_root(&self.outline.o.items[j].r)) { return c.map(|i|self.store.palette_color(i)).unwrap_or(DIM); }
        }
        DIM
    }

    /// View staffelt ganze Karten samt integrierten Gliederungspfeilen.
    fn outline_view_row(&mut self,ui:&mut Ui,row:&Row,selected:bool)->Response {
        let inset=view_indent(row.level);
        let width=(ui.available_width()-inset).max(80.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x=0.0;
            ui.add_space(inset);
            ui.allocate_ui_with_layout(vec2(width,0.0),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.set_width(width);
                self.outline_view_content(ui,row,selected)
            }).inner
        }).inner
    }

    /// Vollständige Karten samt Vorschau und Aktionen innerhalb der eingerückten Fläche.
    fn outline_view_content(&mut self,ui:&mut Ui,row:&Row,selected:bool)->Response {
        match &row.key {
            RowKey::Tree(Ref::Session{key})|RowKey::Loose(Ref::Session{key}) => {
                if let Some(s)=self.outline_sessions().into_iter().find(|s| &s.key()==key) {
                    let active=self.session_shown(&s);
                    let branch=if row.has_children { row.item.map(|i|(i,row.collapsed)) } else { None };
                    return self.session_card_outline(ui,&s,active,selected,branch,true);
                }
            }
            RowKey::Tree(Ref::Md{path})|RowKey::Loose(Ref::Md{path}) => {
                if let Some(e)=self.outline_md().into_iter().find(|e|&e.path==path) {
                    let c=self.outline_row_color(row);
                    return self.inventory_card(ui,&e,selected,c);
                }
            }
            _=>{}
        }
        self.outline_row(ui,row,selected)
    }

    fn outline_new_category(&mut self, after: Option<usize>, child: bool) {
        let title = if child { "Neue Unterkategorie" } else { "Neue Kategorie" };
        let rev = self.outline.o.rev;
        self.ask(DialogKind::Input, title, "Kategorien gibt es nur in der Gliederung; Dateien und Sessions bleiben, wo sie sind.", vec![], "", move |a, r| {
            if let Some(name) = r.and_then(|r| r["value"].as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()) {
                a.outline_op_at(rev, Op::AddCat { after, child, name });
            }
        });
    }

    fn outline_rename(&mut self, row: &Row) {
        match (&row.key, row.item) {
            (RowKey::Tree(Ref::Cat { name, .. }), Some(i)) => {
                let name = name.clone();
                if so::color_root(&self.outline.o.items[i].r).is_some() {
                    if let Some(Some(c))=so::color_root(&self.outline.o.items[i].r) {
                        self.ask(DialogKind::Input,"Farbkategorie benennen","",vec![],&name,move |a,r| {
                            if let Some(n)=r.and_then(|v|v["value"].as_str().map(str::to_owned)) { a.store.set_category_name(c,&n); a.save_store(); a.outline_load(); }
                        });
                    }
                    return;
                }
                let rev = self.outline.o.rev;
                self.ask(DialogKind::Input, "Kategorie umbenennen", "", vec![], &name, move |a, r| {
                    if let Some(n) = r.and_then(|r| r["value"].as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()) {
                        a.outline_op_at(rev, Op::Rename { i, name: n });
                    }
                });
            }
            (RowKey::Tree(Ref::Session { key }) | RowKey::Loose(Ref::Session { key }), _) => {
                if let Some(s) = self.outline_sessions().into_iter().find(|s| &s.key() == key) {
                    self.rename_session(&s);
                }
            }
            _ => self.toast("md-Dateien werden in der Gliederung nicht umbenannt (nur Kategorien und Sessions).", ToastKind::Info),
        }
    }

    fn outline_remove(&mut self, row: &Row) {
        let Some(i) = row.item else { return };
        if so::color_root(&self.outline.o.items[i].r).is_some() { return; }
        if matches!(row.key, RowKey::Tree(Ref::Cat { .. })) {
            let rev = self.outline.o.rev;
            let msg = if row.count > 0 { format!("„{}“\n\nDie {} Einträge darin rücken eine Ebene auf; nichts wird gelöscht.", row.label, row.count) } else { format!("„{}“", row.label) };
            self.ask(DialogKind::Confirm, "Kategorie entfernen?", &msg, vec![], "", move |a, r| {
                if r.and_then(|r| r["confirmed"].as_bool()).unwrap_or(false) {
                    a.outline_op_at(rev, Op::Remove { i });
                }
            });
        } else {
            // Nur der Verweis verschwindet; die Session bzw. Datei bleibt unberührt.
            self.outline_op(Op::Remove { i });
        }
    }

    /// Öffnen: Session laden, md-Datei im Editor, Kategorie/Gruppe auf- bzw. zuklappen.
    fn outline_open(&mut self, row: &Row) {
        match &row.key {
            RowKey::Group(g) => self.outline_op(Op::ToggleUnsorted { group: (*g).into() }),
            RowKey::Tree(Ref::Cat { .. }) => {
                if let Some(i) = row.item {
                    self.outline_op(Op::Toggle { i });
                }
            }
            RowKey::Tree(_) | RowKey::Loose(_) if row.missing => self.toast(row.tip.clone(), ToastKind::Info),
            RowKey::Tree(Ref::Session { key }) | RowKey::Loose(Ref::Session { key }) => {
                if let Some(s) = self.outline_sessions().into_iter().find(|s| &s.key() == key) {
                    self.open_session(&s, None);
                }
            }
            RowKey::Tree(Ref::Md { path }) | RowKey::Loose(Ref::Md { path }) => {
                let p = path.clone();
                self.open_inventory_file(&p);
            }
            RowKey::Loose(Ref::Cat { .. }) => {}
        }
    }

    /// Lose Einträge ans Ende der Gliederung (Ebene 0) übernehmen.
    fn outline_adopt(&mut self, r: Ref) {
        let n = self.outline.o.items.len();
        self.outline_op(Op::Insert { r, dest: n, level: 0 });
    }

    // ---------- Tastatur (Block 4b in `keys`, nur ohne Fokus) ----------

    /// Liefert `true`, wenn eine Taste verbraucht wurde.
    pub(crate) fn outline_keys(&mut self, ctx: &Context) -> bool {
        let rows = self.outline_rows();
        if rows.is_empty() {
            return false;
        }
        let n = rows.len();
        let cur = self.outline.cursor.as_ref().and_then(|c| rows.iter().position(|r| &r.key == c)).unwrap_or(0);
        let row = rows[cur].clone();
        let set = |a: &mut App, i: usize| {
            a.outline.cursor = Some(rows[i.min(n - 1)].key.clone());
            a.outline.scroll = true;
        };
        // Umschalt+Tab zuerst prüfen (wie AUGMOST).
        let shift_tab = take(ctx, |kk, m| kk == Key::Tab && m.shift && !m.alt && !m.command);
        let tab = !shift_tab && plain(ctx, Key::Tab);
        if self.outline.view && (tab || shift_tab || ctx.input(|i| i.modifiers.alt && (i.key_pressed(Key::ArrowUp)||i.key_pressed(Key::ArrowDown)))) { return true; }
        if shift_tab || tab {
            // egui hat Tab bereits als Fokuswechsel vorgemerkt – zurücknehmen.
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
            match (&row.key, row.item) {
                (RowKey::Tree(_), Some(i)) => self.outline_op(if tab { Op::Indent { i } } else { Op::Outdent { i } }),
                (RowKey::Loose(r), _) if tab => self.outline_adopt(r.clone()),
                _ => {}
            }
            return true;
        }
        if k(ctx, Key::ArrowUp, true, false, false) {
            if let Some(i) = row.item { self.outline_op(Op::Up { i }); }
            return true;
        }
        if k(ctx, Key::ArrowDown, true, false, false) {
            if let Some(i) = row.item { self.outline_op(Op::Down { i }); }
            return true;
        }
        if plain(ctx, Key::ArrowDown) {
            set(self, cur + 1);
        } else if plain(ctx, Key::ArrowUp) {
            set(self, cur.saturating_sub(1));
        } else if plain(ctx, Key::Home) {
            set(self, 0);
        } else if plain(ctx, Key::End) {
            set(self, n - 1);
        } else if plain(ctx, Key::ArrowLeft) {
            // Zuklappen; bei zugeklapptem Eintrag bzw. Blatt zum Elternteil.
            match (&row.key, row.item) {
                (RowKey::Tree(_), Some(i)) if row.has_children && !row.collapsed => self.outline_op(Op::SetCollapsed { i, collapsed: true }),
                (RowKey::Tree(_), Some(i)) => {
                    if let Some(p) = self.outline.o.parent(i) {
                        self.outline.cursor = Some(RowKey::Tree(self.outline.o.items[p].r.clone()));
                        self.outline.scroll = true;
                    }
                }
                (RowKey::Group(g), _) if !row.collapsed => self.outline_op(Op::ToggleUnsorted { group: (*g).into() }),
                (RowKey::Loose(_), _) => {
                    if let Some(g) = rows[..cur].iter().rposition(|r| matches!(r.key, RowKey::Group(_))) { set(self, g); }
                }
                _ => {}
            }
        } else if plain(ctx, Key::ArrowRight) {
            match (&row.key, row.item) {
                (RowKey::Tree(_), Some(i)) if row.collapsed => self.outline_op(Op::SetCollapsed { i, collapsed: false }),
                (RowKey::Group(g), _) if row.collapsed && row.has_children => self.outline_op(Op::ToggleUnsorted { group: (*g).into() }),
                _ if row.has_children && !row.collapsed => set(self, cur + 1),
                _ => {}
            }
        } else if plain(ctx, Key::Enter) || plain(ctx, Key::Space) {
            self.outline_open(&row);
        } else if plain(ctx, Key::F2) || plain(ctx, Key::R) {
            self.outline_rename(&row);
        } else if plain(ctx, Key::Delete) || plain(ctx, Key::Backspace) {
            self.outline_remove(&row);
        } else if plain(ctx, Key::N) {
            self.outline_new_category(row.item, false);
        } else if k(ctx, Key::N, false, false, true) {
            self.outline_new_category(row.item.filter(|_| matches!(row.key, RowKey::Tree(Ref::Cat { .. }))), true);
        } else {
            return false;
        }
        true
    }

    // ---------- Darstellung ----------

    pub(crate) fn outline_sidebar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_height(18.0);
            ui.add(Label::new(txt(if self.outline.view { "View · Karten mit Vorschau" } else { "Edit · ziehen, Tab/Umschalt+Tab" }, 11.0, DIM)).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                icons::info(ui.painter(), r, DIM);
                resp.on_hover_text(
                    "Gliederung: nur eine Sicht – Dateien und Sessions werden nie verschoben.\n\
                     Ziehen: einsortieren (waagrecht = Ebene) · Esc bricht ab\n\
                     ↑/↓ wählen · ←/→ zu-/aufklappen · Tab/Umschalt+Tab ein-/ausrücken\n\
                     Alt+↑/↓ verschieben · Enter öffnen · F2/R umbenennen\n\
                     N neue Kategorie · Umschalt+N Unterkategorie · Entf aus Gliederung nehmen",
                );
            });
        });
        ui.add_space(4.0);
        let sep = ui.available_rect_before_wrap();
        ui.painter().hline(sep.x_range(), sep.top(), Stroke::new(1.0, BORDER));
        ui.add_space(4.0);
        let rows = self.outline_rows();
        let cursor = self.outline.cursor.clone();
        let scroll = std::mem::take(&mut self.outline.scroll);
        let mut geo: Vec<(Rect, usize)> = Vec::new();
        let mut tree_bottom = None;
        ScrollArea::vertical().id_salt("outline").auto_shrink([false, false]).max_height((ui.available_height() - crate::view::FOOTER_H).max(40.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            if self.outline.o.items.is_empty() {
                ui.add(Label::new(txt("Noch keine Gliederung. „+ Kategorie“ anlegen und Einträge aus den Gruppen unten hineinziehen (oder Tab).", 11.5, DIM)).wrap());
                ui.add_space(4.0);
            }
            let mut sep_done = false;
            for (ri, row) in rows.iter().enumerate() {
                if !sep_done && row.item.is_none() {
                    sep_done = true;
                    tree_bottom = Some(ui.cursor().top());
                    ui.add_space(5.0);
                    let r = ui.available_rect_before_wrap();
                    ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, alpha(BORDER, 0.7)));
                    ui.add_space(5.0);
                }
                let resp = if self.outline.view { self.outline_view_row(ui, row, cursor.as_ref() == Some(&row.key)) } else { self.outline_row(ui, row, cursor.as_ref() == Some(&row.key)) };
                if row.item.is_some() {
                    geo.push((resp.rect, ri));
                }
                if cursor.as_ref() == Some(&row.key) && scroll {
                    resp.scroll_to_me(None);
                }
            }
            if tree_bottom.is_none() {
                tree_bottom = Some(ui.cursor().top());
            }
        });
        if !self.outline.view { self.outline_drag(ui, &rows, &geo, tree_bottom.unwrap_or(sep.top()), sep.left()); }
    }

    fn outline_row(&mut self, ui: &mut Ui, row: &Row, selected: bool) -> Response {
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::hover());
        let id = Id::new(("outline-row", match &row.key {
            RowKey::Tree(Ref::Cat { id, .. }) => format!("cat:{id}"),
            k => format!("{k:?}"),
        }));
        let draggable = !self.outline.view && !matches!(row.key, RowKey::Group(_)) && !matches!(&row.key,RowKey::Tree(r) if so::color_root(r).is_some());
        let resp = ui.interact(rect, id, if draggable { Sense::click_and_drag() } else { Sense::click() });
        let dragging_me = self.outline.drag.as_ref().is_some_and(|d| d.src == row.key);
        let tone = self.outline_row_color(row);
        let p = ui.painter();
        if selected || resp.hovered() || dragging_me {
            let t = if selected { 0.24 } else { 0.13 };
            p.rect_filled(rect, 5, mix(tone, t, PANEL));
        }
        if selected {
            p.rect_stroke(rect, 5, Stroke::new(1.0, mix(tone, 0.6, PANEL)), StrokeKind::Inside);
        }
        let x0 = rect.left() + 3.0 + if self.outline.view { 0.0 } else { row.level as f32 * INDENT };
        let sym = Rect::from_center_size(pos2(x0 + SYM / 2.0, rect.center().y), vec2(SYM, SYM));
        let is_cat = matches!(row.key, RowKey::Tree(Ref::Cat { .. }) | RowKey::Group(_));
        if matches!(&row.key,RowKey::Tree(r) if so::color_root(r).is_some()) {
            // Aufgeklappte Farboberkategorie deutlich hervorheben (kräftige Fläche, Rahmen,
            // breiter Streifen), damit klar ist, zu welcher Farbe die Karten darunter gehören.
            let open = row.has_children && !row.collapsed;
            let (fill, stripe) = if open { (0.42, 5.0) } else { (0.18, 3.0) };
            p.rect_filled(rect, 4, mix(tone, fill, PANEL));
            if open {
                p.rect_stroke(rect, 4, Stroke::new(1.2, tone), StrokeKind::Inside);
            }
            p.rect_filled(Rect::from_min_size(rect.min,vec2(stripe,rect.height())),2,tone);
        }
        let sym_col = if row.missing { alpha(DIM, 0.6) } else if is_cat { tone } else { tone };
        if row.has_children {
            icons::disclosure(p, sym, sym_col, !row.collapsed);
        } else if is_cat {
            // Leere Kategorie: hohles Dreieck (klappbar, sobald etwas darin liegt).
            icons::disclosure_empty(p, sym, alpha(DIM, 0.7));
        } else if matches!(row.key, RowKey::Tree(Ref::Md { .. }) | RowKey::Loose(Ref::Md { .. })) {
            // md-Datei: kleines Quadrat; Session: Punkt.
            p.rect_stroke(Rect::from_center_size(sym.center(), vec2(5.0, 6.0)), 1, Stroke::new(1.2, sym_col), StrokeKind::Inside);
        } else {
            p.circle_filled(sym.center(), 2.4, sym_col);
        }
        // Zähler bei zugeklappten Kategorien/Gruppen, sehr dezent rechts.
        let mut right = rect.right() - 6.0;
        if row.count > 0 && (row.collapsed || matches!(row.key, RowKey::Group(_))) {
            let g = p.layout_no_wrap(row.count.to_string(), FontId::proportional(10.0), alpha(DIM, 0.9));
            let gw = g.size().x;
            p.galley(pos2(right - gw, rect.center().y - g.size().y / 2.0), g, DIM);
            right -= gw + 6.0;
        }
        let tx = x0 + SYM + 5.0;
        let (size, color, family) = match &row.key {
            RowKey::Group(_) => (11.0, DIM, egui::FontFamily::Proportional),
            RowKey::Tree(Ref::Cat { .. }) => (12.5, FG, egui::FontFamily::Name(md_egui::BOLD_FAMILY.into())),
            _ if row.missing => (12.0, alpha(DIM, 0.8), egui::FontFamily::Proportional),
            _ => (12.0, FG, egui::FontFamily::Proportional),
        };
        let mut job = LayoutJob::simple_singleline(row.label.clone(), FontId::new(size, family), color);
        job.wrap = TextWrapping::truncate_at_width((right - tx).max(10.0));
        let g = p.layout_job(job);
        p.galley(pos2(tx, rect.center().y - g.size().y / 2.0), g, color);
        if resp.hovered() && self.outline.drag.is_none() {
            ui.ctx().set_cursor_icon(if draggable && sym.expand(3.0).contains(resp.hover_pos().unwrap_or_default()) { egui::CursorIcon::Grab } else { egui::CursorIcon::PointingHand });
        }
        let resp = if row.tip.is_empty() { resp } else { resp.on_hover_text_at_pointer(&row.tip) };
        let on_sym = resp.interact_pointer_pos().is_some_and(|p| p.x < sym.right() + 3.0);
        if resp.drag_started() && draggable {
            let grab = resp.interact_pointer_pos().map(|p| p.x - x0).unwrap_or(0.0);
            self.outline.drag = Some(Drag { src: row.key.clone(), label: row.label.clone(), grab_dx: grab });
            self.outline.cursor = Some(row.key.clone());
        } else if resp.clicked() {
            self.outline.cursor = Some(row.key.clone());
            if on_sym || matches!(row.key, RowKey::Group(_)) || (is_cat && row.has_children) {
                if row.has_children || matches!(row.key, RowKey::Group(_)) {
                    self.outline_open(row);
                }
            } else if !is_cat {
                match &row.key {
                    RowKey::Tree(Ref::Md { path }) | RowKey::Loose(Ref::Md { path }) if !row.missing => {
                        self.preview_anchor = rect;
                        self.show_preview(path, &row.label, false);
                    }
                    _ => self.outline_open(row),
                }
            }
        }
        if resp.double_clicked() && !is_cat && matches!(row.key, RowKey::Tree(Ref::Md { .. }) | RowKey::Loose(Ref::Md { .. })) {
            self.outline_open(row);
        }
        let row2 = row.clone();
        resp.context_menu(|ui| {
            ui.set_min_width(190.0);
            self.outline.cursor = Some(row2.key.clone());
            let mut act: Option<Box<dyn FnOnce(&mut App)>> = None;
            match (&row2.key, row2.item) {
                (RowKey::Tree(r), Some(i)) => {
                    if so::color_root(r).is_some() {
                        if ui.button("Neue Unterkategorie").clicked() { act=Some(Box::new(move |a|a.outline_new_category(Some(i),true))); }
                        if ui.button("Farbkategorie benennen …").clicked() { let rr=row2.clone(); act=Some(Box::new(move |a|a.outline_rename(&rr))); }
                        if let Some(f)=act { f(self); ui.close(); }
                        return;
                    }
                    let cats:Vec<_>=self.outline.o.items.iter().enumerate().filter_map(|(j,it)|if let Ref::Cat{name,..}=&it.r {Some((j,name.clone()))}else{None}).collect();
                    ui.menu_button("In Kategorie",|ui| {
                        for (cat,name) in cats {
                            if ui.button(name).clicked() { let r=r.clone(); act=Some(Box::new(move |a|a.outline_op(Op::IntoCat{r,cat}))); }
                        }
                    });
                    if !matches!(r, Ref::Cat { .. }) && ui.button("Öffnen").clicked() {
                        let rr = row2.clone();
                        act = Some(Box::new(move |a| a.outline_open(&rr)));
                    }
                    if let Ref::Session { key } = r {
                        if !key.starts_with("remote:") && ui.button("Kontext-Repair …").clicked() {
                            let key = key.clone();
                            act = Some(Box::new(move |a| if let Some(s) = a.outline_sessions().into_iter().find(|s| s.key() == key) { a.open_repair(&s) }));
                        }
                    }
                    if ui.button("Neue Kategorie danach").clicked() {
                        act = Some(Box::new(move |a| a.outline_new_category(Some(i), false)));
                    }
                    if matches!(r, Ref::Cat { .. }) {
                        if ui.button("Neue Unterkategorie").clicked() {
                            act = Some(Box::new(move |a| a.outline_new_category(Some(i), true)));
                        }
                        if ui.button("Umbenennen …").clicked() {
                            let rr = row2.clone();
                            act = Some(Box::new(move |a| a.outline_rename(&rr)));
                        }
                    }
                    ui.separator();
                    if ui.button("Einrücken (Tab)").clicked() { act = Some(Box::new(move |a| a.outline_op(Op::Indent { i }))); }
                    if ui.button("Ausrücken (Umschalt+Tab)").clicked() { act = Some(Box::new(move |a| a.outline_op(Op::Outdent { i }))); }
                    if ui.button("Nach oben (Alt+↑)").clicked() { act = Some(Box::new(move |a| a.outline_op(Op::Up { i }))); }
                    if ui.button("Nach unten (Alt+↓)").clicked() { act = Some(Box::new(move |a| a.outline_op(Op::Down { i }))); }
                    ui.separator();
                    let label = if matches!(r, Ref::Cat { .. }) { "Kategorie entfernen …" } else { "Aus Gliederung nehmen" };
                    if ui.button(label).clicked() {
                        let rr = row2.clone();
                        act = Some(Box::new(move |a| a.outline_remove(&rr)));
                    }
                }
                (RowKey::Loose(r), _) => {
                    if ui.button("Öffnen").clicked() {
                        let rr = row2.clone();
                        act = Some(Box::new(move |a| a.outline_open(&rr)));
                    }
                    if ui.button("In Gliederung übernehmen (Tab)").clicked() {
                        let r = r.clone();
                        act = Some(Box::new(move |a| a.outline_adopt(r)));
                    }
                    let cats: Vec<(usize, String, usize)> = self.outline.o.items.iter().enumerate().filter_map(|(i, it)| match &it.r { Ref::Cat { name, .. } => Some((i, name.clone(), it.level)), _ => None }).collect();
                    if !cats.is_empty() {
                        ui.menu_button("In Kategorie", |ui| {
                            for (i, name, level) in cats {
                                if ui.button(format!("{}{name}", "  ".repeat(level))).clicked() {
                                    let r = r.clone();
                                    act = Some(Box::new(move |a| a.outline_op(Op::IntoCat { r, cat: i })));
                                }
                            }
                        });
                    }
                }
                _ => {}
            }
            if let Some(f) = act {
                f(self);
                ui.close();
            }
        });
        resp
    }

    /// Ziehen & Ablegen: Einfügelinie zwischen Baumzeilen; Ebene folgt der waagrechten
    /// Zeigerposition (wie AUGMOST `drop_target`). Lose Einträge werden dabei übernommen.
    fn outline_drag(&mut self, ui: &mut Ui, rows: &[Row], geo: &[(Rect, usize)], tree_bottom: f32, left: f32) {
        let Some(state) = self.outline.drag.clone() else { return };
        let ctx = ui.ctx().clone();
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        let Some(pos) = ctx.pointer_latest_pos() else { return };
        let target = outline_drop_target(&self.outline.o, rows, geo, pos - vec2(state.grab_dx, 0.0), tree_bottom, left);
        let p = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("outline-drag")));
        let right = geo.first().map(|g| g.0.right()).unwrap_or(left + 200.0);
        if let Some(t) = &target {
            let x = left + 3.0 + t.level as f32 * INDENT + SYM / 2.0;
            p.line_segment([pos2(x + 4.0, t.y), pos2(right - 4.0, t.y)], Stroke::new(2.0, acc()));
            p.circle_stroke(pos2(x, t.y), 3.5, Stroke::new(1.6, acc()));
        }
        let g = p.layout_no_wrap(state.label.clone(), FontId::proportional(12.0), FG);
        let r = Rect::from_min_size(pos + vec2(14.0, 6.0), g.size() + vec2(12.0, 6.0));
        p.rect_filled(r, 5, alpha(PANEL, 0.95));
        p.rect_stroke(r, 5, Stroke::new(1.0, acc()), StrokeKind::Inside);
        p.galley(r.min + vec2(6.0, 3.0), g, FG);
        ctx.request_repaint();
        if ctx.input(|i| i.pointer.any_released()) {
            self.outline.drag = None;
            let Some(t) = target else { return };
            match &state.src {
                RowKey::Tree(r) => {
                    if let Some(src) = self.outline.o.find(r) {
                        self.outline_op(Op::MoveTo { src, dest: t.dest, level: t.level });
                    }
                }
                RowKey::Loose(r) => self.outline_op(Op::Insert { r: r.clone(), dest: t.dest, level: t.level }),
                RowKey::Group(_) => {}
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DropTarget {
    /// Einfügeindex in `Outline::items` (vor dem Entfernen der Quelle).
    pub dest: usize,
    pub level: usize,
    pub y: f32,
}

/// Ablageziel aus Zeigerposition (bereits um den Griffabstand korrigiert). Zeiger unterhalb
/// des Baums (über den Gruppen „ohne Einordnung“) legt am Ende ab.
pub fn outline_drop_target(o: &Outline, rows: &[Row], geo: &[(Rect, usize)], pos: Pos2, tree_bottom: f32, left: f32) -> Option<DropTarget> {
    let want = ((pos.x - (left + 3.0)) / INDENT).round().max(0.0) as usize;
    if geo.is_empty() {
        return Some(DropTarget { dest: 0, level: 0, y: tree_bottom });
    }
    let slot = geo.iter().position(|(r, _)| pos.y < r.center().y).unwrap_or(geo.len());
    let above = slot.checked_sub(1).map(|s| rows[geo[s].1].level);
    let below = geo.get(slot).map(|g| rows[g.1].level);
    let level = so::drop_level(above, below, want);
    let dest = geo.get(slot).and_then(|g| rows[g.1].item).unwrap_or(o.items.len());
    let y = match geo.get(slot) {
        Some(g) => g.0.top() - 0.5,
        None => geo[slot - 1].0.bottom() + 0.5,
    };
    Some(DropTarget { dest, level, y })
}

/// Wird zum Öffnen der Ansicht aufgerufen: Gliederung frisch laden.
pub fn on_enter(a: &mut App) {
    a.outline_load();
    if a.outline.cursor.is_none() {
        a.outline.cursor = a.outline_rows().first().map(|r| r.key.clone());
    }
}
