use egui::{PointerButton, Response, Ui};
use materialcraft_engine::PatternEdit;

use super::{PatternDrag, PatternEditor};
use crate::{MaterialCraftApp, graph::GraphCamera};

pub(super) fn handle(app: &mut MaterialCraftApp, editor: &mut PatternEditor, ui: &Ui, response: &Response, camera: &GraphCamera) {
    if crate::graph::gesture::cancelled(ui) {
        editor.drag = None;
        return;
    }
    let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) else { return };

    let position = camera.to_graph(pointer);
    let port = crate::graph::port_hit::nearest_by(&app.graph.ports, pointer, |port| (port.pos, (6.0 * editor.zoom).clamp(1.5, 8.0))).map(|port| {
        (
            port.node.0 as u32,
            port.output.then(|| port.port.strip_prefix("output").and_then(|slot| if slot.is_empty() { Some(0) } else { slot.parse::<usize>().ok() })).flatten(),
        )
    });
    let node = app.graph.rects.iter().rev().find(|(_, bounds)| bounds.contains(pointer)).map(|(id, _)| id.0 as u32);
    let modifiers = ui.input(|input| input.modifiers);

    if editor.drag.is_none()
        && GraphCamera::pan_started(ui, response.rect)
        && !(port.is_some() && modifiers.alt && ui.input(|input| input.pointer.primary_pressed()))
    {
        editor.drag = Some(PatternDrag::Pan(crate::graph::gesture::pan_button(ui)));
    } else if editor.drag.is_none() && response.hovered() && ui.input(|input| input.pointer.button_pressed(PointerButton::Primary)) {
        if let Some((id, slot)) = port {
            if modifiers.alt {
                editor.edit(app, PatternEdit::Disconnect { node: id, slot }, false);
            } else {
                editor.drag = Some(PatternDrag::Wire { node: id, slot });
            }
        } else if let Some(id) = node {
            crate::graph::node_selection::select(&mut editor.selection, Some(id), modifiers);
            if editor.selection.contains(&id) {
                editor.drag = Some(PatternDrag::Nodes { start: position, delta: egui::Vec2::ZERO });
            }
        } else {
            crate::graph::node_selection::select(&mut editor.selection, None, modifiers);
            editor.drag = Some(PatternDrag::Box { start: pointer });
        }
    }
    match &mut editor.drag {
        Some(PatternDrag::Nodes { start, delta }) => *delta = position - *start,
        Some(PatternDrag::Pan(_)) => editor.pan -= ui.input(|input| input.pointer.delta()) / editor.zoom,
        Some(PatternDrag::Box { start, .. }) => {
            crate::graph::node_selection::paint_box(ui.painter(), *start, pointer, crate::theme::Tokens::get(ui.ctx()).accent);
        }
        _ => {}
    }

    let owner = match editor.drag {
        Some(PatternDrag::Pan(button)) => button,
        _ => PointerButton::Primary,
    };
    if crate::graph::gesture::finished(ui, owner) {
        match editor.drag.take() {
            Some(PatternDrag::Nodes { delta, .. }) if delta.length() > 0.5 => {
                editor.edit(app, PatternEdit::Move { ids: editor.selection.clone(), by: delta.into(), snap: app.ui.prefs.snap_to_grid }, false);
            }
            Some(PatternDrag::Wire { node, slot }) => {
                if let Some((target, target_slot)) = port {
                    let connection = match (slot, target_slot) {
                        (Some(slot), None) => Some((node, slot, target)),
                        (None, Some(slot)) => Some((target, slot, node)),
                        _ => None,
                    };
                    if let Some((parent, slot, child)) = connection {
                        editor.edit(app, PatternEdit::Connect { parent, slot, child }, false);
                    }
                }
            }
            Some(PatternDrag::Box { start }) => {
                let nodes = app.graph.rects.iter().map(|(id, rect)| (id.0 as u32, *rect));
                crate::graph::node_selection::select_box(&mut editor.selection, nodes, start, pointer, modifiers);
            }
            _ => {}
        }
    }
    if response.secondary_clicked() {
        editor.menu_at = position;
        if let Some(id) = node
            && !editor.selection.contains(&id)
        {
            crate::graph::node_selection::select(&mut editor.selection, Some(id), egui::Modifiers::NONE);
        }
    }
    if response.hovered() && !ui.ctx().egui_wants_keyboard_input() && ui.input(|input| input.key_pressed(egui::Key::Space)) {
        editor.menu_at = position;
        egui::Popup::open_id(ui.ctx(), response.id.with("menu"));
    }
}
