//! Exact numeric edits must survive intervening frames and reach engine parameters
use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::{
    Harness,
    kittest::{By, Queryable},
};
use materialcraft_engine::core::{NodeId, Value};
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;

fn harness(render: bool) -> (Harness<'static, MaterialCraftApp>, NodeId) {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    app.run("file.new", json!({"template":"empty"})).unwrap();
    let id = NodeId(app.run("graph.node.add", json!({"def":"transformation"})).unwrap()["id"].as_u64().unwrap());
    app.session.reset_history();
    let builder = Harness::builder().with_size(vec2(1600.0, 960.0)).with_step_dt(1.0 / 60.0);
    let builder = if render { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    );
    h.run_steps(8);
    (h, id)
}
fn focus_x(h: &mut Harness<'static, MaterialCraftApp>, index: usize) {
    h.get_all(By::new().role(egui::accesskit::Role::TextInput).label("X")).nth(index).unwrap().click();
    h.step();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.step();
}
fn type_text(h: &mut Harness<'static, MaterialCraftApp>, text: &str) {
    h.event(Event::Text(text.into()));
    h.step();
    h.run_steps(4);
}
fn pair(h: &Harness<'static, MaterialCraftApp>, id: NodeId, param: &str) -> [f32; 2] {
    match h.state().session.graph().unwrap().node(id).unwrap().params.get(param) {
        Some(Value::Float2(v)) => *v,
        None => match materialcraft_engine::nodes::find("transformation").unwrap().params.iter().find(|p| p.id == param).unwrap().default {
            Value::Float2(v) => v,
            _ => panic!("not a pair"),
        },
        other => panic!("unexpected value {other:?}"),
    }
}
#[test]
fn graph_name_typed_across_frames_commits_on_focus_loss() {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    app.run("file.new", json!({"template":"empty"})).unwrap();
    app.session.reset_history();
    let builder = Harness::builder().with_size(vec2(1600.0, 960.0)).with_step_dt(1.0 / 60.0);
    let mut h = builder.build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    );
    h.run_steps(8);
    h.state_mut().run("graph.select", json!({"ids": []})).unwrap();
    h.run_steps(3);
    h.get_by_label("Graph name").click();
    h.step();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.step();
    type_text(&mut h, "My_graph");
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(4);
    assert_eq!(h.state().session.graph().unwrap().name, "My_graph", "graph name typed across frames was reset before focus was lost");
}
#[test]
fn offset_x_commits_on_enter_and_focus_loss_with_undo() {
    for enter in [true, false] {
        let (mut h, id) = harness(false);
        focus_x(&mut h, 0);
        type_text(&mut h, "0.4");
        if enter {
            h.key_press(Key::Enter);
            h.step();
        } else {
            h.get_by_label("Node label").click();
            h.step();
        }
        h.run_steps(5);
        assert_eq!(pair(&h, id, "offset"), [0.4, 0.0], "Offset X did not persist (Enter={enter})");
        h.state_mut().run("edit.undo", json!({})).unwrap();
        h.run_steps(3);
        assert_eq!(pair(&h, id, "offset"), [0.0, 0.0]);
        h.state_mut().run("edit.redo", json!({})).unwrap();
        h.run_steps(3);
        assert_eq!(pair(&h, id, "offset"), [0.4, 0.0]);
    }
}
#[test]
fn a_negative_decimal_can_be_typed_across_frames() {
    let (mut h, id) = harness(false);
    focus_x(&mut h, 0);
    for part in ["-", ".", "5"] {
        type_text(&mut h, part);
    }
    h.key_press(Key::Enter);
    h.step();
    h.run_steps(4);
    assert_eq!(pair(&h, id, "offset"), [-0.5, 0.0]);
}
#[test]
fn clicking_the_canvas_and_switching_nodes_keep_the_right_values() {
    let (mut h, id) = harness(false);
    focus_x(&mut h, 0);
    type_text(&mut h, "0.4");
    let pos = h.state().graph.view.left_top() + vec2(20.0, 20.0);
    h.event(Event::PointerMoved(pos));
    h.step();
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run_steps(4);
    assert!(h.state().session.selection.is_empty());
    assert_eq!(pair(&h, id, "offset"), [0.4, 0.0]);
    h.state_mut().run("graph.select", json!({"ids": [id.0]})).unwrap();
    h.run_steps(4);
    focus_x(&mut h, 0);
    type_text(&mut h, "0.5");
    let other = NodeId(h.state_mut().run("graph.node.add", json!({"def":"transformation","at":[300,0]})).unwrap()["id"].as_u64().unwrap());
    h.run_steps(4);
    h.key_press(Key::Enter);
    h.step();
    assert_eq!(pair(&h, id, "offset"), [0.5, 0.0]);
    assert_eq!(pair(&h, other, "offset"), [0.0, 0.0], "an edit leaked into the newly selected node");
}

#[test]
fn invalid_input_is_ignored_and_values_are_clamped() {
    let (mut h, id) = harness(false);
    focus_x(&mut h, 0);
    type_text(&mut h, "NaN");
    h.key_press(Key::Enter);
    h.step();
    h.run_steps(4);
    assert_eq!(pair(&h, id, "offset"), [0.0, 0.0]);
    focus_x(&mut h, 0);
    type_text(&mut h, "123");
    h.key_press(Key::Enter);
    h.step();
    h.run_steps(4);
    assert_eq!(pair(&h, id, "offset"), [1.0, 0.0]);
    focus_x(&mut h, 0);
    type_text(&mut h, "-");
    h.key_press(Key::Escape);
    h.step();
    h.run_steps(4);
    assert_eq!(pair(&h, id, "offset"), [1.0, 0.0]);
}

#[test]
fn integer_input_and_slider_track_both_update_the_node() {
    let (mut h, id) = harness(false);
    let input = By::new().role(egui::accesskit::Role::TextInput).label("Random seed");
    h.get(input).click();
    h.step();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.step();
    type_text(&mut h, "42");
    h.key_press(Key::Enter);
    h.step();
    h.run_steps(4);
    assert_eq!(h.state().session.graph().unwrap().node(id).unwrap().base.seed, 42);
    let pos = h.get(By::new().role(egui::accesskit::Role::Slider).label("Random seed")).rect().center();
    h.event(Event::PointerMoved(pos));
    h.step();
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run_steps(4);
    assert_eq!(h.state().session.graph().unwrap().node(id).unwrap().base.seed, 500);
}

#[test]
#[ignore = "requires an offscreen GPU"]
fn exact_offset_screenshot() {
    let (mut h, id) = harness(true);
    focus_x(&mut h, 0);
    type_text(&mut h, "0.4");
    h.key_press(Key::Enter);
    h.step();
    h.run_steps(8);
    assert_eq!(pair(&h, id, "offset"), [0.4, 0.0]);
    let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    h.render().unwrap().save(dir.join("offset-x-4.png")).unwrap();
}
