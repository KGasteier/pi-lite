//! Regressionstests ohne Fenster oder pi-Prozess.
use crate::app::{App, DialogKind, Job};
use egui::{Event, Key, Modifiers, RawInput};
use serde_json::json;

fn demo() -> App { App::new(egui::Context::default(), true) }

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
