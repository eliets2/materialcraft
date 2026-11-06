//! Resolution changes must reach both material maps and the displayed image, and survive frames.
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use materialcraft_ui_egui::{MaterialCraftApp, UiState, control, view2d};
use serde_json::json;

fn app() -> MaterialCraftApp {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.run("file.new", json!({"template": "empty", "size": 4096})).unwrap();
    let node = app.run("graph.node.add", json!({"def": "uniform", "params": {"color_mode": false}})).unwrap()["id"].as_u64().unwrap();
    app.run("graph.node.add", json!({"def": "output", "connectFrom": {"node": node}})).unwrap();
    app.run("view.2d", json!({"node": node})).unwrap();
    app.session.cook();
    app
}

fn choose_4k(h: &mut Harness<'static, MaterialCraftApp>) {
    h.get_by_label("Preview resolution").click();
    h.run();
    h.get_by_label("4096 px").click();
    h.run();
    assert_eq!(h.state().session.preview_size, Some(4096));
    assert_eq!(h.state().ui.prefs.preview_size, 4096);
    h.state_mut().session.cook();
    h.step();
    let app = h.state();
    let (_, image) = app.session.view2d_image().unwrap();
    assert_eq!((image.w, image.h), (4096, 4096));
    assert!(app.session.material_maps().iter().all(|(_, image)| (image.w, image.h) == (4096, 4096)));
    assert!(!app.session.material_maps().is_empty());
    assert_eq!(app.view2d.shown.as_ref().map(|(w, h, _)| (*w, *h)), Some((4096, 4096)));
}

#[test]
fn selecting_4k_cooks_and_displays_4k_material_maps() {
    let mut h = Harness::builder().with_context_setup(materialcraft_ui_egui::theme::install).with_size(vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.apply_prefs();
            view2d::show(app, ui);
        },
        app(),
    );
    h.input_mut().max_texture_side = Some(4096);
    choose_4k(&mut h);
    // A smaller device limit must refresh the texture without losing the cooked source.
    h.input_mut().max_texture_side = Some(2048);
    h.step();
    assert_eq!(h.state().view2d.shown.as_ref().map(|(w, h, _)| (*w, *h)), Some((4096, 4096)));
}

#[test]
fn four_k_preview_refreshes_texture_after_size_change() {
    let mut h = Harness::builder().with_context_setup(materialcraft_ui_egui::theme::install).with_size(vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.apply_prefs();
            view2d::show(app, ui);
        },
        app(),
    );
    h.input_mut().max_texture_side = Some(4096);
    // The cooked key alone does not name the input size, so changing size must still refresh.
    let node = h.state().session.view2d.node.unwrap();
    h.state_mut().session.preview_size = Some(4096);
    h.state_mut().session.cook();
    h.state_mut().session.cooked.nodes.get_mut(&node).unwrap().key = 0;
    h.state_mut().ui.prefs.preview_size = 4096;
    h.step();
    let (_, image) = h.state().session.view2d_image().unwrap();
    assert_eq!((image.w, image.h), (4096, 4096));
    assert_eq!(h.state().view2d.shown.as_ref().map(|(w, h, _)| (*w, *h)), Some((4096, 4096)));
    h.state_mut().session.preview_size = Some(256);
    h.state_mut().session.cook();
    h.state_mut().session.cooked.nodes.get_mut(&node).unwrap().key = 0;
    h.state_mut().ui.prefs.preview_size = 256;
    h.step();
    let (_, image) = h.state().session.view2d_image().unwrap();
    assert_eq!((image.w, image.h), (256, 256));
    assert_eq!(h.state().view2d.shown.as_ref().map(|(w, h, _)| (*w, *h)), Some((256, 256)));
}

#[test]
fn command_resolution_survives_preferences_and_invalid_requests() {
    let mut app = app();
    let ctx = egui::Context::default();
    for size in [4096, 0, 512] {
        let response = control::handle(&mut app, &ctx, "command.run", &json!({"id": "view.previewSize", "params": {"size": size}}));
        assert_eq!(response["ok"], true);
        app.apply_prefs();
        assert_eq!(app.ui.prefs.preview_size, size);
        assert_eq!(app.session.preview_size, if size == 0 { None } else { Some(size) });
    }
    assert!(app.run("view.previewSize", json!({"size": -1})).is_none());
    assert_eq!(app.ui.prefs.preview_size, 512);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn resolution_snapshot() {
    let mut h = Harness::builder().with_context_setup(materialcraft_ui_egui::theme::install).with_size(vec2(1200.0, 800.0)).wgpu().build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.apply_prefs();
            view2d::show(app, ui);
        },
        app(),
    );
    choose_4k(&mut h);
    let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    h.render().unwrap().save(dir.join("resolution-4k.png")).unwrap();
}
