use crate::MaterialCraftApp;
use egui::{Pos2, Rect, vec2};
use materialcraft_engine::core::NodeId;

/// The whole app in an offscreen egui context, driven one frame at a time
pub(super) struct Frames {
    pub(super) ctx: egui::Context,
    pub(super) app: MaterialCraftApp,
    pub(super) time: f64,
    pub(super) canvas_text: Vec<String>,
}

impl Frames {
    /// `session` after a few frames, so the canvas is laid out
    pub(super) fn new(session: materialcraft_engine::Session) -> Frames {
        let mut f =
            Frames { ctx: egui::Context::default(), app: MaterialCraftApp::headless(session, crate::UiState::default()), time: 0.0, canvas_text: Vec::new() };
        for _ in 0..3 {
            f.run(Vec::new(), egui::Modifiers::NONE);
        }
        f
    }

    /// Draw a desktop-sized frame with synthetic input
    pub(super) fn run(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        self.time += 0.05;
        let events = std::iter::once(egui::Event::ModifiersChanged(modifiers)).chain(events).collect();
        let screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 960.0)));
        let input = egui::RawInput { events, screen_rect, time: Some(self.time), ..Default::default() };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        });
        // These interaction tests inspect shapes without uploading textures
        output.textures_delta.clear();
        self.canvas_text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if self.app.graph.view.contains(text.pos) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect();
    }

    /// Press and release the primary button at `pos`, a second after any earlier click so the two never pair into a double-click
    pub(super) fn click(&mut self, pos: Pos2, modifiers: egui::Modifiers) {
        self.time += 1.0;
        self.run(vec![egui::Event::PointerMoved(pos)], modifiers);
        for pressed in [true, false] {
            self.run(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers }], modifiers);
        }
    }

    /// The center of node `id` on screen last frame
    pub(super) fn center(&self, id: NodeId) -> Pos2 {
        self.app.graph.rects.iter().find(|(n, _)| *n == id).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("node {id:?} is not on screen"))
    }

    /// The node the 2D view shows once the graph has cooked
    pub(super) fn shown(&mut self) -> Option<NodeId> {
        self.app.session.cook();
        self.app.session.view2d_image().map(|(id, _)| id)
    }
}
