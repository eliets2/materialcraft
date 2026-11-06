use egui::{Align2, FontId, Response, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::{theme::Tokens, widgets};

pub(super) fn show(ui: &mut Ui, name: &str, icon: &str, selected: bool) -> Response {
    let theme = Tokens::get(ui.ctx());
    let (bounds, response) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
    widgets::name(&response, egui::Role::TreeItem, name);
    if selected {
        ui.painter().rect_filled(bounds, 4.0, theme.row_selected);
        ui.painter().rect_stroke(bounds, 4.0, Stroke::new(1.0, theme.accent), StrokeKind::Inside);
    } else if response.hovered() {
        ui.painter().rect_filled(bounds, 4.0, theme.row_hover);
    }

    ui.painter().text(
        bounds.left_center() + vec2(4.0, 0.0),
        Align2::LEFT_CENTER,
        format!("{icon}  {name}"),
        FontId::proportional(12.0),
        if selected { theme.text_strong } else { theme.text },
    );
    response
}
