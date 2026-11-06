use egui::{Pos2, Vec2};
use materialcraft_engine::core::{NodeId, PatternGraph, Value};
use serde_json::json;

use super::PatternDrag;
use crate::MaterialCraftApp;

pub struct PatternEditor {
    pub node: NodeId,
    pub graph: usize,
    pub(crate) selection: Vec<u32>,
    pub(crate) inspected_node: Option<u32>,
    pub(super) pan: Vec2,
    pub(super) zoom: f32,
    pub(super) framed: bool,
    pub(super) drag: Option<PatternDrag>,
    pub(super) menu_at: Pos2,
}

impl PatternEditor {
    pub fn new(graph: usize, node: NodeId) -> Self {
        Self { node, graph, selection: Vec::new(), inspected_node: None, pan: Vec2::ZERO, zoom: 1.0, framed: false, drag: None, menu_at: Pos2::ZERO }
    }

    pub(crate) fn current<'a>(&self, app: &'a MaterialCraftApp) -> Option<&'a PatternGraph> {
        if app.session.graph != self.graph {
            return None;
        }

        let node = app.session.graph().ok()?.node(self.node).ok()?;
        if node.def != "fx_map" {
            return None;
        }

        let default = materialcraft_engine::nodes::find(&node.def)?.param("pattern");
        match node.params.get("pattern").or_else(|| default.map(|parameter| &parameter.default)) {
            Some(Value::PatternGraph(graph)) => Some(graph.as_ref()),
            None => {
                static EMPTY: std::sync::OnceLock<PatternGraph> = std::sync::OnceLock::new();
                Some(EMPTY.get_or_init(PatternGraph::default))
            }
            _ => None,
        }
    }

    pub(crate) fn selection_parameters(&self, command: &str) -> serde_json::Value {
        let selection = if matches!(command, "edit.selectAll" | "edit.selectNone") { &[][..] } else { self.selection.as_slice() };
        json!({"graph": self.graph, "pattern": self.node.0, "ids": selection, "at": [self.menu_at.x, self.menu_at.y]})
    }

    pub(super) fn edit(&mut self, app: &mut MaterialCraftApp, edit: materialcraft_engine::PatternEdit, merge: bool) {
        if self.current(app).is_none() {
            return;
        }

        if let Some(result) = app.run("graph.pattern.edit", json!({"id": self.node.0, "edit": edit, "merge": merge}))
            && let Some(id) = result.get("id").and_then(serde_json::Value::as_u64).and_then(|id| u32::try_from(id).ok())
        {
            self.selection = vec![id];
        }
    }
}
