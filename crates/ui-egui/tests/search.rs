//! Search interactions also capture screenshots when MC_SNAPSHOT_DIR is set

use egui::{Event, Pos2, Rect, epaint::Shape, vec2};
use egui_kittest::Harness;
use materialcraft_ui_egui::{MaterialCraftApp, UiState};

fn harness() -> Harness<'static, MaterialCraftApp> {
    let app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let builder = Harness::builder().with_size(vec2(1600.0, 960.0)).with_step_dt(1.0 / 60.0);
    let builder = if std::env::var_os("MC_SNAPSHOT_DIR").is_some() { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    );
    h.run_steps(8);
    let pointer = h.state().graph.view.center();
    h.event(Event::PointerMoved(pointer));
    h.step();
    h.key_press(egui::Key::Tab);
    h.step();
    h.run_steps(3);
    assert!(h.ctx.memory(|m| m.area_rect(egui::Id::unique("node-search"))).is_some());
    assert!(label_position(&h, "Bitmap").is_some(), "fresh search must show its first result");
    h
}

fn label_position(h: &Harness<'static, MaterialCraftApp>, label: &str) -> Option<Pos2> {
    let popup = h.ctx.memory(|m| m.area_rect(egui::Id::unique("node-search"))).unwrap();
    fn find(shape: &Shape, clip: Rect, popup: Rect, label: &str) -> Option<Pos2> {
        match shape {
            Shape::Text(t) if clip.contains(t.pos) && popup.contains(t.pos) && t.galley.text().trim() == label => {
                Some(t.pos + vec2(0.0, t.galley.size().y * 0.5))
            }
            Shape::Vec(v) => v.iter().find_map(|s| find(s, clip, popup, label)),
            _ => None,
        }
    }
    h.output().shapes.iter().find_map(|s| find(&s.shape, s.clip_rect, popup, label))
}

fn save_snapshot(harness: &mut Harness<'static, MaterialCraftApp>, name: &str) {
    if let Some(directory) = std::env::var_os("MC_SNAPSHOT_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        harness.render().unwrap().save(directory.join(format!("{name}.png"))).unwrap();
    }
}

#[path = "search/query.rs"]
mod query;
#[path = "search/rows.rs"]
mod rows;
