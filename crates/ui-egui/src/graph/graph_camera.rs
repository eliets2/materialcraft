use egui::{Pos2, Rect, Ui, Vec2};

pub(crate) struct GraphCamera {
    pub(crate) origin: Pos2,
    pub(crate) pan: Vec2,
    pub(crate) zoom: f32,
}

impl GraphCamera {
    pub(crate) fn to_screen(&self, position: Pos2) -> Pos2 {
        self.origin + (position.to_vec2() - self.pan) * self.zoom
    }

    pub(crate) fn to_graph(&self, position: Pos2) -> Pos2 {
        ((position - self.origin) / self.zoom + self.pan).to_pos2()
    }

    pub(crate) fn state(&self) -> crate::state::GraphCamera {
        crate::state::GraphCamera { pan: self.pan.into(), zoom: self.zoom }
    }

    pub(crate) fn fit(view: Rect, destination: Rect) -> Self {
        if !destination.is_finite() || destination.width() <= 0.0 {
            let state = crate::state::GraphCamera::default();
            return Self { origin: view.min, pan: state.pan.into(), zoom: state.zoom };
        }

        let destination = destination.expand(40.0);
        let zoom = (view.width() / destination.width()).min(view.height() / destination.height()).clamp(0.1, 1.5);
        let pan = destination.center().to_vec2() - view.size() * 0.5 / zoom;
        Self { origin: view.min, pan, zoom }
    }

    pub(crate) fn zoom_from_input(&mut self, ui: &Ui, view: Rect) {
        self.zoom = self.zoom.clamp(crate::state::GraphCamera::ZOOM.0, crate::state::GraphCamera::ZOOM.1);
        if !ui.rect_contains_pointer(view) {
            return;
        }

        let Some(pointer) = ui.input(|input| input.pointer.hover_pos()).filter(|pointer| view.contains(*pointer)) else { return };

        let scroll = ui.input(|input| input.smooth_scroll_delta.y + input.zoom_delta().ln() * 200.0);
        if scroll.is_finite() && scroll != 0.0 {
            let before = self.to_graph(pointer);
            self.zoom = (self.zoom * (scroll * 0.0025).exp()).clamp(crate::state::GraphCamera::ZOOM.0, crate::state::GraphCamera::ZOOM.1);
            self.pan = before.to_vec2() - (pointer - self.origin) / self.zoom;
        }
    }

    pub(crate) fn pan_started(ui: &Ui, view: Rect) -> bool {
        ui.rect_contains_pointer(view)
            && ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Middle) || input.pointer.primary_pressed() && input.modifiers.alt)
    }

    pub(crate) fn frame_key(ui: &Ui, view: Rect) -> bool {
        ui.rect_contains_pointer(view)
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|input| input.key_pressed(egui::Key::F) && input.modifiers.is_none())
    }
}
