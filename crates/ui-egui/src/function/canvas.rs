use super::{FunctionEditor, FunctionTarget};
use crate::{MaterialCraftApp, graph::GraphCamera, i18n::tr};
use egui::Ui;
use materialcraft_engine::FunctionEdit;
use serde_json::json;

pub(super) fn canvas(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui) {
    let Some(frame) = super::render::paint(app, editor, ui) else { return };

    let standalone = matches!(editor.target, FunctionTarget::Graph);
    app.ui.frame_all = false;
    if standalone {
        app.ui.graph_camera = frame.camera.state();
        app.graph.frames = frame.frames.iter().map(|frame| (frame.id, frame.rect)).collect();
        app.graph.comments = frame.comments.clone();
    }
    if frame.clear_annotation {
        app.run("graph.annotation.select", json!({}));
    }
    for (id, value) in frame.values {
        editor.edit(app, FunctionEdit::Value { id, value }, true);
    }
    for (id, response) in &frame.nodes {
        if response.secondary_clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            editor.menu_at = frame.camera.to_graph(pointer);
        }
        response.context_menu(|ui| {
            super::context_menu::show(app, editor, ui);
            ui.separator();
            if ui.button(format!("{}  {}", egui_phosphor::regular::STAR, tr("Set as output node"))).clicked() {
                editor.edit(app, FunctionEdit::Output { id: *id }, false);
                ui.close();
            }
        });
    }
    let on_annotation = ui
        .input(|input| input.pointer.hover_pos())
        .is_some_and(|pointer| frame.frames.iter().any(|frame| frame.contains(pointer)) || frame.comments.iter().any(|(_, rect)| rect.contains(pointer)));
    super::input::handle(app, editor, ui, &frame.response, &frame.ports, &frame.geometry.rects, on_annotation);
    frame.geometry.publish(&mut app.graph, frame.response.rect);
    if standalone {
        if editor.drag.is_none() {
            let camera = GraphCamera { origin: frame.response.rect.min, pan: editor.pan, zoom: editor.zoom };
            crate::graph::annotation_input::handle(app, ui, &camera, &frame.frames, &frame.comments);
            if app.session.annotation_selection.is_some() {
                editor.selection.clear();
            }
        }
        app.ui.graph_camera = crate::state::GraphCamera { pan: editor.pan.into(), zoom: editor.zoom };
        app.session.selection = editor.selection.iter().map(|id| materialcraft_engine::core::NodeId(u64::from(*id))).collect();
    } else {
        app.graph.frames.clear();
        app.graph.comments.clear();
    }
    editor.inspected_node = crate::graph::node_selection::inspected(&editor.selection, editor.inspected_node);
}
