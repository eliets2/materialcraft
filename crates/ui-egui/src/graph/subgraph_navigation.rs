use crate::{
    MaterialCraftApp,
    docking::Panel,
    function::{FunctionEditor, FunctionTarget},
};
use serde_json::json;

use super::graph_parent::GraphParent;

pub(crate) fn can_return(app: &MaterialCraftApp) -> bool {
    app.function_editor.as_ref().is_some_and(|editor| editor.target != FunctionTarget::Graph) || crate::pattern::active(app) || !app.graph_parents.is_empty()
}

pub(crate) fn parent(app: &mut MaterialCraftApp) {
    if app.function_editor.as_ref().is_some_and(|editor| editor.target != FunctionTarget::Graph) {
        app.function_editor = None;
    } else if crate::pattern::active(app) {
        app.pattern_editor = None;
    } else if let Some(parent) = app.graph_parents.pop()
        && open_graph(app, parent.graph)
    {
        app.session.selection = parent.selection.into_iter().filter(|id| app.session.graph().is_ok_and(|graph| graph.node(*id).is_ok())).collect();
        app.ui.graph_camera = parent.camera;
        app.graph.framed_graph = Some(parent.graph);
        app.ui.frame_all = false;
    }
    app.ui.dock.focus(Panel::Graph);
}

pub(crate) fn open_function(app: &mut MaterialCraftApp, node: materialcraft_engine::core::NodeId, target: FunctionTarget) {
    if !matches!(target, FunctionTarget::Pattern { .. }) {
        app.pattern_editor = None;
    }
    app.graph.drag = None;
    app.function_editor = Some(FunctionEditor::new(app.session.graph, node, target));
    app.ui.dock.focus(Panel::Graph);
}

pub(crate) fn open_instance(app: &mut MaterialCraftApp, graph: usize) {
    let parent = GraphParent { graph: app.session.graph, selection: app.session.selection.clone(), camera: app.ui.graph_camera };
    if open_graph(app, graph) {
        app.graph_parents.push(parent);
        app.ui.dock.focus(Panel::Graph);
    }
}

fn open_graph(app: &mut MaterialCraftApp, index: usize) -> bool {
    let parents = std::mem::take(&mut app.graph_parents);
    let material = app.session.material_preview.clone();
    let opened = app.run("graph.open", json!({"graph": index})).is_some();
    app.graph_parents = parents;
    app.session.material_preview = material;
    opened
}

pub(crate) fn breadcrumb(app: &MaterialCraftApp) -> String {
    let mut path: Vec<String> =
        app.graph_parents.iter().filter_map(|parent| app.session.pkg.graphs.get(parent.graph).map(|graph| graph.name.clone())).collect();
    if let Ok(graph) = app.session.graph() {
        path.push(graph.name.clone());
        let owner = app.function_editor.as_ref().and_then(|editor| editor.node).or_else(|| app.pattern_editor.as_ref().map(|editor| editor.node));
        if let Some(node) = owner.and_then(|id| graph.node(id).ok()) {
            path.push(super::node_label(node));
        }
    }
    if let Some(editor) = &app.function_editor {
        match &editor.target {
            FunctionTarget::Pattern { node, parameter } => {
                let label = app
                    .pattern_editor
                    .as_ref()
                    .and_then(|editor| editor.current(app))
                    .and_then(|graph| graph.nodes.iter().find(|item| item.id == *node))
                    .map(|node| node.kind.label());
                if let Some(label) = label {
                    path.push(label.into());
                }
                path.push(parameter.clone());
            }
            FunctionTarget::Dynamic(parameter) => path.push(parameter.clone()),
            FunctionTarget::Parameter(parameter) if parameter != "function" => path.push(parameter.clone()),
            _ => {}
        }
    }
    path.join(" / ")
}
