use crate::theme::Tokens;
use egui::{Sense, Ui, vec2};
use egui_phosphor::regular as icon;

pub(crate) fn section(ui: &mut Ui, id: &str, title: &str, body: impl FnOnce(&mut Ui)) {
    let tokens = Tokens::get(ui.ctx());
    let id = ui.make_persistent_id(id);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    crate::widgets::name(&response, egui::Role::Button, title);
    ui.painter().rect_filled(rect, 3.0, tokens.row);
    let chevron = if state.is_open() { icon::CARET_DOWN } else { icon::CARET_RIGHT };
    ui.painter().text(rect.left_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, chevron, egui::FontId::proportional(10.0), tokens.text_dim);
    ui.painter().text(
        rect.left_center() + vec2(20.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title.to_uppercase(),
        egui::FontId::proportional(10.5),
        tokens.text_strong,
    );
    if response.clicked() {
        state.toggle(ui);
    }
    state.show_body_unindented(ui, |ui| {
        ui.add_space(3.0);
        body(ui);
        ui.add_space(6.0);
    });
}
