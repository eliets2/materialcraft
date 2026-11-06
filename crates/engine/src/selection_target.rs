use crate::{FunctionGraphTarget, Result, Session, commands::bad};
use materialcraft_core::{NodeId, PatternGraph, Value};
use serde_json::Value as Json;

#[derive(Clone, Debug)]
pub enum SelectionTarget {
    Function(FunctionGraphTarget),
    Pattern(NodeId),
}

impl SelectionTarget {
    pub(crate) fn resolve(session: &Session, parameters: &Json) -> Result<Option<Self>> {
        if parameters.get("function").is_some() && parameters.get("pattern").is_some() {
            return Err(bad("graph selection", "specify function or pattern, not both"));
        }
        if let Some(graph) = parameters.get("graph")
            && graph.as_u64() != Some(session.graph as u64)
        {
            return Err(bad("graph selection", "the owning graph is no longer active"));
        }
        if let Some(target) = parameters.get("function") {
            let target = serde_json::from_value(target.clone()).map_err(|error| bad("graph selection", error.to_string()))?;
            return Ok(Some(Self::Function(target)));
        }
        if let Some(owner) = parameters.get("pattern") {
            let owner = owner.as_u64().ok_or_else(|| bad("graph selection", "pattern must be an owner node id"))?;
            return Ok(Some(Self::Pattern(NodeId(owner))));
        }
        Ok(session.graph()?.function.as_ref().map(|_| Self::Function(FunctionGraphTarget::Graph)))
    }

    pub(crate) fn standalone(&self) -> bool {
        matches!(self, Self::Function(FunctionGraphTarget::Graph))
    }

    pub(crate) fn source(&self, session: &Session) -> Result<String> {
        Ok(format!("{}:{self:?}", session.graph()?.name))
    }

    pub(crate) fn pattern(session: &Session, owner: NodeId) -> Result<&PatternGraph> {
        let node = session.graph()?.node(owner)?;
        if node.def != "fx_map" {
            return Err(bad("graph selection", "the node is not an FX-Map"));
        }
        let value = node.params.get("pattern").or_else(|| materialcraft_nodes::find(&node.def)?.param("pattern").map(|parameter| &parameter.default));
        match value {
            Some(Value::PatternGraph(graph)) => Ok(graph),
            _ => Err(bad("graph selection", "missing FX-Map graph")),
        }
    }
}
