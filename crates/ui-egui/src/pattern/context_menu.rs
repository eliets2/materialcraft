use super::PatternEditor;
use crate::{MaterialCraftApp, graph::edit_menu, i18n::tr, menus};
use egui::Ui;
use materialcraft_engine::{PatternEdit, core::PatternNodeKind};

pub(super) fn show(app: &mut MaterialCraftApp, editor: &mut PatternEditor, ui: &mut Ui) {
    let mut add = None;
    ui.menu_button(tr("Add node"), |ui| {
        let entries = PatternNodeKind::ALL.into_iter().filter(|kind| *kind != PatternNodeKind::Dot).map(|kind| (kind.native_id(), kind.label(), "FX-Map"));
        if let Some(id) = crate::graph::add_node_menu::show(ui, entries) {
            add = PatternNodeKind::ALL.into_iter().find(|kind| kind.native_id() == id);
        }
    });
    if ui.button(tr("Add Dot node")).clicked() {
        add = Some(PatternNodeKind::Dot);
        ui.close();
    }
    if let Some(kind) = add {
        editor.edit(app, PatternEdit::Add { kind, at: editor.menu_at.into(), snap: app.ui.prefs.snap_to_grid }, false);
    }
    if let Some(id) = editor.selection.last().copied()
        && ui.button(tr("Set as root")).clicked()
    {
        editor.edit(app, PatternEdit::Root { id }, false);
        ui.close();
    }
    ui.separator();
    if edit_menu::button(ui, "Save package", "file.save", menus::enabled(app, "file.save")) {
        menus::invoke(app, ui.ctx(), "file.save");
    }
    if let Some(command) = edit_menu::selection(ui, |command| super::actions::editor_enabled(app, editor, command).unwrap_or(false)) {
        super::actions::invoke_editor(app, editor, command);
    }
    ui.separator();
    if ui.button(tr("Frame all")).clicked() {
        editor.framed = false;
        ui.close();
    }
}
