use crate::{i18n::tr, menus};
use egui::Ui;

pub(crate) fn button(ui: &mut Ui, label: &str, command: &str, enabled: bool) -> bool {
    let mut button = egui::Button::new(tr(label));
    if let Some(shortcut) = menus::shortcut_of(command) {
        button = button.shortcut_text(shortcut);
    }
    let clicked = ui.add_enabled(enabled, button).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

pub(crate) fn selection(ui: &mut Ui, enabled: impl Fn(&str) -> bool) -> Option<&'static str> {
    let mut chosen = None;
    for (label, command) in [
        ("Paste", "edit.paste"),
        ("Paste without link", "edit.pasteWithoutLinks"),
        ("Select all", "edit.selectAll"),
        ("Copy selection", "edit.copy"),
        ("Cut selection", "edit.cut"),
        ("Delete selection", "edit.delete"),
        ("Delete and relink", "edit.deleteRelink"),
        ("Duplicate selection", "edit.duplicate"),
        ("Duplicate selection without link", "edit.duplicateWithoutLinks"),
        ("Select upstream nodes", "graph.selectUpstream"),
        ("Select downstream nodes", "graph.selectDownstream"),
        ("Disable selection", "graph.node.disable"),
    ] {
        if command == "edit.copy" {
            ui.separator();
        }
        if button(ui, label, command, enabled(command)) {
            chosen = Some(command);
        }
    }
    chosen
}
