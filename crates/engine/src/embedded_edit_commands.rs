use crate::{
    FunctionEdit, FunctionGraphTarget, PatternEdit, Result, Session, cmd,
    commands::{CommandSpec, bad, has_graph},
};

use materialcraft_core::{NodeId, Value};
use serde_json::{Value as Json, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graph.function.edit",
            "Edit function node",
            [],
            None,
            "{target?: {kind: graph|parameter|dynamic|pattern, node?, owner?, parameter?}, edit: {action: add|move|connect|value|output, ...}, merge?: false}",
            has_graph,
            function
        ),
        cmd!(
            "graph.pattern.edit",
            "Edit FX-Map node",
            [],
            None,
            "{id: owner, edit: {action: add|move|connect|disconnect|root|value, ...}, merge?: false}",
            has_graph,
            pattern
        ),
        cmd!("edit.finish", "Finish edit gesture", [], None, "{} ends the current undo group", crate::commands::always, |session, _| {
            session.finish_edit();
            Ok(Json::Null)
        }),
    ]
}

fn commit(session: &mut Session, label: &str, key: Option<String>, run: impl FnOnce(&mut Session) -> Result<Json>) -> Result<Json> {
    match key {
        Some(key) => session.edit_merged(&key, label, run),
        None => session.edit(label, run),
    }
}

fn function(session: &mut Session, parameters: &Json) -> Result<Json> {
    let target: FunctionGraphTarget = serde_json::from_value(parameters.get("target").cloned().unwrap_or_else(|| json!({"kind": "graph"})))
        .map_err(|error| bad("graph.function.edit", error.to_string()))?;
    let edit: FunctionEdit =
        serde_json::from_value(parameters.get("edit").cloned().unwrap_or(Json::Null)).map_err(|error| bad("graph.function.edit", error.to_string()))?;
    let current = target.current(session)?;
    if let FunctionEdit::Add { operation, .. } = &edit
        && !target.available_operations(session).any(|definition| definition.id == operation)
    {
        return Err(bad("graph.function.edit", "this operation is not available in the target function graph"));
    }
    let mut graph = current.clone();
    let result = edit.apply(&mut graph).map_err(|error| bad("graph.function.edit", error))?;
    crate::graph_validation::function(&graph).map_err(|error| bad("graph.function.edit", error))?;
    let key = (parameters.get("merge").and_then(Json::as_bool) == Some(true))
        .then(|| edit.merge_key())
        .flatten()
        .map(|key| format!("function:{}:{target:?}:{key}", session.graph));
    commit(session, "Edit Function Graph", key, |session| {
        target.replace(session, graph)?;
        Ok(result)
    })
}

fn pattern(session: &mut Session, parameters: &Json) -> Result<Json> {
    let id = NodeId(parameters.get("id").and_then(Json::as_u64).ok_or_else(|| bad("graph.pattern.edit", "id is required"))?);
    let node = session.graph()?.node(id)?;
    if node.def != "fx_map" {
        return Err(bad("graph.pattern.edit", "the node is not an FX-Map"));
    }
    let value = node.params.get("pattern").or_else(|| materialcraft_nodes::find(&node.def)?.param("pattern").map(|parameter| &parameter.default));
    let Some(Value::PatternGraph(graph)) = value else { return Err(bad("graph.pattern.edit", "missing FX-Map graph")) };
    let mut graph = (**graph).clone();
    let edit: PatternEdit =
        serde_json::from_value(parameters.get("edit").cloned().unwrap_or(Json::Null)).map_err(|error| bad("graph.pattern.edit", error.to_string()))?;
    let result = edit.apply(&mut graph).map_err(|error| bad("graph.pattern.edit", error))?;
    crate::graph_validation::pattern(&graph).map_err(|error| bad("graph.pattern.edit", error))?;
    let key = (parameters.get("merge").and_then(Json::as_bool) == Some(true))
        .then(|| edit.merge_key())
        .flatten()
        .map(|key| format!("pattern:{}:{}:{key}", session.graph, id.0));
    commit(session, "Edit FX-Map graph", key, |session| {
        session.graph_mut()?.node_mut(id)?.params.insert("pattern".into(), Value::PatternGraph(Box::new(graph)));
        Ok(result)
    })
}
