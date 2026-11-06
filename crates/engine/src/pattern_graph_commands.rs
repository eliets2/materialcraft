use materialcraft_core::{NodeId, PatternGraph, Value};
use serde_json::{Value as Json, json};

use crate::commands::{CommandSpec, bad, has_graph};
use crate::{Result, Session, cmd};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!("graph.pattern.set", "Edit FX-Map graph", [], None, "{id, pattern} replaces an FX-Map subgraph", has_graph, set)]
}

fn set(session: &mut Session, parameters: &Json) -> Result<Json> {
    let id = NodeId(parameters.get("id").and_then(Json::as_u64).ok_or_else(|| bad("graph.pattern.set", "id is required"))?);
    if session.graph()?.node(id)?.def != "fx_map" {
        return Err(bad("graph.pattern.set", "the node is not an FX-Map"));
    }

    let graph: PatternGraph =
        serde_json::from_value(parameters.get("pattern").cloned().unwrap_or(Json::Null)).map_err(|error| bad("graph.pattern.set", error.to_string()))?;
    crate::graph_validation::pattern(&graph).map_err(|error| bad("graph.pattern.set", error))?;

    session.edit("Edit FX-Map graph", |session| {
        session.graph_mut()?.node_mut(id)?.params.insert("pattern".into(), Value::PatternGraph(Box::new(graph)));
        Ok(json!({}))
    })
}
