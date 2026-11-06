use materialcraft_core::{FunctionGraph, FunctionNode, Value};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum FunctionEdit {
    Add {
        operation: String,
        at: [f32; 2],
        #[serde(default)]
        snap: bool,
    },
    Move {
        ids: Vec<u32>,
        by: [f32; 2],
        #[serde(default)]
        snap: bool,
    },
    Connect {
        from: u32,
        to: u32,
        input: usize,
    },
    Value {
        id: u32,
        value: Value,
    },
    Output {
        id: u32,
    },
}

impl FunctionEdit {
    pub(crate) fn apply(&self, graph: &mut FunctionGraph) -> Result<Json, String> {
        match self {
            Self::Add { operation, at, snap } => {
                if graph.nodes.len() >= 4096 {
                    return Err("the function exceeds the 4096-node limit".into());
                }
                let definition = materialcraft_nodes::function::definition(operation).ok_or("unknown function operation")?;
                let id = graph.next_id();
                graph.nodes.push(FunctionNode {
                    id,
                    type_code: 0,
                    op: operation.clone(),
                    inputs: vec![None; definition.inputs.len()],
                    value: materialcraft_nodes::function::default_value(operation),
                    pos: crate::graph_grid::snap_position(*at, *snap),
                });
                if graph.output.is_none() {
                    graph.output = Some(id);
                }
                return Ok(json!({"id": id}));
            }
            Self::Move { ids, by, snap } => {
                for id in ids {
                    if graph.node(*id).is_none() {
                        return Err(format!("no function node {id}"));
                    }
                }
                for node in &mut graph.nodes {
                    if ids.contains(&node.id) {
                        node.pos = crate::graph_grid::translate(node.pos, *by, *snap);
                    }
                }
            }
            Self::Connect { from, to, input } => {
                if from == to || graph.node(*from).is_none() {
                    return Err("invalid function source".into());
                }
                let node = graph.nodes.iter_mut().find(|node| node.id == *to).ok_or("missing function destination")?;
                let count = materialcraft_nodes::function::definition(&node.op).map_or(0, |definition| definition.inputs.len());
                if *input >= count {
                    return Err("invalid function input".into());
                }
                node.inputs.resize(count, None);
                if let Some(slot) = node.inputs.get_mut(*input) {
                    *slot = Some(*from);
                }
            }
            Self::Value { id, value } => {
                let node = graph.nodes.iter_mut().find(|node| node.id == *id).ok_or("missing function node")?;
                if node.value.is_none() {
                    return Err("the operation has no editable value".into());
                }
                node.value = Some(value.clone());
            }
            Self::Output { id } => {
                if graph.node(*id).is_none() {
                    return Err("missing output node".into());
                }
                graph.output = Some(*id);
            }
        }
        Ok(Json::Null)
    }

    pub(crate) fn merge_key(&self) -> Option<String> {
        match self {
            Self::Value { id, .. } => Some(format!("value:{id}")),
            _ => None,
        }
    }
}
