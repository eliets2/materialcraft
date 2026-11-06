//! Node cards distinguish editing previews from configured graph resolution.
use egui::{Event, epaint::Shape, vec2};
use egui_kittest::Harness;
use materialcraft_engine::core::NodeId;
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;

fn harness(preview: u32, render: bool) -> (Harness<'static, MaterialCraftApp>, NodeId) {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.ui.prefs.preview_size = preview;
    app.ui.graph_camera.zoom = 1.0;
    let id = NodeId(app.run("graph.node.add", json!({"def":"uniform"})).unwrap()["id"].as_u64().unwrap());
    app.apply_prefs();
    app.session.cook();
    let builder = Harness::builder().with_size(vec2(1600.0, 960.0)).with_step_dt(1.0 / 60.0);
    let builder = if render { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    );
    h.run_steps(10);
    (h, id)
}

fn texts(shape: &Shape, out: &mut Vec<String>) {
    match shape {
        Shape::Text(t) => out.push(t.galley.text().to_owned()),
        Shape::Vec(shapes) => {
            for s in shapes {
                texts(s, out);
            }
        }
        _ => {}
    }
}
fn painted(h: &Harness<'static, MaterialCraftApp>) -> Vec<String> {
    let mut out = Vec::new();
    for s in &h.output().shapes {
        texts(&s.shape, &mut out);
    }
    out
}

#[test]
fn node_resolution_explains_the_preview_cap() {
    let (mut h, id) = harness(512, false);
    let img = h.state().session.cooked.image(id, 0).unwrap();
    assert_eq!((img.w, img.h), (512, 512));
    assert_eq!(h.state().session.graph().unwrap().size, [10, 10]);
    assert!(painted(&h).iter().any(|t| t == "512x512 Preview"), "missing preview label: {:?}", painted(&h));
    let rect = h.state().graph.rects.iter().find(|(n, _)| *n == id).unwrap().1;
    h.event(Event::PointerMoved(rect.center()));
    h.run_steps(90);
    let text = painted(&h).join("\n");
    assert!(text.contains("Parent size: 1024 x 1024"), "{text}");
    assert!(text.contains("Preview limit: 512 px"), "{text}");
    assert!(text.contains("Edit > Preferences"), "{text}");
    assert!(text.contains("16bpc"), "format information lost: {text}");
    let full = h.state().session.cook_full(0);
    let img = full.image(id, 0).unwrap();
    assert_eq!((img.w, img.h), (1024, 1024));
}

#[test]
fn full_size_mode_retains_the_resolution_and_format() {
    let (h, id) = harness(0, false);
    let img = h.state().session.cooked.image(id, 0).unwrap();
    assert_eq!((img.w, img.h), (1024, 1024));
    assert!(painted(&h).iter().any(|t| t == "1024x1024 - C16"), "{:?}", painted(&h));
    assert!(!painted(&h).iter().any(|t| t == "1024x1024 Preview"));
}

#[test]
#[ignore = "requires an offscreen GPU"]
fn preview_resolution_screenshots() {
    let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    for (preview, name) in [(512, "preview"), (0, "full-size")] {
        let (mut h, id) = harness(preview, true);
        h.render().unwrap().save(dir.join(format!("{name}.png"))).unwrap();
        if preview > 0 {
            let rect = h.state().graph.rects.iter().find(|(n, _)| *n == id).unwrap().1;
            h.event(Event::PointerMoved(rect.center()));
            h.run_steps(90);
            h.render().unwrap().save(dir.join("preview-tooltip.png")).unwrap();
        }
    }
}
