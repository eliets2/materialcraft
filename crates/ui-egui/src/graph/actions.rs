use crate::{MaterialCraftApp, docking::Panel};

pub(crate) fn enabled(app: &MaterialCraftApp, command: &str) -> Option<bool> {
    match command {
        "ui.graph.viewOutputs" | "ui.graph.resetViewOutputs" => Some(app.session.graph().is_ok_and(|graph| graph.function.is_none())),
        "ui.graph.previousPin" | "ui.graph.nextPin" => Some(app.session.graph().is_ok_and(|graph| !graph.pins.is_empty())),
        command if command.starts_with("ui.graph.") => Some(app.session.graph().is_ok()),
        _ => None,
    }
}

pub(crate) fn invoke(app: &mut MaterialCraftApp, command: &str) -> bool {
    match command {
        "graph.node.edit" => {
            if !crate::menus::enabled(app, command) {
                return true;
            }

            let Some(result) = app.run(command, serde_json::Value::Null) else { return true };
            let Some(node) = result.get("node").and_then(serde_json::Value::as_u64) else { return true };

            let node = materialcraft_engine::core::NodeId(node);
            match result.get("kind").and_then(serde_json::Value::as_str) {
                Some("pattern") => {
                    app.function_editor = None;
                    app.pattern_editor = Some(crate::pattern::PatternEditor::new(app.session.graph, node));
                }
                Some("function") => {
                    super::subgraph_navigation::open_function(app, node, crate::function::FunctionTarget::Parameter("function".into()));
                }
                Some("graph") => {
                    if let Some(index) = result.get("graph").and_then(serde_json::Value::as_u64).and_then(|index| usize::try_from(index).ok()) {
                        super::subgraph_navigation::open_instance(app, index);
                    }
                }
                _ => {}
            }
            app.ui.dock.focus(Panel::Graph);
        }
        "ui.graph.snapToGrid" => app.ui.prefs.snap_to_grid = !app.ui.prefs.snap_to_grid,
        "ui.graph.viewOutputs" | "ui.graph.resetViewOutputs" => {
            app.run("view.3d.outputs", serde_json::json!({"reset": command == "ui.graph.resetViewOutputs"}));
        }
        "ui.graph.computeThumbnails" => {
            app.graph.thumbnails_cleared = false;
            app.ui.prefs.thumbnails = true;
            app.session.dirty = true;
        }
        "ui.graph.clearThumbnails" => {
            app.thumbs.clear_nodes();
            app.graph.thumbnails_cleared = true;
        }
        "ui.graph.previousPin" => navigate_pin(app, false),
        "ui.graph.nextPin" => navigate_pin(app, true),
        _ => return false,
    }

    true
}

fn navigate_pin(app: &mut MaterialCraftApp, forward: bool) {
    let Ok(graph) = app.session.graph() else { return };
    if graph.pins.is_empty() {
        return;
    }

    let camera = &mut app.ui.graph_camera;
    let zoom = camera.zoom.clamp(0.08, 4.0);
    let center = [camera.pan[0] + app.graph.view.width() * 0.5 / zoom, camera.pan[1] + app.graph.view.height() * 0.5 / zoom];
    let current = graph.pins.iter().position(|pin| (pin.pos[0] - center[0]).abs() < 0.1 && (pin.pos[1] - center[1]).abs() < 0.1);
    let count = graph.pins.len();
    let index = match current {
        Some(index) if forward => (index + 1) % count,
        Some(index) => (index + count - 1) % count,
        None if forward => 0,
        None => count - 1,
    };
    let Some(pin) = graph.pins.get(index) else { return };

    camera.pan = [pin.pos[0] - app.graph.view.width() * 0.5 / zoom, pin.pos[1] - app.graph.view.height() * 0.5 / zoom];
    if let Some(editor) = &mut app.function_editor
        && editor.target == crate::function::FunctionTarget::Graph
    {
        editor.set_camera(*camera);
    }
}
