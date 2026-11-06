use super::WireRef;
use crate::{MaterialCraftApp, i18n::tr};
use egui::{Pos2, Ui};
use egui_phosphor::regular as icon;
use serde_json::json;

/// `wires` as the `{wires: [{to, toPort}, ...]}` parameter of the wire commands
fn wires_json(wires: &[WireRef]) -> serde_json::Value {
    json!({"wires": wires.iter().map(|(to, port)| json!({"to": to.0, "toPort": port})).collect::<Vec<_>>()})
}

/// Delete the selected wires in one undo step (the Delete and Backspace keys); `false` when none is selected
pub fn delete_selected_wires(app: &mut MaterialCraftApp) -> bool {
    let wires = std::mem::take(&mut app.graph.selected_wires);
    if wires.is_empty() {
        return false;
    }
    app.run("graph.wire.delete", wires_json(&wires));
    true
}

/// Reverse the sources of selected wires so pressing X again restores them
pub fn swap_selected_wires(app: &mut MaterialCraftApp) -> bool {
    if app.graph.selected_wires.is_empty() {
        return false;
    }
    let params = wires_json(&app.graph.selected_wires);
    app.run("graph.wire.swap", params);
    true
}

pub(super) fn wire_menu(app: &mut MaterialCraftApp, ui: &mut Ui, (to, port): WireRef, at: Pos2) {
    let n = app.graph.selected_wires.len();
    if ui.button(format!("{}  {}", icon::DOT_OUTLINE, tr("Insert reroute dot"))).clicked() {
        app.run("graph.wire.insertDot", json!({"to": to.0, "toPort": port, "at": [at.x, at.y]}));
        app.graph.selected_wires.clear();
        ui.close();
    }
    if n >= 2 && ui.button(format!("{}  {}  (X)", icon::ARROWS_LEFT_RIGHT, tr("Swap selected links"))).clicked() {
        swap_selected_wires(app);
        ui.close();
    }
    let label = if n >= 2 { tr("Delete selected links") } else { tr("Delete link") };

    if ui.button(format!("{}  {label}", icon::TRASH)).clicked() {
        if n < 2 {
            app.graph.selected_wires = vec![(to, port)];
        }
        delete_selected_wires(app);
        ui.close();
    }
}
