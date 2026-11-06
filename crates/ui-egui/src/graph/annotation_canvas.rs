use egui::{Align2, Color32, FontId, Rect, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::{
    AnnotationSelection,
    core::{Comment, Frame, Pin},
};

use super::{GraphCamera, GraphDrag, frame_hit::FrameHit};
use crate::{MaterialCraftApp, theme::Tokens};

pub(crate) fn frames(app: &MaterialCraftApp, ui: &Ui, frames: &[Frame], camera: &GraphCamera) -> Vec<FrameHit> {
    let painter = ui.painter_at(app.graph.view);
    let theme = Tokens::get(ui.ctx());
    let mut hits = Vec::with_capacity(frames.len());

    let mut frames: Vec<_> = frames.iter().collect();
    frames.sort_by(|first, second| (second.rect[2] * second.rect[3]).total_cmp(&(first.rect[2] * first.rect[3])));

    for frame in frames {
        let mut rect = Rect::from_min_size(pos2(frame.rect[0], frame.rect[1]), vec2(frame.rect[2], frame.rect[3]));
        rect = rect.translate(offset(app, |contents| contents.frames.contains(&frame.id)));

        if let Some(GraphDrag::Frame { id, delta, resize: true, .. }) = &app.graph.drag
            && *id == frame.id
        {
            rect.max = rect.min + (rect.size() + *delta).max(Vec2::splat(10.0));
        }

        let rect = Rect::from_min_max(camera.to_screen(rect.min), camera.to_screen(rect.max));
        let color = frame.color;
        let channel = |value| (materialcraft_engine::core::linear_to_srgb(value) * 255.0).round() as u8;
        let fill = Color32::from_rgba_unmultiplied(channel(color[0]), channel(color[1]), channel(color[2]), (color[3] * 255.0).round() as u8);
        let selected = app.session.annotation_selection == Some(AnnotationSelection::Frame(frame.id));
        let outline = if selected { Stroke::new(2.0, theme.accent) } else { Stroke::new(1.0, Color32::from_black_alpha(180)) };

        painter.rect(rect, 0.0, fill, outline, StrokeKind::Outside);

        let title = if frame.title_visible {
            painter.text(
                rect.left_top() - vec2(0.0, 6.0 * camera.zoom),
                Align2::LEFT_BOTTOM,
                &frame.title,
                FontId::proportional((24.0 * camera.zoom).clamp(8.0, 64.0)),
                theme.text_strong,
            )
        } else {
            Rect::NOTHING
        };

        if !frame.description.is_empty() {
            let content = rect.shrink(6.0 * camera.zoom);
            let text =
                painter.layout(frame.description.clone(), FontId::proportional((14.0 * camera.zoom).clamp(6.0, 36.0)), theme.text, content.width().max(1.0));
            painter.with_clip_rect(content.intersect(app.graph.view)).galley(content.min, text, theme.text);
        }

        let grip = Rect::from_min_size(rect.max - vec2(12.0, 12.0), vec2(12.0, 12.0));
        painter.line_segment([grip.left_bottom(), grip.right_top()], Stroke::new(1.0, theme.text_dim));
        hits.push(FrameHit { id: frame.id, rect, title, grip });
    }

    hits
}

pub(crate) fn comments(app: &MaterialCraftApp, ui: &Ui, comments: &[Comment], camera: &GraphCamera) -> Vec<(u64, Rect)> {
    let painter = ui.painter_at(app.graph.view);
    let theme = Tokens::get(ui.ctx());
    let mut hits = Vec::with_capacity(comments.len());

    for comment in comments {
        let mut position = pos2(comment.pos[0], comment.pos[1]) + offset(app, |contents| contents.comments.contains(&comment.id));
        if let Some(GraphDrag::Comment { id, delta }) = &app.graph.drag
            && *id == comment.id
        {
            position += *delta;
        }

        let selected = app.session.annotation_selection == Some(AnnotationSelection::Comment(comment.id));
        let color = if selected { theme.accent } else { theme.text };

        let text = painter.layout(comment.text.clone(), FontId::proportional((14.0 * camera.zoom).clamp(6.0, 36.0)), color, comment.size[0] * camera.zoom);
        let rect = Rect::from_min_size(camera.to_screen(position), text.size().max(vec2(24.0, 16.0)));
        painter.galley(rect.min, text, color);
        hits.push((comment.id, rect));
    }

    hits
}

pub(crate) fn pins(app: &MaterialCraftApp, ui: &Ui, pins: &[Pin], camera: &GraphCamera) {
    let painter = ui.painter_at(app.graph.view);
    let theme = Tokens::get(ui.ctx());

    for pin in pins {
        let position = pos2(pin.pos[0], pin.pos[1]) + offset(app, |contents| contents.pins.contains(&pin.id));
        let position = camera.to_screen(position);
        painter.text(position, Align2::CENTER_BOTTOM, icon::MAP_PIN, FontId::proportional(18.0), theme.accent);
        painter.text(position + vec2(10.0, -10.0), Align2::LEFT_CENTER, &pin.name, FontId::proportional(11.0), theme.text);
    }
}

pub(crate) fn offset(app: &MaterialCraftApp, contains: impl FnOnce(&materialcraft_engine::FrameContents) -> bool) -> Vec2 {
    match &app.graph.drag {
        Some(GraphDrag::Frame { delta, resize: false, contents, .. }) if contains(contents) => {
            materialcraft_engine::graph_grid::snap_position([delta.x, delta.y], app.ui.prefs.snap_to_grid).into()
        }
        _ => Vec2::ZERO,
    }
}
