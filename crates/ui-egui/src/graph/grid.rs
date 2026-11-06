use super::GraphCamera;
use egui::{Color32, Rect, Stroke, pos2};

pub(crate) fn paint(p: &egui::Painter, cam: &GraphCamera, rect: Rect) {
    for (step, alpha) in [(materialcraft_engine::graph_grid::GRID_SPACING, 10u8), (256.0, 22)] {
        let s = step * cam.zoom;
        if s < 8.0 {
            continue;
        }
        let col = Color32::from_white_alpha(alpha);
        let x0 = cam.pan.x - (cam.pan.x.rem_euclid(step));
        let mut x = x0;
        while cam.to_screen(pos2(x, 0.0)).x < rect.right() {
            let sx = cam.to_screen(pos2(x, 0.0)).x;
            p.line_segment([pos2(sx, rect.top()), pos2(sx, rect.bottom())], Stroke::new(1.0, col));
            x += step;
        }
        let y0 = cam.pan.y - (cam.pan.y.rem_euclid(step));
        let mut y = y0;
        while cam.to_screen(pos2(0.0, y)).y < rect.bottom() {
            let sy = cam.to_screen(pos2(0.0, y)).y;
            p.line_segment([pos2(rect.left(), sy), pos2(rect.right(), sy)], Stroke::new(1.0, col));
            y += step;
        }
    }
}
