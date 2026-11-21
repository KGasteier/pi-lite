//! Regressionstests ohne Fenster oder pi-Prozess.
use crate::app::{App, DialogKind, EditorTarget, Job, SIDE_VIEWS};
use egui::{Event, Key, Modifiers, RawInput};
use serde_json::json;

fn demo() -> App { App::new(egui::Context::default(), true) }

#[test]
fn helper_selection_accepts_dialog_object_and_persists_choice() {
    let mut a = demo();
    a.models = vec![json!({"provider":"demo-provider", "id":"demo-helper", "name":"Demo helper"})];
    a.configure_preview_helper();
    let value = a.dialogs.front().unwrap().options[0].clone();
    a.close_dialog(Some(json!({"value":value})));
    assert_eq!(a.store.helper_provider, "demo-provider");
    assert_eq!(a.store.helper_model, "demo-helper");
}

fn prompt_frame(a: &mut App, events: Vec<Event>) {
    let ctx = a.ctx.clone();
    let mut out = ctx.run_ui(RawInput { events, ..Default::default() }, |ui| a.draw(ui));
    out.textures_delta.clear();
}

fn enter_event(modifiers: Modifiers) -> Event {
    Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers }
}

#[test]
fn prompt_history_arrows_traverse_multiline_entries() {
    let mut a = demo();
    crate::setup_ctx(&a.ctx);
    a.items.push(crate::app::Item::User("Erster\nPrompt".into()));
    a.items.push(crate::app::Item::User("Letzter\nPrompt".into()));
    a.set_prompt("Entwurf".into());
    prompt_frame(&mut a, vec![]);
    let arrow = |key| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    for (key, expected) in [(Key::ArrowUp, "Letzter\nPrompt"), (Key::ArrowUp, "Erster\nPrompt"), (Key::ArrowDown, "Letzter\nPrompt"), (Key::ArrowDown, "Entwurf")] {
        prompt_frame(&mut a, vec![arrow(key)]);
        assert_eq!(a.prompt, expected, "Nach {key:?}");
        assert!(a.ctx.memory(|m| m.has_focus(crate::view::prompt_id())), "History soll im Eingabefeld bleiben");
    }
}

#[test]
fn enter_sends_with_empty_ime_preedit() {
    let mut a = demo();
    crate::setup_ctx(&a.ctx);
    a.set_prompt("Testnachricht".into());
    prompt_frame(&mut a, vec![]);
    prompt_frame(&mut a, vec![Event::Ime(egui::ImeEvent::Preedit { text: String::new(), active_range_chars: None }), enter_event(Modifiers::NONE)]);
    assert!(a.prompt.is_empty(), "Enter muss bei inaktiver IME senden");
}

#[test]
fn alt_enter_inserts_newline_in_prompt() {
    let mut a = demo();
    crate::setup_ctx(&a.ctx);
    a.set_prompt("Testnachricht".into());
    prompt_frame(&mut a, vec![]);
    prompt_frame(&mut a, vec![enter_event(Modifiers::ALT)]);
    assert!(a.prompt.contains('\n'), "Alt+Enter muss umbrechen");
}

#[test]
fn active_ime_preedit_does_not_submit() {
    let mut a = demo();
    crate::setup_ctx(&a.ctx);
    a.set_prompt("Testnachricht".into());
    prompt_frame(&mut a, vec![]);
    prompt_frame(&mut a, vec![Event::Ime(egui::ImeEvent::Preedit { text: "候補".into(), active_range_chars: None }), enter_event(Modifiers::NONE)]);
    assert!(!a.prompt.is_empty());
    assert!(a.prompt_ime_active);
}

#[test]
fn active_job_keeps_unsent_prompt() {
    let mut a = demo();
    a.active_job = Some(Job::Cli("test".into()));
    a.prompt = "Bitte warten".into();
    a.submit();
    assert_eq!(a.prompt, "Bitte warten");
}

#[test]
fn active_cli_blocks_session_restart() {
    let mut a = demo();
    a.active_job = Some(Job::Cli("test".into()));
    let epoch = a.log_epoch;
    a.new_session();
    assert_eq!(a.log_epoch, epoch);
}

#[test]
fn confirm_arrow_then_enter_selects_no() {
    let mut a = demo();
    a.ask(DialogKind::Confirm, "Test", "", vec![], "", |a, value| {
        a.prompt = value.unwrap()["confirmed"].to_string();
    });
    let ctx = a.ctx.clone();
    crate::setup_ctx(&ctx);
    let events = [Key::ArrowRight, Key::Enter].into_iter().map(|key| Event::Key {
        key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE,
    }).collect();
    let mut out = ctx.run_ui(RawInput { events, ..Default::default() }, |ui| a.draw(ui));
    out.textures_delta.clear();
    assert_eq!(a.prompt, json!(false).to_string());
    assert!(a.dialogs.is_empty());
}

#[test]
fn set_side_view_switches_label_and_clears_preview() {
    let mut a = demo();
    a.preview = None;
    a.set_side_view(1);
    assert_eq!(a.side_view, 1);
    assert_eq!(SIDE_VIEWS[a.side_view], "Extensions / Plugins");
    // Im Demo-Modus liefert `load_inventory` keine echten Dateien, löscht aber die Liste.
    assert!(a.inventory.is_empty());
}

#[test]
fn choose_pick_side_view_updates_state() {
    let mut a = demo();
    a.choose_pick(crate::app::PickerKind::SideView, 2);
    assert_eq!(a.side_view, 2);
}

#[test]
fn editor_reference_link_opens_editor_without_toast_stub() {
    let mut a = demo();
    let before = a.toasts.len();
    a.handle_markdown_link("src/app.rs:42");
    // Im Demo-Modus zeigt `open_editor_at` einen Hinweis-Toast statt echten Editor zu öffnen,
    // aber *nicht* den generischen "Verweis: ..."-Platzhalter.
    assert!(a.toasts.len() > before);
    assert!(!a.toasts.iter().any(|t| t.text.starts_with("Verweis:")));
}

#[test]
fn editor_reference_non_file_shows_placeholder_toast() {
    let mut a = demo();
    a.handle_markdown_link("irgendein Hinweis ohne Dateibezug");
    assert!(a.toasts.iter().any(|t| t.text.starts_with("Verweis:")));
}

#[test]
fn apply_editor_prompt_target_sets_prompt() {
    let mut a = demo();
    a.editor_target = EditorTarget::Prompt;
    a.editor_text = "neuer Text".into();
    a.editor_open = true;
    a.apply_editor();
    assert!(!a.editor_open);
    assert_eq!(a.prompt, "neuer Text");
    assert!(matches!(a.editor_target, EditorTarget::Prompt));
}

#[test]
fn choose_folder_opens_folder_dialog_and_browses() {
    let mut a = demo();
    a.cwd = std::env::temp_dir().to_string_lossy().to_string();
    a.choose_folder();
    assert_eq!(a.dialogs.front().map(|d| d.kind), Some(DialogKind::Folder));
    // Im Demo-Modus wird `browse_folder` synchron (deferred+drain) ausgeführt.
    a.drain();
    let d = a.dialogs.front().expect("Folder-Dialog sollte offen sein");
    assert!(!d.text.is_empty());
}

#[test]
fn list_cursor_navigates_sessions_with_arrow_keys() {
    let mut a = demo();
    assert!(!a.visible_sessions().is_empty(), "Demo-Modus sollte Beispiel-Sessions anlegen");
    let ctx = a.ctx.clone();
    crate::setup_ctx(&ctx);
    let events = vec![Event::Key { key: Key::ArrowDown, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
    let mut out = ctx.run_ui(RawInput { events, ..Default::default() }, |ui| a.draw(ui));
    out.textures_delta.clear();
    // Start bei Index 0 (unwrap_or), ArrowDown bewegt zu Index 1.
    assert_eq!(a.list_cursor, Some(1));
}

#[test]
fn prompt_recall_is_scoped_and_leaves_original_draft() {
    let mut a = demo();
    a.state["sessionFile"] = json!("/demo/first.jsonl");
    a.items = vec![crate::app::Item::User("Erster Prompt".into()), crate::app::Item::User("Letzter Prompt".into())];
    a.set_prompt("Entwurf".into());
    a.recall(-1);
    assert_eq!(a.prompt, "Letzter Prompt");
    a.recall(-1);
    assert_eq!(a.prompt, "Erster Prompt");
    a.recall(-1);
    assert_eq!(a.prompt, "Erster Prompt");
    a.recall(1);
    a.recall(1);
    assert_eq!(a.prompt, "Entwurf");
    a.recall(-1);
    a.state["sessionFile"] = json!("/demo/second.jsonl");
    a.items = vec![crate::app::Item::User("Nur zweite Session".into())];
    a.recall(-1);
    assert_eq!(a.prompt, "Nur zweite Session");
    a.recall(1);
    assert_eq!(a.prompt, "Entwurf");
}

#[test]
fn closed_material_dialog_discards_pending_preview() {
    let mut a = demo();
    a.ask(DialogKind::Material, "Test", "", vec![], "", |_, _| {});
    a.preview_material("/nonexistent-test-preview.png".into());
    let seq = a.material.preview_seq;
    a.close_dialog(None);
    assert!(a.material.preview_seq > seq);
    assert!(a.material.preview_path.is_none());
    assert!(a.material.preview_texture.is_none());
    a.pump();
    assert!(a.material.preview_text.is_empty());
}

// ---------- Material (Stufe 3) ----------

#[test]
fn material_plus_slash_command_is_rejected() {
    let mut a = demo();
    a.material.items.push(crate::material_ui::MaterialItem { value: "/tmp/a.png".into(), url: false, image: None });
    a.prompt = "/neu".into();
    let before = a.toasts.len();
    a.submit();
    assert!(a.toasts.len() > before);
    assert!(a.toasts.last().unwrap().text.contains("nicht mit einem Befehl"));
    // Material und Eingabe bleiben erhalten, nichts wurde gesendet.
    assert_eq!(a.material.items.len(), 1);
    assert_eq!(a.prompt, "/neu");
}

#[test]
fn material_plus_busy_agent_is_rejected() {
    let mut a = demo();
    a.material.items.push(crate::material_ui::MaterialItem { value: "/tmp/a.png".into(), url: false, image: None });
    a.busy = true;
    a.prompt = "Was zeigt das Bild?".into();
    let before = a.toasts.len();
    a.submit();
    assert!(a.toasts.len() > before);
    assert!(a.toasts.last().unwrap().text.contains("erst senden, wenn der Agent bereit ist"));
    assert_eq!(a.material.items.len(), 1);
    assert_eq!(a.prompt, "Was zeigt das Bild?");
}

#[test]
fn material_items_capped_at_twelve_and_deduplicated() {
    let mut a = demo();
    let twelve: Vec<_> = (0..12).map(|i| crate::material_ui::MaterialItem { value: format!("/tmp/f{i}.txt"), url: false, image: None }).collect();
    a.merge_material(twelve.clone());
    assert_eq!(a.material.items.len(), 12);
    // Ein weiteres Element über das Limit hinaus wird abgelehnt; die Liste bleibt bei 12.
    a.merge_material(vec![crate::material_ui::MaterialItem { value: "/tmp/f12.txt".into(), url: false, image: None }]);
    assert_eq!(a.material.items.len(), 12);
    // Ein bereits enthaltener Pfad wird nicht doppelt aufgenommen.
    a.material.items.clear();
    a.merge_material(vec![twelve[0].clone()]);
    a.merge_material(vec![twelve[0].clone()]);
    assert_eq!(a.material.items.len(), 1);
}

#[test]
fn remove_material_is_blocked_while_preparing() {
    let mut a = demo();
    let item = crate::material_ui::MaterialItem { value: "/tmp/a.png".into(), url: false, image: None };
    a.material.items.push(item.clone());
    a.material.preparing = true;
    a.remove_material(item);
    assert_eq!(a.material.items.len(), 1, "Während der Vorbereitung darf nichts entfernt werden");
    a.material.preparing = false;
    a.remove_material(a.material.items[0].clone());
    assert!(a.material.items.is_empty());
}

#[test]
fn choose_material_opens_select_dialog() {
    let mut a = demo();
    a.choose_material();
    assert_eq!(a.dialogs.front().map(|d| d.kind), Some(DialogKind::Select));
}

#[test]
fn empty_prompt_with_material_does_not_submit() {
    let mut a = demo();
    a.material.items.push(crate::material_ui::MaterialItem { value: "/tmp/a.png".into(), url: false, image: None });
    a.prompt = "   ".into();
    a.submit();
    // Leere Eingabe bricht vor jeder Materialprüfung ab.
    assert_eq!(a.material.items.len(), 1);
    assert_eq!(a.prompt, "   ");
}

// ---------- Schriften (eingebettet, kein Datei-I/O) ----------

#[test]
fn sidebar_temporarily_excludes_other_harnesses() {
    assert_eq!(SIDE_VIEWS.len(), 4);
    assert!(!SIDE_VIEWS.iter().any(|s| s.contains("Harness")));
    assert_eq!(crate::app::side_view_category(4), None);
}

#[test]
fn fonts_cover_ui_symbols_without_filesystem_access() {
    // `setup_ctx` darf ausschließlich `include_bytes!`-eingebettete Schriften verwenden
    // (Figtree + die von epaint/egui mitgelieferten `default_fonts`-Schriften Hack/
    // Ubuntu-Light/NotoEmoji/emoji-icon-font) – kein Zugriff auf `/usr/share/fonts` o.ä.,
    // damit das Erscheinungsbild auf allen drei Plattformen identisch ist. Dieser Test
    // prüft, dass alle im UI tatsächlich verwendeten Sonderzeichen mit diesen eingebetteten
    // Schriften darstellbar sind (kein Platzhalter-Glyph/Tofu-Box).
    let ctx = egui::Context::default();
    crate::setup_ctx(&ctx);
    // `Context::fonts`/`fonts_mut` sind erst nach dem ersten `run`-Zyklus verfügbar.
    let mut out = ctx.run_ui(RawInput::default(), |_ui| {});
    out.textures_delta.clear();
    // Zeichen aus icons/view/material_ui (Häkchen, Kästchen, Pfeile, Trennpunkt, Kreuz, ⌘).
    let proportional_symbols = "☑☐•◦▪↑↓→←×⌘äöü";
    let mono_symbols = "×·äöü";
    // Hinweis: `Fonts::has_glyph`/`has_glyphs` sind für diesen Zweck NICHT zuverlässig,
    // weil epaint intern die erste Schrift in der Fallback-Kette, die ein
    // Platzhalter-Ersatzzeichen (z.B. `◻`) enthält, als "Ersatzglyph-Schrift" merkt
    // (siehe `CachedFamily::new` in epaint) – wird ein *echtes* Zeichen zufällig über
    // genau diese Schrift aufgelöst, meldet `has_glyph` fälschlich `false`, obwohl es
    // korrekt gerendert wird. Wir prüfen daher direkt über `Font::characters()` (die
    // tatsächliche, pro Schrift gesammelte Zeichenliste der Fallback-Kette), was dem
    // tatsächlichen Render-Pfad (`glyph_info`/`resolve_face`) entspricht.
    ctx.fonts_mut(|f| {
        let prop_chars = f.fonts.font(&egui::FontFamily::Proportional).characters().clone();
        for c in proportional_symbols.chars() {
            assert!(
                prop_chars.contains_key(&c),
                "Proportional-Schrift sollte Zeichen {c:?} abdecken"
            );
        }
        let regular_chars = f.fonts.font(&egui::FontFamily::Name("status-regular".into())).characters().clone();
        assert!(regular_chars.contains_key(&'≈'), "Status-Regular muss ≈ ohne Ersatzzeichen darstellen");
        let mono_chars = f.fonts.font(&egui::FontFamily::Monospace).characters().clone();
        for c in mono_symbols.chars() {
            assert!(
                mono_chars.contains_key(&c),
                "Monospace-Schrift sollte Zeichen {c:?} abdecken"
            );
        }
        // Normaler lateinischer Text (inkl. deutscher Umlaute) muss natürlich auch gehen.
        for c in "Hallo Welt äöüß".chars() {
            assert!(prop_chars.contains_key(&c), "Lateinisches Zeichen {c:?} sollte abgedeckt sein");
        }
    });
}

#[test]
fn fonts_register_bold_family_distinct_from_proportional() {
    // `BOLD_FAMILY` (siehe `md-egui`) muss registriert sein, sonst fällt `show_doc`
    // lautlos auf eine optische Ersatz-Hervorhebung zurück (siehe dort).
    let ctx = egui::Context::default();
    crate::setup_ctx(&ctx);
    let mut out = ctx.run_ui(RawInput::default(), |_ui| {});
    out.textures_delta.clear();
    let bold = egui::FontFamily::Name(md_egui::BOLD_FAMILY.into());
    ctx.fonts(|f| {
        assert!(f.families().contains(&bold), "BOLD_FAMILY sollte von setup_ctx registriert werden");
    });
}

// ---------- demo_scene (synthetische Inventar-/Vorschau-/Material-Konfiguration) ----------

#[test]
fn demo_scene_inventory_uses_only_synthetic_demo_paths() {
    let mut a = demo();
    a.demo_scene("inventory");
    assert!(!a.inventory.is_empty());
    // Keine echten/privaten Pfade – ausschließlich erfundene `/demo/...`-Pfade.
    for e in &a.inventory {
        assert!(e.path.starts_with("/demo/"), "unerwarteter Pfad im Inventar: {}", e.path);
    }
    for loc in &a.locations {
        assert!(loc.path.starts_with("/demo/"), "unerwarteter Ordner-Pfad: {}", loc.path);
    }
    for m in &a.material.items {
        assert!(m.value.starts_with("/demo/"), "unerwarteter Material-Pfad: {}", m.value);
    }
}

#[test]
fn demo_scene_longlog_builds_ten_thousand_entries() {
    let mut a = demo();
    a.demo_scene("longlog");
    assert_eq!(a.items.len(), 10_000);
}

// ---------- Virtualisierung unter einer langen, erzwungenen Sitzung ----------

#[test]
fn virtualized_log_renders_only_a_fraction_of_ten_thousand_entries() {
    // Reiner Verhaltenstest ohne Fenster/Screenshot (siehe Vorgabe): baut eine 10.000
    // Einträge lange Sitzung auf, zeichnet mehrere Frames (fensterlos über `ctx.run_ui`,
    // feste kleine Bildschirmgröße) und prüft über den `virtual_log`-Höhencache, dass
    // tatsächlich nur ein kleiner Bruchteil der Einträge je vermessen/gezeichnet wurde –
    // die eigentliche Absicherung der Virtualisierung unter einer langen Sitzung.
    let mut a = demo();
    a.demo_scene_longlog(10_000);
    // Normalen Viewport und den erzwungenen Sprung ans Ende im selben Test prüfen.
    a.force_scroll = false;
    let ctx = a.ctx.clone();
    crate::setup_ctx(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
    for frame in 0..4 {
        if frame == 2 { a.force_scroll = true; }
        let input = RawInput { screen_rect: Some(screen), time: Some(frame as f64 * 0.1), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| a.draw(ui));
        out.textures_delta.clear();
    }
    let measured = ctx.data(|d| d.get_temp::<crate::virtual_log::HeightCache>(egui::Id::new("copi-log-heights")).map(|c| c.measured_count()).unwrap_or(0));
    assert!(measured > 0, "es sollte überhaupt etwas gemessen worden sein");
    assert!(
        measured < 500,
        "Virtualisierung sollte bei 10.000 Einträgen nur einen kleinen Ausschnitt zeichnen, gemessen: {measured}"
    );
}

#[test]
fn jump_to_end_is_in_the_output_and_only_present_above_the_end() {
    let ctx = egui::Context::default();
    crate::setup_ctx(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(480.0, 400.0));
    let mut jump = false;
    let mut id = egui::Id::NULL;
    let mut viewport = egui::Rect::NOTHING;
    let mut draw = |offset, height, events| {
        let mut frame = ctx.run_ui(RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ui| {
            let out = egui::ScrollArea::vertical().max_height(230.0).auto_shrink([false, false]).vertical_scroll_offset(offset).show(ui, |ui| {
                ui.allocate_exact_size(egui::vec2(100.0, height), egui::Sense::hover());
            });
            id = out.id.with("jump-end");
            viewport = out.inner_rect;
            jump |= crate::view::log_jump(ui, &out);
        });
        frame.textures_delta.clear();
        (ctx.read_response(id), viewport, jump)
    };
    for (offset, height) in [(0.0, 40.0), (1000.0, 1000.0)] {
        draw(offset, height, vec![]);
        assert!(draw(offset, height, vec![]).0.is_none(), "Nicht nötig, daher kein Knopf");
    }
    let (resp, view, _) = draw(20.0, 1000.0, vec![]);
    let rect = resp.expect("Knopf im zurückgescrollten Verlauf").rect;
    assert!(view.contains_rect(rect));
    assert!(view.right() - rect.right() < 20.0 && view.bottom() - rect.bottom() < 20.0);
    let at = rect.center();
    draw(20.0, 1000.0, vec![Event::PointerMoved(at)]);
    let click = |pressed| Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    draw(20.0, 1000.0, vec![click(true)]);
    assert!(draw(20.0, 1000.0, vec![click(false)]).2, "Schaltfläche empfängt den Klick");
    draw(1000.0, 1000.0, vec![]);
    assert!(draw(1000.0, 1000.0, vec![]).0.is_none(), "Nach dem Sprung wieder unsichtbar");
}

// ---------- Statuszeile und Netz-Seitenleiste ----------

#[test]
fn ctrl_shift_b_toggles_net_sidebar_and_frames_render() {
    let mut a = demo();
    crate::setup_ctx(&a.ctx);
    assert!(!a.store.net_visible, "Netz-Seitenleiste ist standardmäßig aus");
    let ev = Event::Key { key: Key::B, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers { shift: true, ..Modifiers::COMMAND } };
    prompt_frame(&mut a, vec![ev]);
    assert!(a.store.net_visible);
    // Laufender Agent: Statuszeile mit Phase/Zählern und Netz-Leiste zeichnen ohne Panik.
    a.set_status("arbeitet …", true);
    a.activity.event(&json!({"type":"agent_start"}), 0.0);
    a.activity.event(&json!({"type":"tool_execution_start","toolCallId":"t","toolName":"bash","args":{"command":"ls"}}), 0.0);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(700.0, 500.0));
    let mut out = a.ctx.clone().run_ui(RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| a.draw(ui));
    out.textures_delta.clear();
    assert_eq!(a.activity.phase_label("bereit"), "Werkzeug: bash");
}

#[test]
fn duration_percent_matches_js_rules() {
    let mut s = crate::store::Store::default();
    assert_eq!(s.duration_percent(90.0), (45.0, false, 180.0));
    for n in [10.0, 20.0, 30.0] { s.record_duration(n); }
    let (p, over, est) = s.duration_percent(21.0);
    assert!(over && est == 20.0);
    assert!((p - 100.0 * 21.0 / 30.0).abs() < 1e-9, "Rest = Median längerer Läufe (30−21 = 9)");
    assert_eq!(s.duration_percent(1000.0).0, 100.0 * 1000.0 / 1020.0);
}
