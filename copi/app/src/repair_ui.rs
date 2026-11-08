//! Dialog „Kontext-Repair“ für eine lokale Session (Backend: `pi_backend::repair`).
//! Analyse im Hintergrund; Anwenden hängt nur pi-eigene `context_edit`- bzw.
//! `session_info`-Einträge an (die Originale bleiben in der Datei, pi wertet beim
//! Kontextaufbau die letzte Bearbeitung aus). Rückgängig schneidet genau diesen Anhang ab,
//! solange die Datei danach unverändert ist.
use egui::{vec2, Align, Color32, Context, Frame, Id, Label, Layout, Margin, RichText, ScrollArea, Stroke};
use pi_backend::repair::{Applied, Report};
use pi_backend::sessions::SessionInfo;

use crate::app::{App, DialogKind, ToastKind};
use crate::commands::{fmt_date, token_short};
use crate::theme::*;
use crate::view::{btn, spinner, txt};

pub struct RepairState {
    pub session: SessionInfo,
    pub loading: bool,
    pub error: String,
    pub report: Option<Report>,
    /// Auswahl je Fund (`report.findings`).
    pub on: Vec<bool>,
    pub title: String,
    pub set_title: bool,
    pub applied: Option<Applied>,
    pub working: bool,
    pub seq: u64,
}

fn bold(s: impl Into<String>, size: f32) -> RichText {
    RichText::new(s).size(size).color(FG).family(egui::FontFamily::Name(md_egui::BOLD_FAMILY.into()))
}

impl App {
    /// Kontext-Repair für `s` öffnen und Analyse starten (nur lokale Sessions).
    pub fn open_repair(&mut self, s: &SessionInfo) {
        if s.is_remote() {
            self.toast("Kontext-Repair ist für Remote-Sessions noch nicht verfügbar (GEPLANT, NICHT IMPLEMENTIERT).", ToastKind::Info);
            return;
        }
        let seq = self.repair.as_ref().map_or(0, |r| r.seq) + 1;
        self.repair = Some(RepairState { session: s.clone(), loading: true, error: String::new(), report: None, on: Vec::new(), title: String::new(), set_title: false, applied: None, working: false, seq });
        self.preview = None;
        self.repair_analyze();
    }

    fn repair_analyze(&mut self) {
        let Some(r) = &mut self.repair else { return };
        r.loading = true;
        r.error.clear();
        r.seq += 1;
        let seq = r.seq;
        let path = r.session.path.clone();
        if self.demo {
            let report = demo_report(&path);
            self.repair_loaded(seq, Ok(report));
            return;
        }
        self.spawn(move || pi_backend::repair::analyze(&path, true), move |a, res: Result<Report, String>| a.repair_loaded(seq, res));
    }

    fn repair_loaded(&mut self, seq: u64, res: Result<Report, String>) {
        let Some(r) = &mut self.repair else { return };
        if r.seq != seq {
            return;
        }
        r.loading = false;
        match res {
            Ok(rep) => {
                r.on = rep.findings.iter().map(|f| f.default_on).collect();
                r.set_title = rep.name.is_empty() && rep.suggested_title.is_some();
                r.title = if rep.name.is_empty() { rep.suggested_title.clone().unwrap_or_default() } else { rep.name.clone() };
                r.report = Some(rep);
            }
            Err(e) => r.error = e,
        }
    }

    /// Ist die Session gerade die eigene, laufende Session?
    fn repair_is_open(&self, s: &SessionInfo) -> bool {
        !s.is_remote() && s.path == self.session_file()
    }

    fn repair_apply(&mut self) {
        let Some(r) = &self.repair else { return };
        let Some(rep) = &r.report else { return };
        if self.repair_is_open(&r.session) && self.busy {
            self.toast("pi arbeitet noch in dieser Session – erst abbrechen oder warten.", ToastKind::Warn);
            return;
        }
        let edits: Vec<(String, String)> = rep.findings.iter().zip(&r.on).filter(|(_, o)| **o).map(|(f, _)| (f.target.clone(), f.replacement.clone())).collect();
        let title = (r.set_title && !r.title.trim().is_empty() && r.title.trim() != rep.name).then(|| r.title.trim().to_string());
        if edits.is_empty() && title.is_none() {
            self.toast("Nichts ausgewählt.", ToastKind::Info);
            return;
        }
        let (path, len, leaf) = (rep.path.clone(), rep.len, rep.leaf.clone());
        let n = edits.len();
        let saved = rep.saved_tokens(&r.on);
        if self.demo {
            self.toast(format!("Demo: {n} Kürzung(en) würden angehängt (≈ {} Tokens).", token_short(Some(saved as f64))), ToastKind::Info);
            return;
        }
        if let Some(r) = &mut self.repair { r.working = true; }
        self.spawn(
            move || pi_backend::repair::apply(&path, len, &leaf, &edits, title.as_deref()),
            move |a, res: Result<Applied, String>| {
                if let Some(r) = &mut a.repair { r.working = false; }
                match res {
                    Ok(ap) => {
                        a.toast(format!("Kontext-Repair angewandt: {} Eintrag/Einträge angehängt, ≈ {} Tokens weniger.", ap.entries, token_short(Some(saved as f64))), ToastKind::Info);
                        if let Some(r) = &mut a.repair { r.applied = Some(ap); }
                        a.repair_after_write();
                        a.repair_analyze();
                    }
                    Err(e) => a.toast(e, ToastKind::Error),
                }
            },
        );
    }

    fn repair_undo(&mut self) {
        let Some(r) = &self.repair else { return };
        let Some(ap) = r.applied.clone() else { return };
        if self.repair_is_open(&r.session) && self.busy {
            self.toast("pi arbeitet noch in dieser Session – erst abbrechen oder warten.", ToastKind::Warn);
            return;
        }
        let path = r.session.path.clone();
        self.spawn(
            move || pi_backend::repair::undo(&path, &ap),
            |a, res: Result<(), String>| match res {
                Ok(()) => {
                    a.toast("Kontext-Repair rückgängig gemacht.", ToastKind::Info);
                    if let Some(r) = &mut a.repair { r.applied = None; }
                    a.repair_after_write();
                    a.repair_analyze();
                }
                Err(e) => a.toast(e, ToastKind::Error),
            },
        );
    }

    /// Nach dem Schreiben: Sessionliste auffrischen; die eigene offene Session neu laden,
    /// damit pi den neuen Leaf übernimmt (sonst hängt pi an den alten Zweig an).
    fn repair_after_write(&mut self) {
        let Some(s) = self.repair.as_ref().map(|r| r.session.clone()) else { return };
        if self.repair_is_open(&s) {
            self.cwd = s.cwd.clone();
            self.start(Some(s.path.clone()), None);
        } else {
            self.load_sessions();
        }
    }

    fn repair_export(&mut self) {
        let Some(rep) = self.repair.as_ref().and_then(|r| r.report.clone()) else { return };
        let stem = std::path::Path::new(&rep.path).file_stem().and_then(|s| s.to_str()).unwrap_or("session").to_string();
        let dir = if rep.cwd.is_empty() { pi_backend::home().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default() } else { rep.cwd.clone() };
        let prefill = format!("{}/{}.md", dir.trim_end_matches('/'), stem);
        self.ask(DialogKind::Input, "Session als Markdown exportieren", "Zielpfad der .md-Datei (wird nicht überschrieben, falls vorhanden):", vec![], &pi_backend::tilde(&prefill), move |a, r| {
            let Some(target) = r.and_then(|v| v["value"].as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()) else { return };
            let target = pi_backend::tilde_expand(&target);
            if !target.to_lowercase().ends_with(".md") {
                a.toast("Zielpfad muss auf .md enden.", ToastKind::Error);
                return;
            }
            if a.demo {
                a.toast("Demo: kein Export.", ToastKind::Info);
                return;
            }
            let rep2 = rep.clone();
            a.spawn(
                move || -> Result<String, String> {
                    let md = pi_backend::repair::export_md(&rep2.path, &rep2)?;
                    use std::io::Write;
                    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|e| format!("{target}: {e}"))?;
                    f.write_all(md.as_bytes()).map_err(|e| e.to_string())?;
                    Ok(target)
                },
                |a, res: Result<String, String>| match res {
                    Ok(p) => a.toast(format!("Exportiert: {}", pi_backend::tilde(&p)), ToastKind::Info),
                    Err(e) => a.toast(e, ToastKind::Error),
                },
            );
        });
    }

    /// `.md` als neue lokale Session importieren (Arbeitsordner = aktueller Workspace).
    pub fn import_md_session(&mut self) {
        let cwd = self.cwd.clone();
        self.ask(DialogKind::Input, "Markdown als neue Session importieren", &format!("Pfad der .md-Datei (max. 4 MB). Die neue Session gehört zu {}.", pi_backend::tilde(&cwd)), vec![], "", move |a, r| {
            let Some(src) = r.and_then(|v| v["value"].as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()) else { return };
            if a.demo {
                a.toast("Demo: kein Import.", ToastKind::Info);
                return;
            }
            a.spawn(
                move || pi_backend::repair::import_md(&src, &cwd),
                |a, res: Result<String, String>| match res {
                    Ok(p) => {
                        a.toast(format!("Importiert als neue Session: {}", pi_backend::tilde(&p)), ToastKind::Info);
                        a.load_sessions();
                    }
                    Err(e) => a.toast(e, ToastKind::Error),
                },
            );
        });
    }

    pub(crate) fn repair_overlay(&mut self, ctx: &Context) {
        if self.repair.is_none() {
            return;
        }
        let screen = ctx.content_rect();
        let (w, h) = ((screen.width() - 40.0).clamp(320.0, 980.0), (screen.height() - 60.0).clamp(260.0, 800.0));
        let open_here = self.repair.as_ref().is_some_and(|r| self.repair_is_open(&r.session));
        let busy = self.busy;
        let (mut close, mut apply, mut undo, mut export, mut reanalyze, mut delete) = (false, false, false, false, false, false);
        let Some(r) = &mut self.repair else { return };
        let label = [&r.session.name, &r.session.title].into_iter().find(|t| !t.is_empty()).cloned().unwrap_or("(leer)".into());
        egui::Modal::new(Id::new("repair"))
            .backdrop_color(Color32::from_black_alpha(0x99))
            .frame(Frame::new().fill(PANEL).stroke(Stroke::new(1.0, acc())).corner_radius(12).inner_margin(Margin::same(14)))
            .show(ctx, |ui| {
                ui.set_width(w - 28.0);
                ui.set_max_height(h - 28.0);
                ui.horizontal(|ui| {
                    ui.add(Label::new(bold("Kontext-Repair", 16.0)));
                    ui.add(Label::new(txt(&label, 13.0, DIM)).truncate());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if btn(ui, "×", 14.0, vec2(9.0, 2.0), vec2(0.0, 26.0), false, true).on_hover_text("Schließen (Esc)").clicked() {
                            close = true;
                        }
                        if btn(ui, "Neu analysieren", 11.0, vec2(7.0, 3.0), vec2(0.0, 26.0), false, !r.loading && !r.working).clicked() {
                            reanalyze = true;
                        }
                    });
                });
                ui.add(Label::new(txt(pi_backend::tilde(&r.session.path), 10.5, DIM)).truncate());
                ui.add_space(6.0);
                if r.loading {
                    ui.horizontal(|ui| {
                        spinner(ui);
                        ui.add(Label::new(txt("Session wird gelesen und analysiert (nur lesend) …", 12.0, DIM)));
                    });
                    return;
                }
                if !r.error.is_empty() {
                    ui.add(Label::new(txt(&r.error, 12.5, ERR)).wrap());
                    return;
                }
                let Some(rep) = r.report.clone() else { return };
                let saved = rep.saved_tokens(&r.on);
                ui.add(Label::new(txt(
                    format!(
                        "Kontext ≈ {} Tokens · {} Nachrichten · {} Kompaktierungen · {} frühere Bearbeitungen · zuletzt {}",
                        token_short(Some(rep.context_tokens as f64)),
                        rep.messages,
                        rep.compactions,
                        rep.context_edits,
                        if rep.last_activity.is_empty() { fmt_date(r.session.modified) } else { rep.last_activity.clone() }
                    ),
                    12.0,
                    FG,
                )).wrap());
                ui.add_space(4.0);
                let body_h = (ui.available_height() - 76.0).max(120.0);
                ScrollArea::vertical().id_salt("repair-body").max_height(body_h).auto_shrink([false, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    // Kontextfäule
                    ui.add(Label::new(bold("Kontextfäule", 13.0)));
                    if rep.rot.is_empty() {
                        ui.add(Label::new(txt("Keine Anzeichen.", 12.0, OK)));
                    }
                    for x in &rep.rot {
                        let (c, pre) = if x.level >= 2 { (ERR, "Blockierend") } else { (WARN, "Warnung") };
                        ui.add(Label::new(txt(format!("{pre}: {}", x.text), 12.0, c)).wrap());
                    }
                    if let Some(hint) = &rep.delete_hint {
                        ui.add_space(4.0);
                        Frame::new().fill(mix(WARN, 0.12, PANEL)).corner_radius(6).inner_margin(Margin::same(8)).show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.add(Label::new(txt(format!("Geringe Relevanz: {hint}"), 12.0, FG)).wrap());
                                if !open_here && btn(ui, "Löschen …", 11.0, vec2(7.0, 3.0), vec2(0.0, 24.0), false, true).clicked() {
                                    delete = true;
                                }
                            });
                        });
                    }
                    // Funde
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add(Label::new(bold(format!("Platzfresser ({})", rep.findings.len()), 13.0)));
                        if !rep.findings.is_empty() {
                            if ui.small_button("alle").clicked() { r.on.iter_mut().for_each(|o| *o = true); }
                            if ui.small_button("keine").clicked() { r.on.iter_mut().for_each(|o| *o = false); }
                        }
                    });
                    if rep.findings.is_empty() {
                        ui.add(Label::new(txt("Nichts Kürzbares gefunden (die letzten Werkzeugergebnisse bleiben stets unangetastet).", 12.0, DIM)).wrap());
                    }
                    for (i, f) in rep.findings.iter().enumerate() {
                        let Some(on) = r.on.get_mut(i) else { continue };
                        ui.horizontal(|ui| {
                            ui.checkbox(on, "");
                            ui.add(Label::new(txt(format!("−{}", token_short(Some(f.tokens_saved as f64))), 11.5, OK)));
                            ui.add(Label::new(txt(f.kind.label(), 11.0, acc())));
                            ui.add(Label::new(txt(&f.label, 12.0, FG)).truncate()).on_hover_text(format!("Ersatz im Modellkontext:\n{}", f.replacement.chars().take(700).collect::<String>()));
                        });
                    }
                    // Titel
                    ui.add_space(8.0);
                    ui.add(Label::new(bold("Titel", 13.0)));
                    ui.horizontal(|ui| {
                        let tip = if rep.name.is_empty() { "Noch kein Name gesetzt – Vorschlag aus Ziel bzw. erstem Prompt" } else { "Vorhandener Name; ändern setzt einen neuen session_info-Eintrag" };
                        ui.checkbox(&mut r.set_title, "setzen").on_hover_text(tip);
                        let resp = ui.add(egui::TextEdit::singleline(&mut r.title).desired_width(ui.available_width() - 8.0).char_limit(120));
                        if resp.changed() { r.set_title = !r.title.trim().is_empty() && r.title.trim() != rep.name; }
                    });
                    let list = |ui: &mut egui::Ui, head: &str, items: &[String], color: Color32| {
                        if items.is_empty() { return; }
                        ui.add_space(8.0);
                        ui.add(Label::new(bold(head, 13.0)));
                        for s in items {
                            ui.add(Label::new(txt(format!("• {}", s.replace('\n', " ")), 12.0, color)).wrap());
                        }
                    };
                    list(ui, "Status", &rep.status, FG);
                    list(ui, "ToDos", &rep.todos, FG);
                    list(ui, "Letzte Prompts", &rep.last_prompts.iter().map(|p| p.chars().take(240).collect()).collect::<Vec<String>>(), DIM);
                    list(ui, "Geänderte Dateien", &rep.modified_files.iter().map(|f| pi_backend::tilde(f)).collect::<Vec<_>>(), DIM);
                    list(ui, "Überschneidungen mit anderen Sessions", &rep.overlaps.iter().map(|o| format!("{} ({}) – {}", o.title, fmt_date(o.modified), o.shared.iter().map(|f| pi_backend::tilde(f)).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>(), FG);
                });
                ui.add_space(6.0);
                let sep = ui.available_rect_before_wrap();
                ui.painter().hline(sep.x_range(), sep.top(), Stroke::new(1.0, BORDER));
                ui.add_space(6.0);
                let note = if open_here && busy { "pi arbeitet gerade in dieser Session – Anwenden erst danach." } else if open_here { "Offene Session: wird nach dem Anwenden neu geladen." } else { "Nicht anwenden, solange die Session in einem anderen Fenster läuft." };
                ui.add(Label::new(txt(format!("Hängt nur pi-eigene context_edit-/session_info-Einträge an; Originale bleiben in der Datei. {note}"), 10.5, DIM)).wrap());
                ui.horizontal(|ui| {
                    let n = r.on.iter().filter(|o| **o).count();
                    let can = !r.working && (n > 0 || r.set_title) && !(open_here && busy);
                    if btn(ui, &format!("Anwenden ({n} · −{})", token_short(Some(saved as f64))), 12.5, vec2(10.0, 4.0), vec2(0.0, 28.0), true, can).clicked() {
                        apply = true;
                    }
                    if r.applied.is_some() && btn(ui, "Rückgängig", 12.0, vec2(9.0, 4.0), vec2(0.0, 28.0), false, !r.working).on_hover_text("Schneidet genau den eben angehängten Repair wieder ab (nur solange die Session nicht weiterlief)").clicked() {
                        undo = true;
                    }
                    if r.working { spinner(ui); }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if btn(ui, ".md importieren …", 11.5, vec2(8.0, 4.0), vec2(0.0, 28.0), false, true).on_hover_text("Markdown-Datei als neue Session anlegen").clicked() {
                            close = true;
                            export = false;
                            ui.ctx().data_mut(|d| d.insert_temp(Id::new("repair-import"), true));
                        }
                        if btn(ui, ".md exportieren …", 11.5, vec2(8.0, 4.0), vec2(0.0, 28.0), false, true).on_hover_text("Bericht und aktiven Kontext (mit Kürzungen) als Markdown speichern").clicked() {
                            export = true;
                        }
                    });
                });
            });
        let import = ctx.data_mut(|d| d.remove_temp::<bool>(Id::new("repair-import"))).unwrap_or(false);
        if reanalyze { self.repair_analyze(); }
        if apply { self.repair_apply(); }
        if undo { self.repair_undo(); }
        if export { self.repair_export(); }
        if delete {
            if let Some(s) = self.repair.take().map(|r| r.session) { self.remove_session(&s); }
        }
        if close || import {
            self.repair = None;
            self.focus_prompt = true;
        }
        if import { self.import_md_session(); }
    }
}

/// Synthetischer Bericht für Demo/Screenshots (keine echten Dateien).
pub fn demo_report(path: &str) -> Report {
    use pi_backend::repair::{Finding, Kind, Overlap, Rot};
    let f = |t: &str, kind, label: &str, tok| Finding { target: t.into(), kind, label: label.into(), tokens_saved: tok, replacement: "[Kontext-Repair: … gekürzt; das Original bleibt in der Sessiondatei.]".into(), default_on: true };
    Report {
        path: path.into(),
        cwd: "/demo/projekt".into(),
        len: 1,
        leaf: "demo".into(),
        entries: 412,
        messages: 274,
        compactions: 5,
        context_edits: 2,
        context_tokens: 148_000,
        findings: vec![
            f("a1", Kind::Large, "bash `cargo test` · 48211 Zeichen", 11_900),
            f("a2", Kind::DuplicateRead, "/demo/projekt/src/app.rs · später erneut gelesen", 7_400),
            f("a3", Kind::ErrorLoop, "Fehler edit von 4 · Could not find exact text", 1_100),
            f("a4", Kind::Image, "1 Bild(er) in Nutzernachricht · Screenshot der Sidebar", 1_600),
        ],
        rot: vec![Rot { level: 1, text: "Kontext ≈ 148k Tokens – Antworten werden unschärfer.".into() }, Rot { level: 1, text: "5 Kompaktierungen – ältere Details sind nur noch zusammengefasst.".into() }],
        last_prompts: vec!["Sidebar in Kontextmanager umbauen".into(), "Tests laufen lassen und committen".into()],
        status: vec!["Gliederung mit Farbkategorien fertig".into()],
        todos: vec!["Sortierung nach Ort".into(), "Kontext-Repair-Dialog".into()],
        modified_files: vec!["/demo/projekt/src/app.rs".into()],
        name: String::new(),
        suggested_title: Some("Sidebar als Kontextmanager".into()),
        delete_hint: None,
        overlaps: vec![Overlap { path: "/demo/sessions/2.jsonl".into(), title: "Recherche".into(), modified: 1_700_000_000, shared: vec!["/demo/projekt/src/app.rs".into()] }],
        last_activity: String::new(),
    }
}
