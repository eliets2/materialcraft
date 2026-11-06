use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Ui, vec2};

pub(crate) fn paint(painter: &Painter, position: Pos2, anchor: Align2, label: &str, zoom: f32, color: Color32) {
    let font = FontId::proportional((10.5 * zoom).clamp(6.0, 14.0));
    let text = painter.layout_no_wrap(label.to_string(), font, color);
    let bounds = anchor.anchor_size(position, text.size()).expand(2.0);
    painter.rect_filled(bounds, 2.0, Color32::from_black_alpha(160));
    painter.galley(bounds.min + vec2(2.0, 2.0), text, color);
}

pub(crate) fn visible(ui: &Ui, bounds: Rect) -> bool {
    ui.input(|input| input.pointer.hover_pos().is_some()) && ui.rect_contains_pointer(bounds.expand(8.0))
}
