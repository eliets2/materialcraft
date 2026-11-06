use crate::MaterialCraftApp;
use egui::{Pos2, Vec2, vec2};
use materialcraft_engine::core::FunctionGraph;
use materialcraft_engine::core::NodeId;
use serde_json::json;

use super::{FunctionDrag, FunctionTarget};

pub struct FunctionEditor {
    pub node: Option<NodeId>,
    pub graph: usize,
    pub(crate) selection: Vec<u32>,
    pub(crate) inspected_node: Option<u32>,
    pub target: FunctionTarget,
    pub(super) pan: Vec2,
    pub(super) zoom: f32,
    pub(super) drag: Option<FunctionDrag>,
    pub(super) menu_at: Pos2,
    pub(crate) framed: bool,
}

impl FunctionEditor {
    pub fn standalone(graph: usize) -> Self {
        Self::with_target(graph, None, FunctionTarget::Graph)
    }

    pub(crate) fn set_camera(&mut self, camera: crate::state::GraphCamera) {
        self.pan = camera.pan.into();
        self.zoom = camera.zoom;
    }

    pub fn new(graph: usize, node: NodeId, target: FunctionTarget) -> Self {
        Self::with_target(graph, Some(node), target)
    }

    fn with_target(graph: usize, node: Option<NodeId>, target: FunctionTarget) -> Self {
        Self {
            graph,
            node,
            target,
            selection: Vec::new(),
            inspected_node: None,
            pan: vec2(-40.0, -40.0),
            zoom: 1.0,
            drag: None,
            menu_at: Pos2::ZERO,
            framed: false,
        }
    }

    pub(crate) fn current<'a>(&self, app: &'a MaterialCraftApp) -> Option<&'a FunctionGraph> {
        if self.graph != app.session.graph {
            return None;
        }

        self.command_target()?.current(&app.session).ok()
    }

    pub(crate) fn command_target(&self) -> Option<materialcraft_engine::FunctionGraphTarget> {
        use materialcraft_engine::FunctionGraphTarget;
        Some(match &self.target {
            FunctionTarget::Graph => FunctionGraphTarget::Graph,
            FunctionTarget::Parameter(parameter) => FunctionGraphTarget::Parameter { node: self.node?, parameter: parameter.clone() },
            FunctionTarget::Dynamic(parameter) => FunctionGraphTarget::Dynamic { node: self.node?, parameter: parameter.clone() },
            FunctionTarget::Pattern { node, parameter } => FunctionGraphTarget::Pattern { owner: self.node?, node: *node, parameter: parameter.clone() },
        })
    }

    pub(crate) fn edit(&mut self, app: &mut MaterialCraftApp, edit: materialcraft_engine::FunctionEdit, merge: bool) {
        if self.current(app).is_none() {
            return;
        }
        let Some(target) = self.command_target() else { return };

        if let Some(result) = app.run("graph.function.edit", json!({"target": target, "edit": edit, "merge": merge}))
            && let Some(id) = result.get("id").and_then(serde_json::Value::as_u64).and_then(|id| u32::try_from(id).ok())
        {
            self.selection = vec![id];
            if self.target == FunctionTarget::Graph {
                app.session.selection = vec![NodeId(u64::from(id))];
                app.session.annotation_selection = None;
            }
        }
    }

    pub(crate) fn selection_parameters(&self, command: &str) -> serde_json::Value {
        let selection = if matches!(command, "edit.selectAll" | "edit.selectNone") { &[][..] } else { self.selection.as_slice() };
        json!({"graph": self.graph, "function": self.command_target(), "ids": selection, "at": [self.menu_at.x, self.menu_at.y]})
    }
}
