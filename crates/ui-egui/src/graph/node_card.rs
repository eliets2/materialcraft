use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

pub(crate) const WIDTH: f32 = 100.0;
pub(crate) const HEADER_HEIGHT: f32 = 18.0;
pub(crate) const FOOTER_HEIGHT: f32 = 14.0;
const PORT_GAP: f32 = 18.0;
const DOT_RADIUS: f32 = 7.0;

pub(crate) fn bounds(position: [f32; 2], inputs: usize, outputs: usize, previews: bool, dot: bool) -> Rect {
    let position = pos2(position[0], position[1]);
    if dot {
        return Rect::from_center_size(position, vec2(DOT_RADIUS * 2.0, DOT_RADIUS * 2.0));
    }

    let ports = inputs.max(outputs) as f32 * PORT_GAP + 8.0;
    let body = if previews { WIDTH.max(ports) } else { ports.max(26.0) };
    Rect::from_min_size(position, vec2(WIDTH, HEADER_HEIGHT + body + FOOTER_HEIGHT))
}

pub(crate) fn port(bounds: Rect, index: usize, count: usize, output: bool, dot: bool) -> Pos2 {
    let x = if output { bounds.right() } else { bounds.left() };
    if dot {
        return pos2(x, bounds.center().y);
    }

    let body_height = bounds.height() - HEADER_HEIGHT - FOOTER_HEIGHT;
    let first = bounds.top() + HEADER_HEIGHT + (body_height - count as f32 * PORT_GAP) * 0.5 + PORT_GAP * 0.5;
    pos2(x, first + index as f32 * PORT_GAP)
}

pub(crate) fn header(bounds: Rect, zoom: f32) -> Rect {
    Rect::from_min_size(bounds.min, vec2(bounds.width(), HEADER_HEIGHT * zoom))
}

pub(crate) fn body(bounds: Rect, zoom: f32) -> Rect {
    Rect::from_min_max(pos2(bounds.left(), bounds.top() + HEADER_HEIGHT * zoom), pos2(bounds.right(), bounds.bottom() - FOOTER_HEIGHT * zoom))
}

pub(crate) fn paint(painter: &Painter, bounds: Rect, zoom: f32, label: &str, color: Color32) {
    let radius = (5.0 * zoom).clamp(1.0, 6.0) as u8;
    let header = header(bounds, zoom);
    painter.rect_filled(bounds, radius, Color32::from_rgb(0x1e, 0x20, 0x24));
    painter.rect_filled(header, CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, color);
    let font = FontId::proportional((10.5 * zoom).clamp(6.0, 16.0));
    let title = crate::widgets::elided(painter, label, font.size, Color32::WHITE, header.width() - 8.0);
    painter.galley(header.left_center() + vec2(4.0, -title.size().y * 0.5), title, Color32::WHITE);
}

pub(crate) fn outline(painter: &Painter, bounds: Rect, zoom: f32, stroke: Stroke) {
    painter.rect_stroke(bounds, (5.0 * zoom).clamp(1.0, 6.0) as u8, stroke, StrokeKind::Outside);
}

pub(crate) fn paint_dot(painter: &Painter, bounds: Rect, color: Color32) {
    painter.circle(bounds.center(), bounds.width() * 0.5, Color32::from_gray(0x50), Stroke::new(1.5, color));
}
