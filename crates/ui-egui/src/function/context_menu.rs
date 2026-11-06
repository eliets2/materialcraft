use super::{FunctionEditor, FunctionTarget};
use crate::{MaterialCraftApp, graph::edit_menu, i18n::tr, menus};
use egui::Ui;
use materialcraft_engine::FunctionEdit;
use serde_json::json;

pub(super) fn show(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui) {
    let Some(target) = editor.command_target() else { return };

    let mut add = None;
    let standalone = matches!(editor.target, FunctionTarget::Graph);
    ui.set_min_width(290.0);
    ui.menu_button(tr("Add node"), |ui| {
        let entries = target
            .available_operations(&app.session)
            .filter(|operation| operation.id != "passthrough")
            .map(|operation| (operation.id, operation.label, operation.category));
        add = crate::graph::add_node_menu::show(ui, entries);
    });
    if standalone {
        for (label, command, parameters) in [
            ("Add Comment", "graph.comment.add", json!({"at": [editor.menu_at.x, editor.menu_at.y]})),
            ("Add Frame", "graph.frame.add", json!({"rect": [editor.menu_at.x, editor.menu_at.y, 400, 260]})),
            ("Add Navigation pin", "graph.pin.add", json!({"at": [editor.menu_at.x, editor.menu_at.y]})),
        ] {
            if edit_menu::button(ui, label, command, true) {
                app.run(command, parameters);
                editor.selection.clear();
            }
        }
    }
    if ui.button(tr("Add Dot node")).clicked() {
        add = Some("passthrough");
        ui.close();
    }
    ui.separator();
    if edit_menu::button(ui, "Save package", "file.save", menus::enabled(app, "file.save")) {
        menus::invoke(app, ui.ctx(), "file.save");
    }
    if let Some(command) =
        edit_menu::selection(ui, |command| super::actions::editor_enabled(app, editor, command).unwrap_or_else(|| menus::enabled(app, command)))
        && !super::actions::invoke_editor(app, editor, command)
    {
        app.run(command, json!({}));
    }
    if standalone {
        for (label, command) in [("Previous pin", "ui.graph.previousPin"), ("Next pin", "ui.graph.nextPin")] {
            if edit_menu::button(ui, label, command, menus::enabled(app, command)) {
                crate::graph::actions::invoke(app, command);
                editor.set_camera(app.ui.graph_camera);
            }
        }
    }
    if let Some(operation) = add {
        editor.edit(app, FunctionEdit::Add { operation: operation.into(), at: editor.menu_at.into(), snap: app.ui.prefs.snap_to_grid }, false);
    }
}
