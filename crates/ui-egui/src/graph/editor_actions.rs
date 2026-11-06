use super::editor_target::EditorTarget;
use crate::MaterialCraftApp;

pub(crate) fn enabled(app: &MaterialCraftApp, command: &str) -> Option<bool> {
    if command == "ui.graph.parent" {
        return Some(super::subgraph_navigation::can_return(app));
    }
    match EditorTarget::resolve(app) {
        EditorTarget::Function => crate::function::enabled(app, command),
        EditorTarget::Pattern => crate::pattern::enabled(app, command),
        EditorTarget::Material => None,
    }
}

pub(crate) fn invoke(app: &mut MaterialCraftApp, command: &str) -> bool {
    if command == "ui.graph.parent" {
        super::subgraph_navigation::parent(app);
        return true;
    }
    match EditorTarget::resolve(app) {
        EditorTarget::Function => crate::function::invoke(app, command),
        EditorTarget::Pattern => crate::pattern::invoke(app, command),
        EditorTarget::Material => false,
    }
}
