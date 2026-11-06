//! Pointer drags should reach nearby round values, as standard egui sliders do.
use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::{
    Harness,
    kittest::{By, Queryable},
};
use materialcraft_engine::core::{NodeId, Value};
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;

fn harness(ppp: f32, render: bool, definition: &str) -> (Harness<'static, MaterialCraftApp>, NodeId) {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let id = NodeId(app.run("graph.node.add", json!({"def":definition})).unwrap()["id"].as_u64().unwrap());
    app.session.reset_history();
    let builder = Harness::builder().with_size(vec2(1600.0, 960.0)).with_pixels_per_point(ppp).with_step_dt(1.0 / 60.0);
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

fn drag_near(h: &mut Harness<'static, MaterialCraftApp>, label: &str, index: usize, value: f32, lo: f32, hi: f32) {
    let rect = h.get_all(By::new().role(egui::accesskit::Role::Slider).label(label)).nth(index).unwrap().rect();
    let start = pos2(rect.left() + rect.width() * 0.2, rect.center().y);
    // Aim slightly off the exact value, within one physical pixel.
    let target = pos2(rect.left() + 5.0 + (rect.width() - 10.0) * (value - lo) / (hi - lo) + 0.4 / h.ctx.pixels_per_point(), rect.center().y);
    h.event(Event::PointerMoved(start));
    h.step();
    h.event(Event::PointerButton { pos: start, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.event(Event::PointerMoved(start + vec2(10.0, 0.0)));
    h.step();
    h.event(Event::PointerMoved(target));
    h.step();
    h.event(Event::PointerButton { pos: target, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.step();
    h.run_steps(3);
}
fn angle_value(h: &Harness<'static, MaterialCraftApp>, id: NodeId) -> f32 {
    match h.state().session.graph().unwrap().node(id).unwrap().params.get("angle") {
        Some(Value::Float(v)) => *v,
        None => 0.0,
        other => panic!("unexpected angle {other:?}"),
    }
}

#[test]
fn dragging_can_reach_exact_round_angles_at_multiple_pixel_scales() {
    for ppp in [1.0, 2.0] {
        for angle in [90.0, -90.0, 0.0, 360.0, -360.0] {
            let (mut h, id) = harness(ppp, false, "directional_blur");
            drag_near(&mut h, "Angle", 0, angle, -360.0, 360.0);
            assert_eq!(angle_value(&h, id), angle / 360.0, "angle={angle}, pixels_per_point={ppp}");
            if angle == 90.0 {
                h.state_mut().run("edit.undo", json!({})).unwrap();
                assert_eq!(angle_value(&h, id), 0.0);
                h.state_mut().run("edit.redo", json!({})).unwrap();
                assert_eq!(angle_value(&h, id), 0.25);
            }
        }
    }
}

#[test]
fn fractional_parameter_values_remain_reachable() {
    let (mut h, id) = harness(1.0, false, "transformation");
    drag_near(&mut h, "Y", 0, 0.25, -1.0, 1.0);
    drag_near(&mut h, "X", 0, -0.5, -1.0, 1.0);
    let n = h.state().session.graph().unwrap().node(id).unwrap();
    assert_eq!(n.params.get("offset"), Some(&Value::Float2([-0.5, 0.25])));
}

#[test]
#[ignore = "requires an offscreen GPU"]
fn exact_angle_screenshot() {
    let (mut h, id) = harness(1.0, true, "directional_blur");
    drag_near(&mut h, "Angle", 0, 90.0, -360.0, 360.0);
    assert_eq!(angle_value(&h, id), 0.25);
    let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    h.render().unwrap().save(dir.join("rotation-90.png")).unwrap();
}
