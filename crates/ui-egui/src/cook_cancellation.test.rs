use super::*;
use materialcraft_engine::eval::Cooked;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

fn waiting() -> (MaterialCraftApp, mpsc::Sender<Result<PreviewCook, String>>, Arc<AtomicBool>) {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), crate::UiState::default());
    app.session.cook();
    app.session.cooked.ms = 123.0;
    let (tx, rx) = mpsc::channel();
    let flag = Arc::new(AtomicBool::new(false));
    app.cooker.rx = Some(rx);
    app.cooker.epoch = app.session.cook_epoch;
    app.cooker.cancel = Some(flag.clone());
    (app, tx, flag)
}

#[test]
fn rapid_edits_wait_for_the_cancelled_worker() {
    let (mut app, tx, flag) = waiting();
    app.session.dirty = true;
    for _ in 0..20 {
        drive(&mut app, &egui::Context::default());
        assert!(app.session.dirty, "a second worker started before the first ended");
        assert!(flag.load(Ordering::Relaxed));
        assert!(Arc::ptr_eq(app.cooker.cancel.as_ref().unwrap(), &flag));
    }
    tx.send(Ok(PreviewCook { cooked: Cooked { cancelled: true, ..Default::default() }, ..Default::default() })).unwrap();
    drive(&mut app, &egui::Context::default());
    assert!(!app.session.dirty, "latest snapshot should start once the worker ends");
    assert_eq!(app.session.cooked.ms, 123.0, "partial results replaced the display");
}

#[test]
fn ready_results_are_discarded_when_an_edit_is_pending() {
    let (mut app, tx, _) = waiting();
    tx.send(Ok(PreviewCook { cooked: Cooked { ms: 7.0, ..Default::default() }, ..Default::default() })).unwrap();
    app.session.dirty = true;
    drive(&mut app, &egui::Context::default());
    assert_eq!(app.session.cooked.ms, 123.0);
    assert!(!app.session.dirty);
}
