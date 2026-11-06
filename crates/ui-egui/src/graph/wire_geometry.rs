use super::WireRef;
use egui::{Color32, Pos2, Rect, Stroke, pos2};

/// Pointer distance in points within which a wire is hit
pub(super) const WIRE_HIT: f32 = 5.0;

pub(crate) fn bezier_points(a: Pos2, b: Pos2) -> [Pos2; 4] {
    curve_points(a, b, egui::Vec2::X)
}

pub(crate) fn curve_points(source: Pos2, destination: Pos2, axis: egui::Vec2) -> [Pos2; 4] {
    let offset = axis * ((destination - source).dot(axis).abs() * 0.5).max(30.0);
    [source, source + offset, destination - offset, destination]
}

pub(crate) fn bezier(p: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    p.add(egui::epaint::CubicBezierShape::from_points_stroke(bezier_points(a, b), false, Color32::TRANSPARENT, stroke));
}

/// The point at `t` (0..1) along a cubic bezier
pub(super) fn bezier_at(c: &[Pos2; 4], t: f32) -> Pos2 {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    pos2(c.iter().zip(w).map(|(p, w)| p.x * w).sum(), c.iter().zip(w).map(|(p, w)| p.y * w).sum())
}

/// Distance from `p` to a cubic bezier (sampled as a polyline)
pub(super) fn bezier_distance(c: &[Pos2; 4], p: Pos2) -> f32 {
    const N: usize = 32;
    let mut best = f32::INFINITY;
    let mut prev = c[0];
    for i in 1..=N {
        let q = bezier_at(c, i as f32 / N as f32);
        let (seg, rel) = (q - prev, p - prev);
        let t = if seg.length_sq() > 0.0 { (rel.dot(seg) / seg.length_sq()).clamp(0.0, 1.0) } else { 0.0 };

        best = best.min((prev + seg * t).distance(p));
        prev = q;
    }
    best
}

/// The wire nearest `p` within [`WIRE_HIT`] points
pub(super) fn wire_at(wires: &[(WireRef, [Pos2; 4])], p: Pos2) -> Option<&WireRef> {
    wires
        .iter()
        .filter(|(_, c)| Rect::from_points(c).expand(WIRE_HIT).contains(p))
        .map(|(w, c)| (w, bezier_distance(c, p)))
        .filter(|(_, d)| *d <= WIRE_HIT)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w)
}
