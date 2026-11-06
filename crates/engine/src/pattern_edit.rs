use materialcraft_core::{PatternGraph, PatternNodeKind, Value};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum PatternEdit {
    Add {
        kind: PatternNodeKind,
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
        parent: u32,
        slot: usize,
        child: u32,
    },
    Disconnect {
        node: u32,
        slot: Option<usize>,
    },
    Root {
        id: u32,
    },
    Value {
        id: u32,
        parameter: String,
        value: Option<Value>,
    },
}

impl PatternEdit {
    pub(crate) fn apply(&self, graph: &mut PatternGraph) -> Result<Json, String> {
        match self {
            Self::Add { kind, at, snap } => {
                let id = graph.add(*kind)?;
                if let Some(node) = graph.nodes.last_mut() {
                    node.position = crate::graph_grid::snap_position(*at, *snap);
                }
                return Ok(json!({"id": id}));
            }
            Self::Move { ids, by, snap } => {
                for id in ids {
                    if !graph.nodes.iter().any(|node| node.id == *id) {
                        return Err(format!("no FX-Map node {id}"));
                    }
                }
                for node in &mut graph.nodes {
                    if ids.contains(&node.id) {
                        node.position = crate::graph_grid::translate(node.position, *by, *snap);
                    }
                }
            }
            Self::Connect { parent, slot, child } => {
                if !graph.nodes.iter().any(|node| node.id == *child) {
                    return Err("missing child node".into());
                }
                let destination =
                    graph.nodes.iter_mut().find(|node| node.id == *parent).and_then(|node| node.children.get_mut(*slot)).ok_or("invalid child slot")?;
                *destination = Some(*child);
            }
            Self::Disconnect { node, slot } => {
                if let Some(slot) = slot {
                    let destination = graph
                        .nodes
                        .iter_mut()
                        .find(|candidate| candidate.id == *node)
                        .and_then(|node| node.children.get_mut(*slot))
                        .ok_or("invalid child slot")?;
                    *destination = None;
                } else {
                    for child in graph.nodes.iter_mut().flat_map(|node| &mut node.children) {
                        if *child == Some(*node) {
                            *child = None;
                        }
                    }
                }
            }
            Self::Root { id } => {
                if !graph.nodes.iter().any(|node| node.id == *id) {
                    return Err("missing root node".into());
                }
                graph.root = Some(*id);
            }
            Self::Value { id, parameter, value } => {
                let node = graph.nodes.iter_mut().find(|node| node.id == *id).ok_or("missing FX-Map node")?;
                let definition = materialcraft_nodes::pattern::definition(node.kind).ok_or("unknown FX-Map node")?;
                if !definition.params.iter().any(|candidate| candidate.id == parameter) {
                    return Err("unknown FX-Map parameter".into());
                }
                if let Some(value) = value {
                    node.parameters.insert(parameter.clone(), value.clone());
                } else {
                    node.parameters.remove(parameter);
                }
            }
        }
        Ok(Json::Null)
    }

    pub(crate) fn merge_key(&self) -> Option<String> {
        match self {
            Self::Value { id, parameter, .. } => Some(format!("value:{id}:{parameter}")),
            _ => None,
        }
    }
}
