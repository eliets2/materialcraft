use crate::commands::{CommandSpec, bad, has_graph};
use crate::{FunctionGraphTarget, Result, Session, cmd};
use materialcraft_core::FunctionGraph;
use serde_json::{Value as Json, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.function.set",
        "Edit Function Graph",
        [],
        None,
        "{function, target?: {kind: graph|parameter|dynamic|pattern, node?, owner?, parameter?}} replaces a function graph",
        has_graph,
        set
    )]
}

fn set(session: &mut Session, parameters: &Json) -> Result<Json> {
    let target: FunctionGraphTarget = serde_json::from_value(parameters.get("target").cloned().unwrap_or_else(|| json!({"kind": "graph"})))
        .map_err(|error| bad("graph.function.set", error.to_string()))?;
    target.current(session)?;

    let graph: FunctionGraph =
        serde_json::from_value(parameters.get("function").cloned().unwrap_or(Json::Null)).map_err(|error| bad("graph.function.set", error.to_string()))?;
    crate::graph_validation::function(&graph).map_err(|error| bad("graph.function.set", error))?;

    session.edit("Edit Function Graph", |session| {
        target.replace(session, graph)?;
        Ok(json!({}))
    })
}
