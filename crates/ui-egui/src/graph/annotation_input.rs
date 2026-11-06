use super::{GraphCamera, GraphDrag, frame_hit::FrameHit};
use crate::MaterialCraftApp;
use egui::{Rect, Ui, Vec2};
use serde_json::json;

pub(crate) fn handle(app: &mut MaterialCraftApp, ui: &Ui, camera: &GraphCamera, frames: &[FrameHit], comments: &[(u64, Rect)]) {
    if super::gesture::cancelled(ui) {
        app.graph.drag = None;
        return;
    }

    let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) else { return };

    let pressed = ui.input(|input| input.pointer.primary_pressed());
    let motion = ui.input(|input| input.pointer.delta()) / camera.zoom;

    if pressed && app.graph.view.contains(pointer) && app.graph.drag.is_none() && !app.graph.rects.iter().any(|(_, rect)| rect.contains(pointer)) {
        if let Some((id, _)) = comments.iter().rev().find(|(_, rect)| rect.contains(pointer)) {
            begin_comment(app, *id);
        } else if let Some(hit) = frames.iter().rev().find(|hit| hit.contains(pointer)) {
            begin_frame(app, hit.id, hit.grip.contains(pointer));
        } else {
            app.run("graph.annotation.select", json!({}));
        }
    } else if let Some(GraphDrag::Frame { delta, .. } | GraphDrag::Comment { delta, .. }) = &mut app.graph.drag {
        *delta += motion;
    }

    if super::gesture::finished(ui, egui::PointerButton::Primary)
        && let Some(drag) = app.graph.drag.take()
    {
        finish(app, drag);
    }
}

pub(super) fn begin_comment(app: &mut MaterialCraftApp, id: u64) {
    app.run("graph.annotation.select", json!({"kind": "comment", "id": id}));
    app.graph.drag = Some(GraphDrag::Comment { id, delta: Vec2::ZERO });
}

pub(super) fn begin_frame(app: &mut MaterialCraftApp, id: u64, resize: bool) {
    let contents = app
        .session
        .graph()
        .ok()
        .and_then(|graph| graph.frames.iter().find(|frame| frame.id == id).map(|frame| materialcraft_engine::FrameContents::capture(graph, frame)))
        .unwrap_or_default();
    app.run("graph.annotation.select", json!({"kind": "frame", "id": id}));
    app.graph.drag = Some(GraphDrag::Frame { id, delta: Vec2::ZERO, resize, contents });
}

pub(super) fn finish(app: &mut MaterialCraftApp, drag: GraphDrag) {
    match drag {
        GraphDrag::Frame { id, delta, resize, .. } if delta.length() > 0.5 => {
            if resize {
                if let Some(frame) = app.session.graph().ok().and_then(|graph| graph.frames.iter().find(|frame| frame.id == id)) {
                    let rect = [frame.rect[0], frame.rect[1], frame.rect[2] + delta.x, frame.rect[3] + delta.y];
                    app.run("graph.frame.edit", json!({"id": id, "rect": rect}));
                }
            } else {
                app.run("graph.frame.move", json!({"id": id, "by": [delta.x, delta.y]}));
            }
        }
        GraphDrag::Comment { id, delta } if delta.length() > 0.5 => {
            if let Some(comment) = app.session.graph().ok().and_then(|graph| graph.comments.iter().find(|comment| comment.id == id)) {
                let position = [comment.pos[0] + delta.x, comment.pos[1] + delta.y];
                app.run("graph.comment.edit", json!({"id": id, "at": position}));
            }
        }
        _ => {}
    }
}
