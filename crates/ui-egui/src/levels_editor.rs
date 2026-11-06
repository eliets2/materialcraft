use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};
use materialcraft_engine::core::{Node, NodeId, Value};
use materialcraft_engine::image::{Image, Kind};
use materialcraft_engine::nodes::atomic::levels_fn;
use serde_json::json;

use crate::MaterialCraftApp;
use crate::i18n::tr;
use crate::theme::Tokens;

/// Histogram bins (one per 8-bit level)
pub const BINS: usize = 256;

/// Smallest gap kept between the input low and high handles
const MIN_SPAN: f32 = 0.004;

/// Histograms of an image's values in 0..1, clamped: one for a gray image, else R, G and B
pub fn histogram(img: &Image) -> Vec<[u32; BINS]> {
    let channels = if img.kind == Kind::Gray { 1 } else { 3 };
    let stride = img.kind.channels();
    let mut h = vec![[0u32; BINS]; channels];
    for px in img.data.chunks(stride) {
        for (c, bins) in h.iter_mut().enumerate() {
            let v = px.get(c).copied().unwrap_or(0.0);
            let v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
            if let Some(b) = bins.get_mut(((v * (BINS - 1) as f32).round() as usize).min(BINS - 1)) {
                *b = b.saturating_add(1);
            }
        }
    }
    h
}

/// The bar height scale: the tallest bin away from pure black and white (masks pile up there and would flatten everything else), at least one
fn scale(h: &[[u32; BINS]]) -> f32 {
    let inner = h.iter().flat_map(|b| b.get(1..BINS - 1).unwrap_or(&[]).iter().copied()).max().unwrap_or(0);
    let all = h.iter().flat_map(|b| b.iter().copied()).max().unwrap_or(0);
    (if inner > 0 { inner } else { all }).max(1) as f32
}

/// The five levels values of `n`: in low, in mid (relative), in high, out low, out high
pub fn values(n: &Node) -> [f32; 5] {
    let get = |id: &str, default: f32| n.params.get(id).and_then(Value::as_f32).filter(|value| value.is_finite()).unwrap_or(default);
    [get("in_low", 0.0), get("in_mid", 0.5), get("in_high", 1.0), get("out_low", 0.0), get("out_high", 1.0)]
}

/// Where the mid handle sits on the 0..1 axis
pub fn mid_position(v: &[f32; 5]) -> f32 {
    v[0] + v[1].clamp(0.0, 1.0) * (v[2] - v[0])
}

/// The new value of handle `k` (0 in low, 1 in mid, 2 in high, 3 out low, 4 out high) dragged to `x` on the 0..1 axis: input low and high cannot cross, mid is stored relative to the range
pub fn drag_value(v: &[f32; 5], k: usize, x: f32) -> f32 {
    let x = if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
    let (lo, hi) = (v[0].min(v[2]), v[0].max(v[2]));
    match k {
        0 => x.min(hi - MIN_SPAN).max(0.0),
        2 => x.max(lo + MIN_SPAN).min(1.0),
        1 => {
            let span = v[2] - v[0];
            if span.abs() < 1e-6 { 0.5 } else { ((x - v[0]) / span).clamp(0.01, 0.99) }
        }
        _ => x,
    }
}

/// The histogram of what feeds the node's input, computed once per cooked result
fn input_histogram(app: &MaterialCraftApp, ui: &Ui, node: NodeId) -> Option<std::sync::Arc<Vec<[u32; BINS]>>> {
    let g = app.session.graph().ok()?;
    let c = g.input_of(node, "input")?;
    let port = app.session.output_ports(c.from).iter().position(|p| *p == c.from_port)?;
    let r = app.session.cooked.nodes.get(&c.from)?;
    let img = r.outputs.get(port)?.as_image()?;
    let id = egui::Id::unique(("levels-histogram", r.key, port));
    if let Some(h) = ui.ctx().data(|d| d.get_temp::<std::sync::Arc<Vec<[u32; BINS]>>>(id)) {
        return Some(h);
    }
    let h = std::sync::Arc::new(histogram(img));
    ui.ctx().data_mut(|d| d.insert_temp(id, h.clone()));
    Some(h)
}

/// A handle: a small triangle pointing up at `x`, filled with `fill`
fn handle(p: &egui::Painter, x: f32, top: f32, fill: Color32, active: bool, tk: &Tokens) {
    let s = 6.0;
    let pts = vec![pos2(x, top), pos2(x - s, top + s * 1.6), pos2(x + s, top + s * 1.6)];
    p.add(Shape::convex_polygon(pts, fill, Stroke::new(if active { 1.5 } else { 1.0 }, if active { tk.accent } else { Color32::from_gray(0x10) })));
}

/// The editor for Levels node `n`
pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui, n: &Node) {
    let tk = Tokens::get(ui.ctx());
    let v = values(n);
    // Handles of parameters driven by an input or a function are left alone
    let locked = |id: &str| n.bindings.contains_key(id) || n.functions.contains_key(id);
    let ids = ["in_low", "in_mid", "in_high", "out_low", "out_high"];
    let w = ui.available_width().max(120.0);
    let pad = 7.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, 96.0 + 16.0 + 12.0 + 16.0), Sense::hover());
    let axis = |x: f32| rect.left() + pad + x * (rect.width() - 2.0 * pad);
    let unaxis = |sx: f32| (sx - rect.left() - pad) / (rect.width() - 2.0 * pad).max(1.0);
    let plot = Rect::from_min_max(pos2(axis(0.0), rect.top()), pos2(axis(1.0), rect.top() + 96.0));
    let p = ui.painter_at(rect.expand(2.0));
    p.rect_filled(plot, 2.0, Color32::from_gray(0x16));

    // The input histogram
    match input_histogram(app, ui, n.id) {
        Some(h) => {
            let k = scale(&h);
            let colors: &[Color32] = if h.len() == 1 {
                &[Color32::from_gray(0xb8)]
            } else {
                &[
                    Color32::from_rgba_unmultiplied(0xe0, 0x50, 0x50, 110),
                    Color32::from_rgba_unmultiplied(0x50, 0xd0, 0x60, 110),
                    Color32::from_rgba_unmultiplied(0x50, 0x80, 0xf0, 110),
                ]
            };

            let bw = plot.width() / BINS as f32;
            for (bins, col) in h.iter().zip(colors) {
                for (i, &c) in bins.iter().enumerate() {
                    if c == 0 {
                        continue;
                    }
                    let hgt = (c as f32 / k).min(1.0) * (plot.height() - 2.0);
                    let x0 = plot.left() + i as f32 * bw;
                    p.rect_filled(Rect::from_min_max(pos2(x0, plot.bottom() - hgt), pos2(x0 + bw.max(1.0), plot.bottom())), 0.0, *col);
                }
            }
        }
        None => {
            p.text(plot.center(), egui::Align2::CENTER_CENTER, tr("Connect an input to see its histogram"), egui::FontId::proportional(10.5), tk.text_dim);
        }
    }
    // Shade what the input range clips, and draw the transfer curve
    let (lo, hi) = (v[0].min(v[2]), v[0].max(v[2]));
    let shade = Color32::from_black_alpha(120);
    p.rect_filled(Rect::from_min_max(plot.min, pos2(axis(lo), plot.bottom())), 0.0, shade);
    p.rect_filled(Rect::from_min_max(pos2(axis(hi), plot.top()), plot.max), 0.0, shade);
    let curve: Vec<Pos2> = (0..=96)
        .map(|i| {
            let x = i as f32 / 96.0;
            let y = levels_fn(x, v[0], v[1], v[2], v[3], v[4]);
            let y = if y.is_finite() { y.clamp(0.0, 1.0) } else { 0.0 };
            pos2(axis(x), plot.bottom() - y * plot.height())
        })
        .collect();
    p.add(Shape::line(curve, Stroke::new(1.5, tk.accent)));
    p.rect_stroke(plot, 2.0, Stroke::new(1.0, Color32::from_gray(0x40)), StrokeKind::Inside);

    // Input handles under the histogram, output handles under the gradient bar
    let in_row = plot.bottom() + 2.0;
    let bar = Rect::from_min_max(pos2(plot.left(), in_row + 16.0), pos2(plot.right(), in_row + 26.0));
    let mesh = {
        let mut m = egui::Mesh::default();
        m.colored_vertex(bar.left_top(), Color32::BLACK);
        m.colored_vertex(bar.right_top(), Color32::WHITE);
        m.colored_vertex(bar.right_bottom(), Color32::WHITE);
        m.colored_vertex(bar.left_bottom(), Color32::BLACK);
        m.add_triangle(0, 1, 2);
        m.add_triangle(0, 2, 3);
        m
    };

    p.add(Shape::mesh(mesh));
    let out_row = bar.bottom() + 2.0;
    let xs = [v[0], mid_position(&v), v[2], v[3], v[4]];
    let rows = [in_row, in_row, in_row, out_row, out_row];
    let fills = [Color32::from_gray(0x08), Color32::from_gray(0x80), Color32::WHITE, Color32::from_gray(0x08), Color32::WHITE];
    let names = ["Level in low", "Level in mid", "Level in high", "Level out low", "Level out high"];
    // Mid last, so it wins where it overlaps low or high
    for k in [0usize, 2, 3, 4, 1] {
        let x = axis(xs[k].clamp(0.0, 1.0));
        let hit = Rect::from_min_size(pos2(x - 7.0, rows[k] - 1.0), vec2(14.0, 13.0));
        let resp = ui.interact(hit, ui.scope_id().with(("levels-handle", n.id.0, k)), if locked(ids[k]) { Sense::hover() } else { Sense::drag() });
        crate::widgets::name(&resp, egui::Role::Slider, names[k]);
        let active = resp.dragged() || resp.hovered();
        handle(&p, x, rows[k], fills[k], active, &tk);
        if resp.dragged()
            && let Some(ptr) = resp.interact_pointer_pos()
        {
            let nv = drag_value(&v, k, unaxis(ptr.x));
            if (nv - v[k]).abs() > 1e-5 {
                app.run("graph.node.param", json!({"id": n.id.0, "param": ids[k], "value": nv}));
            }
        }
        resp.on_hover_text(format!("{}: {:.3}", tr(names[k]), v[k]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_counts_every_value_once_per_channel() {
        let gray = Image::gray(4, 4, |x, _| x as f32 / 3.0);
        let h = histogram(&gray);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].iter().sum::<u32>(), 16);
        assert_eq!((h[0][0], h[0][85], h[0][170], h[0][255]), (4, 4, 4, 4));
        let color = Image::filled(2, 2, Kind::Color, [2.0, f32::NAN, -1.0, 0.5]);
        let h = histogram(&color);
        assert_eq!(h.len(), 3);
        // Out of range and NaN values land in the end bins
        assert_eq!((h[0][255], h[1][0], h[2][0]), (4, 4, 4));
        // The scale ignores the end bins when there is anything in between
        assert_eq!(scale(&[[0; BINS]]), 1.0);
    }

    #[test]
    fn input_handles_do_not_cross_and_mid_follows_the_range() {
        let v = [0.2, 0.5, 0.6, 0.0, 1.0];
        assert!((mid_position(&v) - 0.4).abs() < 1e-6);
        // Low cannot pass high, high cannot pass low
        assert!(drag_value(&v, 0, 0.9) < 0.6);
        assert!(drag_value(&v, 2, 0.1) > 0.2);
        assert_eq!(drag_value(&v, 0, -1.0), 0.0);
        assert_eq!(drag_value(&v, 2, 2.0), 1.0);
        // Mid is stored relative to the range, kept inside it
        assert!((drag_value(&v, 1, 0.3) - 0.25).abs() < 1e-5);
        assert_eq!(drag_value(&v, 1, 0.0), 0.01);
        assert_eq!(drag_value(&v, 1, 1.0), 0.99);
        // Output handles may cross (that inverts); NaN never escapes
        assert_eq!(drag_value(&v, 3, 0.8), 0.8);
        assert_eq!(drag_value(&v, 4, f32::NAN), 0.0);
        // A collapsed range keeps mid centered
        assert_eq!(drag_value(&[0.5, 0.3, 0.5, 0.0, 1.0], 1, 0.7), 0.5);
    }
}
