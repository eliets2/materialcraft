use crate::{Result, Session, commands::bad};
use materialcraft_core::{FunctionGraph, NodeId, Value};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FunctionGraphTarget {
    #[default]
    Graph,
    Parameter {
        node: NodeId,
        parameter: String,
    },
    Dynamic {
        node: NodeId,
        parameter: String,
    },
    Pattern {
        owner: NodeId,
        node: u32,
        parameter: String,
    },
}

impl FunctionGraphTarget {
    pub fn available_operations(&self, session: &Session) -> impl Iterator<Item = &'static materialcraft_nodes::function::FunctionDefinition> + use<> {
        let value_processor = matches!(self, Self::Parameter { node, parameter }
            if parameter == "function" && session.graph().ok().and_then(|graph| graph.node(*node).ok()).is_some_and(|node| node.def == "value_processor"));
        materialcraft_nodes::function::FUNCTIONS.iter().filter(move |operation| !value_processor || !matches!(operation.id, "samplelum" | "samplecol"))
    }

    pub fn current<'a>(&self, session: &'a Session) -> Result<&'a FunctionGraph> {
        let document = session.graph()?;
        let value = match self {
            Self::Graph => return document.function.as_ref().ok_or_else(|| bad("graph.function.edit", "no Function Graph is open")),
            Self::Dynamic { node, parameter } => {
                return document.node(*node)?.functions.get(parameter).ok_or_else(|| bad("graph.function.edit", "missing parameter function"));
            }
            Self::Parameter { node, parameter } => {
                let node = document.node(*node)?;
                node.params.get(parameter).or_else(|| materialcraft_nodes::find(&node.def)?.param(parameter).map(|parameter| &parameter.default))
            }
            Self::Pattern { owner, node, parameter } => {
                let Some(Value::PatternGraph(graph)) = document.node(*owner)?.params.get("pattern") else {
                    return Err(bad("graph.function.edit", "missing FX-Map graph"));
                };

                graph.nodes.iter().find(|candidate| candidate.id == *node).and_then(|node| node.parameters.get(parameter))
            }
        };

        match value {
            Some(Value::Function(graph)) => Ok(graph),
            _ => Err(bad("graph.function.edit", "missing function")),
        }
    }

    pub(crate) fn replace(&self, session: &mut Session, function: FunctionGraph) -> Result<()> {
        self.current(session)?;
        let document = session.graph_mut()?;
        match self {
            Self::Graph => document.function = Some(function),
            Self::Dynamic { node, parameter } => {
                document.node_mut(*node)?.functions.insert(parameter.clone(), function);
            }
            Self::Parameter { node, parameter } => {
                document.node_mut(*node)?.params.insert(parameter.clone(), Value::Function(Box::new(function)));
            }
            Self::Pattern { owner, node, parameter } => {
                let Some(Value::PatternGraph(graph)) = document.node_mut(*owner)?.params.get_mut("pattern") else {
                    return Err(bad("graph.function.edit", "missing FX-Map graph"));
                };

                let node = graph.nodes.iter_mut().find(|candidate| candidate.id == *node).ok_or_else(|| bad("graph.function.edit", "missing FX-Map node"))?;
                node.parameters.insert(parameter.clone(), Value::Function(Box::new(function)));
            }
        }
        Ok(())
    }
}
