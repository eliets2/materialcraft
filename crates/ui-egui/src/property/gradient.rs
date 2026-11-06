use crate::{i18n::tr, theme::Tokens};
use egui::{Color32, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use materialcraft_engine::core::Gradient;

/// A gradient editor: the ramp with its keys below; drag keys, click the ramp to add one, pick a key's color, right-click a key to remove it
pub fn gradient_editor(ui: &mut Ui, id: &str, gradient: &Gradient) -> Option<Gradient> {
    let tokens = Tokens::get(ui.ctx());
    let width = ui.available_width().min(260.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click_and_drag());
    crate::widgets::name(&response, egui::Role::Canvas, "Gradient editor");
    let painter = ui.painter_at(rect.expand(6.0));
    let bar = Rect::from_min_size(rect.min, vec2(width, 18.0));
    let n = 64;
    for i in 0..n {
        let t0 = i as f32 / n as f32;
        let c = gradient.eval(t0 + 0.5 / n as f32);
        let q = |v: f32| (materialcraft_engine::image::linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0) as u8;
        let r = Rect::from_min_max(pos2(bar.left() + bar.width() * t0, bar.top()), pos2(bar.left() + bar.width() * (t0 + 1.0 / n as f32) + 0.5, bar.bottom()));
        painter.rect_filled(r, 0.0, Color32::from_rgb(q(c[0]), q(c[1]), q(c[2])));
    }
    painter.rect_stroke(bar, 2.0, Stroke::new(1.0, tokens.field_border), egui::StrokeKind::Inside);
    let selection_id = ui.make_persistent_id(id).with("sel");
    let mut selected: usize = ui.data(|d| d.get_temp(selection_id)).unwrap_or(0);
    let key_x = |pos: f32| bar.left() + pos * bar.width();
    let pointer = response.interact_pointer_pos().or(response.hover_pos());
    let nearest = pointer.and_then(|q| {
        gradient
            .keys
            .iter()
            .enumerate()
            .map(|(i, k)| (i, (key_x(k.pos) - q.x).abs()))
            .filter(|(_, d)| *d < 7.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    });
    for (i, k) in gradient.keys.iter().enumerate() {
        let x = key_x(k.pos);
        let triangle = vec![pos2(x, bar.bottom() + 1.0), pos2(x - 5.0, bar.bottom() + 10.0), pos2(x + 5.0, bar.bottom() + 10.0)];
        painter.add(egui::Shape::convex_polygon(triangle, if i == selected { tokens.accent } else { tokens.text }, Stroke::NONE));
    }
    let mut edited_gradient: Option<Gradient> = None;
    if response.drag_started()
        && let Some(i) = nearest
    {
        selected = i;
    }
    if response.dragged()
        && let Some(q) = pointer
        && nearest.is_some()
    {
        let mut updated_gradient = gradient.clone();
        if let Some(k) = updated_gradient.keys.get_mut(selected) {
            k.pos = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
        }
        let moved = updated_gradient.keys.get(selected).copied();
        updated_gradient.sort();
        if let Some(m) = moved {
            selected = updated_gradient.keys.iter().position(|k| k.pos == m.pos && k.color == m.color).unwrap_or(selected);
        }
        edited_gradient = Some(updated_gradient);
    }
    if response.clicked()
        && let Some(q) = pointer
    {
        match nearest {
            Some(i) => selected = i,
            None => {
                let t = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                let mut updated_gradient = gradient.clone();
                updated_gradient.keys.push(materialcraft_engine::core::GradientKey::new(t, gradient.eval(t)));
                updated_gradient.sort();
                selected = updated_gradient.keys.iter().position(|k| k.pos == t).unwrap_or(0);
                edited_gradient = Some(updated_gradient);
            }
        }
    }
    if response.secondary_clicked()
        && let Some(i) = nearest
        && gradient.keys.len() > 1
    {
        let mut updated_gradient = gradient.clone();
        updated_gradient.keys.remove(i);
        selected = 0;
        edited_gradient = Some(updated_gradient);
    }
    ui.data_mut(|d| d.insert_temp(selection_id, selected));
    if let Some(k) = gradient.keys.get(selected) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {:.2}", tr("Key at"), k.pos)).size(11.0).color(tokens.text_dim));
            let mut rgb = [k.color[0], k.color[1], k.color[2]];
            if crate::widgets::color_swatch(ui, &format!("{id}-key"), &mut rgb, 60.0) {
                let mut updated_gradient = edited_gradient.clone().unwrap_or_else(|| gradient.clone());
                if let Some(selected_key) = updated_gradient.keys.get_mut(selected) {
                    selected_key.color = [rgb[0], rgb[1], rgb[2], selected_key.color[3]];
                }
                edited_gradient = Some(updated_gradient);
            }
        });
    }
    edited_gradient
}
