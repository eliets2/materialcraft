//! Resetting the search selection must also reveal it in a previously scrolled list.
use egui::{Event, Key, Modifiers};
use egui_kittest::{Harness, kittest::Queryable};
use materialcraft_ui_egui::MaterialCraftApp;

fn scroll(h: &mut Harness<'static, MaterialCraftApp>, rows: usize) {
    for _ in 0..rows {
        h.key_press(Key::ArrowDown);
        h.step();
    }
    h.run_steps(25);
}
fn filter_after_scrolling(h: &mut Harness<'static, MaterialCraftApp>) {
    scroll(h, 45);
    h.event(Event::Text("blur".into()));
    h.step();
    h.run_steps(25);
    assert!(super::label_position(h, "Blur").is_some(), "selected first filtered result is offscreen");
}
fn assert_inserted(h: &mut Harness<'static, MaterialCraftApp>, def: &str) {
    h.key_press(Key::Enter);
    h.step();
    let g = h.state().session.graph().unwrap();
    assert_eq!(g.nodes.len(), 1);
    assert_eq!(g.nodes[0].def, def);
}
#[test]
fn filtering_after_scrolling_reveals_and_inserts_first_match() {
    let mut h = super::harness();
    filter_after_scrolling(&mut h);
    super::save_snapshot(&mut h, "filtered-first-result");
    assert_inserted(&mut h, "blur");
}
#[test]
fn clearing_a_scrolled_query_reveals_and_inserts_first_match() {
    let mut h = super::harness();
    h.event(Event::Text("blur".into()));
    h.step();
    scroll(&mut h, 15);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.step();
    h.key_press(Key::Backspace);
    h.step();
    h.run_steps(25);
    assert!(super::label_position(&h, "Bitmap").is_some(), "selected first result is offscreen after clearing");
    assert_inserted(&mut h, "bitmap");
}
#[test]
fn reopening_a_scrolled_search_reveals_first_match() {
    let mut h = super::harness();
    scroll(&mut h, 45);
    h.key_press(Key::Escape);
    h.step();
    h.run_steps(3);
    h.key_press(Key::Tab);
    h.step();
    h.run_steps(25);
    assert!(h.get_by_label("Search nodes").rect().width() > 0.0);
    assert!(super::label_position(&h, "Bitmap").is_some(), "selected first result is offscreen after reopening");
    assert_inserted(&mut h, "bitmap");
}
