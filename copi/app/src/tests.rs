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

// ---------- Material (Stufe 3) ----------

#[test]
fn material_plus_slash_command_is_rejected() {
    let mut a = demo();
    a.material.items.push(crate::material_ui::MaterialItem { value: "/tmp/a.png".into(), url: false });
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
    a.material.items.push(crate::material_ui::MaterialItem { value: "/tmp/a.png".into(), url: false });
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
    let twelve: Vec<_> = (0..12).map(|i| crate::material_ui::MaterialItem { value: format!("/tmp/f{i}.txt"), url: false }).collect();
    a.merge_material(twelve.clone());
    assert_eq!(a.material.items.len(), 12);
    // Ein weiteres Element über das Limit hinaus wird abgelehnt; die Liste bleibt bei 12.
    a.merge_material(vec![crate::material_ui::MaterialItem { value: "/tmp/f12.txt".into(), url: false }]);
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
    let item = crate::material_ui::MaterialItem { value: "/tmp/a.png".into(), url: false };
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
    a.material.items.push(crate::material_ui::MaterialItem { value: "/tmp/a.png".into(), url: false });
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
    // `push` setzt `force_scroll`, was den nächsten Frame zwingt, *alles* zu rendern (siehe
    // `log()`/`view.rs`), damit nach einem Sitzungswechsel ans Ende gescrollt werden kann.
    // Für diesen Test interessiert aber gerade das normale (nicht erzwungene) Verhalten.
    a.force_scroll = false;
    let ctx = a.ctx.clone();
    crate::setup_ctx(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
    for frame in 0..3 {
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

// ---------- Mehrere Backends/Instanzen (net.rs / net_view.rs) ----------

mod net {
    use crate::net::*;
    use pi_backend::backends::BackendCfg;
    use pi_backend::instances::{Ev, InstanceInfo, State, Tagged};
    use std::sync::Arc;

    fn state() -> NetState {
        let mut n = NetState::new(Arc::new(|| {}), true);
        n.demo_scene();
        n
    }
    fn ev(b: &str, i: &str, ev: Ev) -> Tagged {
        Tagged { backend: b.into(), inst: i.into(), ev }
    }
    fn key(b: &str, i: &str) -> InstKey {
        (b.into(), i.into())
    }

    #[test]
    fn demo_scene_has_two_backends_four_instances_one_waiting() {
        let n = state();
        assert_eq!(n.backends.len(), 2);
        assert_eq!(n.backends.iter().map(|b| b.insts.len()).sum::<usize>(), 4);
        let waiting: Vec<_> = n.backends.iter().flat_map(|b| &b.insts).filter(|i| i.info.state == State::Waiting).collect();
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].chat.asks.len(), 1, "Rückfrage steht inline bereit");
        assert_eq!(waiting[0].chat.asks[0].kind, AskKind::Confirm);
    }

    #[test]
    fn line_events_become_chat_items() {
        let mut n = state();
        let k = key("hub-buero", "c3d4");
        for l in [
            r#"{"type":"message_start","message":{"role":"user","content":"hallo"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","delta":"hm"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hal"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"lo!"}}"#,
            r#"{"type":"tool_execution_start","toolCallId":"x","toolName":"bash","args":{"command":"ls -la\nmehr"}}"#,
            r#"{"type":"tool_execution_end","toolCallId":"x","isError":true}"#,
            r#"{"type":"extension_ui_request","id":"q1","method":"select","title":"Welche?","options":["a","b"]}"#,
            r#"{"type":"extension_ui_request","id":"q1","method":"select","title":"doppelt","options":["a"]}"#,
            "kein json",
        ] {
            n.on_ev(ev("hub-buero", "c3d4", Ev::Line(l.into())));
        }
        let c = &n.inst(&k).unwrap().chat;
        assert!(matches!(&c.items[0], NetItem::User(t) if t == "hallo"));
        assert!(matches!(&c.items[1], NetItem::Thinking(t) if t == "hm"));
        assert!(matches!(&c.items[2], NetItem::Assistant { text, done: true, .. } if text == "Hallo!"), "Werkzeugstart schließt die Antwort ab");
        assert!(matches!(&c.items[3], NetItem::Tool { name, detail, done: true, err: true, .. } if name == "bash" && detail == "ls -la…"));
        assert_eq!(c.items.len(), 4);
        assert_eq!(c.asks.len(), 1, "gleiche Rückfrage-ID nur einmal");
        assert_eq!(c.asks[0].options, vec!["a", "b"]);
        // Abschluss leert die Rückfragen
        n.on_ev(ev("hub-buero", "c3d4", Ev::Line(r#"{"type":"agent_settled"}"#.into())));
        assert!(n.inst(&k).unwrap().chat.asks.is_empty());
    }

    #[test]
    fn info_updates_pill_and_new_instances_appear() {
        let mut n = state();
        let mut info = n.inst(&key("lokal", "l2")).unwrap().info.clone();
        info.state = State::Busy;
        info.last = "neu".into();
        n.on_ev(ev("lokal", "l2", Ev::Info(info.clone())));
        let i = &n.inst(&key("lokal", "l2")).unwrap().info;
        assert_eq!((i.state, i.last.as_str()), (State::Busy, "neu"));
        info.id = "l9".into();
        n.on_ev(ev("lokal", "l9", Ev::Info(info)));
        assert_eq!(n.backends[0].insts.len(), 3);
        // Unbekanntes Backend wird ignoriert
        let other = InstanceInfo { id: "z".into(), name: "z".into(), harness: "pi".into(), cwd: "/".into(), state: State::Idle, asks: 0, last: String::new(), session: None };
        n.on_ev(ev("gibts-nicht", "z", Ev::Info(other)));
        assert_eq!(n.backends.iter().map(|b| b.insts.len()).sum::<usize>(), 5);
    }

    #[test]
    fn info_with_fewer_asks_drops_only_old_asks() {
        let mut n = state();
        let k = key("hub-buero", "a1b2");
        let mut info = n.inst(&k).unwrap().info.clone();
        info.asks = 0;
        info.state = State::Busy;
        // Frische Rückfrage (< 2 s) bleibt trotz Info mit 0 offenen stehen (getrennte Streams).
        n.on_ev(ev("hub-buero", "a1b2", Ev::Info(info)));
        assert_eq!(n.inst(&k).unwrap().chat.asks.len(), 1);
        // Erneut gelieferter Verlauf (Live mit 0 offenen) räumt sie ab.
        n.on_ev(ev("hub-buero", "a1b2", Ev::Live));
        assert!(n.inst(&k).unwrap().chat.asks.is_empty());
    }

    #[test]
    fn gone_removes_instance_and_selection_exit_marks_state() {
        let mut n = state();
        let k = key("lokal", "l1");
        assert!(n.selected.contains(&k));
        n.on_ev(ev("lokal", "l1", Ev::Gone));
        assert!(n.inst(&k).is_none());
        assert!(!n.selected.contains(&k), "entfernte Instanz verlässt das Raster");
        let w = key("hub-buero", "a1b2");
        n.on_ev(ev("hub-buero", "a1b2", Ev::Exit));
        let i = n.inst(&w).unwrap();
        assert_eq!(i.info.state, State::Exited);
        assert!(i.chat.asks.is_empty());
    }

    #[test]
    fn link_false_sets_hint_and_true_clears_error() {
        let mut n = state();
        n.on_ev(ev("hub-buero", "", Ev::Link(false)));
        assert_eq!(n.backends[1].link, Some(false));
        n.backends[1].error = Some("x".into());
        n.on_ev(ev("hub-buero", "", Ev::Link(true)));
        assert_eq!(n.backends[1].link, Some(true));
        assert!(n.backends[1].error.is_none());
    }

    #[test]
    fn selection_click_and_ctrl_click() {
        let mut n = state();
        n.grid = false;
        n.select(key("lokal", "l2"), false);
        assert_eq!(n.selected, vec![key("lokal", "l2")]);
        assert!(n.grid, "Auswahl schaltet aufs Raster");
        n.select(key("hub-buero", "a1b2"), true);
        n.select(key("lokal", "l1"), true);
        assert_eq!(n.selected.len(), 3);
        n.select(key("hub-buero", "a1b2"), true);
        assert_eq!(n.selected, vec![key("lokal", "l2"), key("lokal", "l1")], "Strg-Klick nimmt wieder heraus, Reihenfolge bleibt");
        n.select(key("hub-buero", "c3d4"), false);
        assert_eq!(n.selected, vec![key("hub-buero", "c3d4")]);
    }

    #[test]
    fn grid_dimensions() {
        assert_eq!(grid_dims(1, 1000.0, 600.0), (1, 600.0, false));
        assert_eq!(grid_dims(2, 1000.0, 600.0), (2, 600.0, false));
        assert_eq!(grid_dims(3, 1000.0, 600.0), (2, 300.0, false));
        assert_eq!(grid_dims(4, 1000.0, 600.0), (2, 300.0, false));
        let (c, h, s) = grid_dims(5, 1000.0, 600.0);
        assert!(c == 2 && h == 300.0 && s, "ab 5 scrollbar");
        let (c, _, s) = grid_dims(2, 500.0, 600.0);
        assert!(c == 1 && !s, "schmal: einspaltig, 2 Kacheln passen");
        let (c, h, s) = grid_dims(3, 500.0, 400.0);
        assert!(c == 1 && h == 260.0 && s, "schmal mit 3: Mindesthöhe, scrollbar");
    }

    #[test]
    fn backend_form_validation() {
        let list = vec![BackendCfg::local(), BackendCfg::hub("hub-buero", "Büro", "http://b:1", "geheim")];
        let f = |name: &str, url: &str, token: &str, env: &str| BackendForm { name: name.into(), url: url.into(), token: token.into(), token_env: env.into(), ..Default::default() };
        assert!(validate_form(&f("", "http://x:1", "t", ""), &list).unwrap_err().contains("Namen"));
        assert!(validate_form(&f("A", "ftp://x", "t", ""), &list).unwrap_err().contains("http://"));
        assert!(validate_form(&f("A", "http://x :1", "t", ""), &list).unwrap_err().contains("Leerzeichen"));
        assert!(validate_form(&f("A", "http://x:1", "", ""), &list).unwrap_err().contains("Token"));
        assert!(validate_form(&f("A", "http://x:1", "t", "ENV"), &list).unwrap_err().contains("nicht beides"));
        assert!(validate_form(&f("A", "http://x:1", "", "a-b"), &list).unwrap_err().contains("Umgebungsvariable"));
        assert!(validate_form(&f("büro", "http://x:1", "t", ""), &list).unwrap_err().contains("schon"));
        let c = validate_form(&f(" Server 2 ", "http://10.0.0.2:8787/", "abc", ""), &list).unwrap();
        assert_eq!((c.id.as_str(), c.label.as_str(), c.url.as_str(), c.token.as_str(), c.kind.as_str()), ("hub-server-2", "Server 2", "http://10.0.0.2:8787", "abc", "hub"));
        let c = validate_form(&f("Env", "https://h.example", "", "PI_HUB_TOKEN"), &list).unwrap();
        assert!(c.token.is_empty() && c.token_env == "PI_HUB_TOKEN");
        // Bearbeiten: leeres Token behält das gespeicherte, ID bleibt, eigener Name ist erlaubt.
        let mut e = f("Büro", "http://b:2", "", "");
        e.edit = Some(1);
        let c = validate_form(&e, &list).unwrap();
        assert_eq!((c.id.as_str(), c.token.as_str(), c.url.as_str()), ("hub-buero", "geheim", "http://b:2"));
        assert_eq!(slug_id("Büro", &["hub-b-ro"]), "hub-b-ro-2");
    }

    #[test]
    fn errors_are_friendly_and_never_contain_token() {
        let m = friendly_err("Hub: Token abgelehnt (401)", "supergeheim");
        assert!(m.contains("401") && !m.contains("supergeheim"));
        let m = friendly_err("Hub nicht erreichbar: io: Connection refused", "x");
        assert!(m.starts_with("Server nicht erreichbar") && m.contains("Connection refused"));
        let m = friendly_err("http://h/?token=supergeheim kaputt", "supergeheim");
        assert!(!m.contains("supergeheim"));
    }

    #[test]
    fn form_open_never_prefills_token() {
        let mut n = state();
        n.backends[1].cfg.token = "supergeheim".into();
        n.open_form(Some(1));
        let f = n.form.as_ref().unwrap();
        assert!(f.token.is_empty() && f.has_token);
        assert!(!format!("{f:?}").contains("supergeheim"));
    }

    #[test]
    fn answer_removes_ask_locally() {
        let mut n = state();
        let k = key("hub-buero", "a1b2");
        n.answer(&k, "ask-1", serde_json::json!({"confirmed": true}));
        assert!(n.inst(&k).unwrap().chat.asks.is_empty());
    }

    #[test]
    fn short_cwd_keeps_last_two_parts() {
        assert_eq!(short_cwd("/a/b/c/d"), "…/c/d");
        assert_eq!(short_cwd("/a/b"), "/a/b");
    }
}

#[test]
fn net_scene_frames_render_wide_and_narrow() {
    for (w, h) in [(1280.0, 800.0), (700.0, 500.0)] {
        let mut a = demo();
        crate::setup_ctx(&a.ctx);
        a.demo_scene("net");
        assert!(a.store.net_visible && a.net.grid);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
        for frame in 0..3 {
            let mut out = a.ctx.clone().run_ui(RawInput { screen_rect: Some(screen), time: Some(frame as f64 * 0.1), ..Default::default() }, |ui| a.draw(ui));
            out.textures_delta.clear();
        }
        // Dialoge zeichnen ohne Panik; Esc schließt sie.
        a.net.open_form(None);
        a.net.open_new_inst("hub-buero");
        prompt_frame(&mut a, vec![]);
        assert!(a.net.new_inst.as_ref().unwrap().error.is_some(), "Demo: keine Ordnerliste, verständlicher Hinweis");
        prompt_frame(&mut a, vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
        assert!(a.net.form.is_none() && a.net.new_inst.is_none());
        // Einzel-Chat bleibt erreichbar
        a.net.grid = false;
        prompt_frame(&mut a, vec![]);
    }
}

/// Live gegen einen laufenden Mock-Hub (nicht im normalen Lauf). Aufruf:
/// `XDG_CONFIG_HOME=<tmp> COPI_LIVE_HUB=1 cargo test -- --ignored net_live_mock_hub --nocapture`
/// mit `backends.json` in `<tmp>/copi/`, die genau einen Hub (Harness mock) einträgt.
#[test]
#[ignore]
fn net_live_mock_hub() {
    use crate::net::*;
    use pi_backend::instances::State;
    use std::time::{Duration, Instant};
    if std::env::var("COPI_LIVE_HUB").is_err() {
        return;
    }
    let mut n = NetState::new(std::sync::Arc::new(|| {}), false);
    let wait = |n: &mut NetState, what: &str, f: &dyn Fn(&NetState) -> bool| {
        let t = Instant::now();
        while !f(n) {
            assert!(t.elapsed() < Duration::from_secs(10), "Zeitüberschreitung: {what}");
            n.pump(true);
            std::thread::sleep(Duration::from_millis(20));
        }
        eprintln!("ok: {what} ({} ms)", t.elapsed().as_millis());
    };
    n.pump(true);
    let hub = n.backends.iter().find(|b| b.remote()).expect("Hub in backends.json").cfg.id.clone();
    wait(&mut n, "Hub verbunden", &|n| n.backend(&hub).is_some_and(|b| b.link == Some(true)));
    let mut keys = Vec::new();
    for name in ["eins", "zwei"] {
        n.open_new_inst(&hub);
        wait(&mut n, "Ordnerliste", &|n| n.new_inst.as_ref().is_some_and(|x| !x.loading));
        let ni = n.new_inst.as_mut().unwrap();
        assert!(ni.error.is_none(), "{:?}", ni.error);
        ni.harness = "mock".into();
        ni.name = name.into();
        n.create_inst();
        wait(&mut n, "Instanz angelegt", &|n| n.new_inst.is_none());
        keys.push(n.selected.last().unwrap().clone());
    }
    assert_eq!(n.selected.len(), 2, "beide im Raster");
    for k in &keys {
        wait(&mut n, "Verlauf live", &|n| n.inst(k).is_some_and(|i| i.chat.live && i.info.state == State::Idle));
    }
    // Zwei Prompts parallel
    n.inst_mut(&keys[0]).unwrap().chat.input = "hallo eins".into();
    n.inst_mut(&keys[1]).unwrap().chat.input = "bitte lösch das".into();
    n.send_prompt(&keys[0]);
    n.send_prompt(&keys[1]);
    wait(&mut n, "Antwort eins", &|n| n.inst(&keys[0]).is_some_and(|i| i.chat.items.iter().any(|x| matches!(x, NetItem::Assistant { done: true, .. }))));
    wait(&mut n, "Rückfrage zwei", &|n| n.inst(&keys[1]).is_some_and(|i| !i.chat.asks.is_empty() && i.info.state == State::Waiting && i.info.asks == 1));
    let ask = n.inst(&keys[1]).unwrap().chat.asks[0].clone();
    assert_eq!(ask.kind, AskKind::Confirm);
    n.answer(&keys[1], &ask.id, serde_json::json!({"confirmed": true}));
    wait(&mut n, "Antwort nach Bestätigung", &|n| n.inst(&keys[1]).is_some_and(|i| i.chat.items.iter().any(|x| matches!(x, NetItem::Assistant { text, .. } if text.contains("Erledigt")))));
    wait(&mut n, "zwei wieder bereit", &|n| n.inst(&keys[1]).is_some_and(|i| i.info.state == State::Idle && i.chat.asks.is_empty()));
    for k in &keys {
        let i = n.inst(k).unwrap();
        eprintln!("{}: {:?}, {} Einträge, last={:?}", i.info.name, i.info.state, i.chat.items.len(), i.info.last);
    }
    // Stopp = entfernen
    n.stop(&keys[0]);
    wait(&mut n, "eins entfernt", &|n| n.inst(&keys[0]).is_none() && !n.selected.contains(&keys[0]));
    n.stop(&keys[1]);
    wait(&mut n, "zwei entfernt", &|n| n.inst(&keys[1]).is_none());
}

/// Live: Fehlertexte bei falschem Token (Hub auf `COPI_LIVE_HUB_URL`) und geschlossenem Port.
#[test]
#[ignore]
fn net_live_errors() {
    use crate::net::friendly_err;
    use pi_backend::backends::BackendCfg;
    let Ok(url) = std::env::var("COPI_LIVE_HUB_URL") else { return };
    for (u, tok) in [(url.as_str(), "falsch-geheim"), ("http://127.0.0.1:1", "falsch-geheim")] {
        let api = BackendCfg::hub("h", "H", u, tok).build().unwrap();
        let e = api.list().unwrap_err();
        let m = friendly_err(&e, tok);
        eprintln!("{u}: {m}");
        assert!(!m.contains(tok));
    }
}
