use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::PatternNodeKind;

use super::geometry;
use crate::i18n::tr;

pub(super) fn paint(painter: &Painter, bounds: Rect, kind: PatternNodeKind, zoom: f32, outline: Color32) {
    if kind == PatternNodeKind::Dot {
        painter.circle(bounds.center(), 6.0 * zoom, Color32::from_gray(180), Stroke::new(2.0, outline));
        return;
    }
    let color = match kind {
        PatternNodeKind::Quadrant => Color32::from_rgb(230, 157, 31),
        PatternNodeKind::Switch => Color32::from_rgb(75, 153, 109),
        PatternNodeKind::Iterate => Color32::from_rgb(79, 131, 185),
        PatternNodeKind::Dot => Color32::from_gray(180),
    };
    painter.rect_filled(bounds, 2.0, color);
    let header = Rect::from_min_size(bounds.min, vec2(bounds.width(), geometry::HEADER_HEIGHT * zoom));
    painter.rect_filled(header, 2.0, color.gamma_multiply(0.75));
    painter.text(header.center(), Align2::CENTER_CENTER, tr(kind.label()), FontId::proportional(11.0 * zoom), Color32::WHITE);
    let symbol = match kind {
        PatternNodeKind::Quadrant => icon::GRID_FOUR,
        PatternNodeKind::Switch => icon::GIT_BRANCH,
        PatternNodeKind::Iterate => icon::ARROWS_CLOCKWISE,
        PatternNodeKind::Dot => icon::CIRCLE,
    };
    painter.text(bounds.center() + vec2(0.0, 8.0 * zoom), Align2::CENTER_CENTER, symbol, FontId::proportional(48.0 * zoom), Color32::from_gray(35));
    painter.rect_stroke(bounds, 2.0, Stroke::new(2.0, outline), StrokeKind::Outside);
}
