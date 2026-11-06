use crate::theme::Tokens;
use egui::{Pos2, Sense, Stroke, Ui, pos2, vec2};
use materialcraft_engine::core::Curve;

/// A tone curve editor: drag points, double-click to add one, right-click a point to remove it. Returns the new curve when it changed
pub fn curve_editor(ui: &mut Ui, id: &str, curve: &Curve) -> Option<Curve> {
    let tokens = Tokens::get(ui.ctx());
    let width = ui.available_width().min(240.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, width * 0.75), Sense::click_and_drag());
    crate::widgets::name(&response, egui::Role::Canvas, "Curve editor");
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, tokens.field);
    for i in 1..4 {
        let t = i as f32 / 4.0;
        painter.line_segment(
            [pos2(rect.left() + rect.width() * t, rect.top()), pos2(rect.left() + rect.width() * t, rect.bottom())],
            Stroke::new(1.0, tokens.separator),
        );
        painter.line_segment(
            [pos2(rect.left(), rect.top() + rect.height() * t), pos2(rect.right(), rect.top() + rect.height() * t)],
            Stroke::new(1.0, tokens.separator),
        );
    }
    let to_screen = |x: f32, y: f32| pos2(rect.left() + x * rect.width(), rect.bottom() - y * rect.height());
    let from_screen = |s: Pos2| (((s.x - rect.left()) / rect.width()).clamp(0.0, 1.0), ((rect.bottom() - s.y) / rect.height()).clamp(0.0, 1.0));
    let curve_points: Vec<Pos2> = (0..=64).map(|i| i as f32 / 64.0).map(|x| to_screen(x, curve.eval(x).clamp(0.0, 1.0))).collect();
    painter.add(egui::Shape::line(curve_points, Stroke::new(2.0, tokens.accent)));
    let drag_id = ui.make_persistent_id(id).with("drag");
    let mut dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id));
    let pointer = response.interact_pointer_pos().or(response.hover_pos());
    let nearest = pointer.and_then(|pointer_position| {
        curve
            .points
            .iter()
            .enumerate()
            .map(|(i, point)| (i, to_screen(point.x, point.y).distance(pointer_position)))
            .filter(|(_, d)| *d < 10.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    });
    for (i, point) in curve.points.iter().enumerate() {
        let hot = Some(i) == nearest || Some(i) == dragging;
        painter.circle(
            to_screen(point.x, point.y),
            if hot { 5.0 } else { 4.0 },
            tokens.panel,
            Stroke::new(1.5, if hot { tokens.text_strong } else { tokens.text }),
        );
    }
    let mut edited_curve: Option<Curve> = None;
    if response.drag_started() {
        dragging = nearest;
    }
    if response.dragged()
        && let (Some(i), Some(pointer_position)) = (dragging, pointer)
    {
        let (x, y) = from_screen(pointer_position);
        let mut updated_curve = curve.clone();
        let last = updated_curve.points.len().saturating_sub(1);
        if let Some(point) = updated_curve.points.get_mut(i) {
            // End points keep their x
            if i != 0 && i != last {
                point.x = x;
            }
            point.y = y;
        }
        updated_curve.points.sort_by(|a, b| a.x.total_cmp(&b.x));
        edited_curve = Some(updated_curve);
    }
    if response.drag_stopped() {
        dragging = None;
    }
    ui.data_mut(|d| d.insert_temp(drag_id, dragging));
    if response.double_clicked()
        && let Some(pointer_position) = pointer
    {
        let (x, _) = from_screen(pointer_position);
        let mut updated_curve = curve.clone();
        updated_curve.points.push(materialcraft_engine::core::CurvePoint::new(x, curve.eval(x)));
        updated_curve.points.sort_by(|a, b| a.x.total_cmp(&b.x));
        edited_curve = Some(updated_curve);
    }
    if response.secondary_clicked()
        && let Some(i) = nearest
        && i != 0
        && i + 1 != curve.points.len()
    {
        let mut updated_curve = curve.clone();
        updated_curve.points.remove(i);
        edited_curve = Some(updated_curve);
    }
    edited_curve
}
