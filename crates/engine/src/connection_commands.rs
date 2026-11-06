use materialcraft_core::NodeId;
use serde_json::Value as J;

use crate::commands::{CommandSpec, bad, has_graph, strp, u64p};
use crate::{Result, Session, cmd};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graph.connect",
            "Connect",
            [],
            None,
            "{from, fromPort? (output), to, toPort? (first compatible input), detach? {to, toPort}}",
            has_graph,
            connect
        ),
        cmd!("graph.disconnect", "Disconnect", [], None, "{to, toPort}", has_graph, disconnect),
    ]
}

fn connect(session: &mut Session, parameters: &J) -> Result<J> {
    let from = NodeId(u64p(parameters, "from").ok_or_else(|| bad("graph.connect", "`from` is required"))?);
    let to = NodeId(u64p(parameters, "to").ok_or_else(|| bad("graph.connect", "`to` is required"))?);
    let outputs = session.output_ports(from);
    let from_port = strp(parameters, "fromPort")
        .map(str::to_string)
        .or_else(|| outputs.first().cloned())
        .ok_or_else(|| bad("graph.connect", "the source has no outputs"))?;
    let to_port = match strp(parameters, "toPort") {
        Some(port) => port.to_string(),
        None => connection_input(session, from, &from_port, to)?,
    };

    let detached = parameters
        .get("detach")
        .map(|input| {
            let node = u64p(input, "to").map(NodeId).ok_or_else(|| bad("graph.connect", "detach needs `to`"))?;
            let port = strp(input, "toPort").ok_or_else(|| bad("graph.connect", "detach needs `toPort`"))?;
            let connection = session.graph()?.input_of(node, port).ok_or_else(|| bad("graph.connect", "the detached wire no longer exists"))?;
            if connection.from != from || connection.from_port != from_port {
                return Err(bad("graph.connect", "the detached wire has a different source"));
            }

            Ok((node, port.to_string()))
        })
        .transpose()?;

    validate_connection(session, from, &from_port, to, &to_port)?;

    session.edit("Connect", |session| {
        if let Some((node, port)) = detached {
            session.graph_mut()?.disconnect(node, &port);
        }

        session.graph_mut()?.connect(from, &from_port, to, &to_port)?;
        Ok(J::Null)
    })
}

pub(crate) fn connection_input(session: &Session, from: NodeId, from_port: &str, to: NodeId) -> Result<String> {
    session
        .input_ports(to)
        .into_iter()
        .find(|port| validate_connection(session, from, from_port, to, port).is_ok())
        .ok_or_else(|| bad("graph.connect", "the target has no compatible input"))
}

fn validate_connection(session: &Session, from: NodeId, from_port: &str, to: NodeId, to_port: &str) -> Result<()> {
    let outputs = session.output_ports(from);
    let inputs = session.input_ports(to);
    if !outputs.iter().any(|port| port == from_port) {
        return Err(bad("graph.connect", format!("no output `{from_port}` on node {}", from.0)));
    }
    if !inputs.iter().any(|port| port == to_port) {
        return Err(bad("graph.connect", format!("no input `{to_port}` on node {}", to.0)));
    }

    let graph = session.graph()?;
    let target = graph.node(to)?;
    let source_type = materialcraft_nodes::output_value_type(graph, from).map_err(|error| bad("graph.connect", error))?;
    if let Some(expected) = target.input_values.get(to_port) {
        if source_type != Some(*expected) {
            return Err(bad("graph.connect", format!("input `{to_port}` requires {}", expected.label())));
        }
    } else if source_type.is_some() && !matches!(target.def.as_str(), "output" | "dot") {
        return Err(bad("graph.connect", format!("input `{to_port}` requires an image")));
    }

    Ok(())
}

fn disconnect(s: &mut Session, p: &J) -> Result<J> {
    let to = NodeId(u64p(p, "to").ok_or_else(|| bad("graph.disconnect", "`to` is required"))?);
    let port = strp(p, "toPort").ok_or_else(|| bad("graph.disconnect", "`toPort` is required"))?.to_string();
    s.edit("Disconnect", |s| {
        if !s.graph_mut()?.disconnect(to, &port) {
            return Err(bad("graph.disconnect", "nothing is connected there"));
        }
        Ok(J::Null)
    })
}

impl Session {
    /// Input port ids of node `id` (empty for a missing node)
    pub fn input_ports(&self, id: NodeId) -> Vec<String> {
        self.graph()
            .ok()
            .and_then(|graph| graph.node(id).ok().map(|node| materialcraft_eval::ports(&self.pkg, graph, node).0.into_iter().map(|port| port.id).collect()))
            .unwrap_or_default()
    }

    /// Output port ids of node `id`
    pub fn output_ports(&self, id: NodeId) -> Vec<String> {
        self.graph()
            .ok()
            .and_then(|graph| graph.node(id).ok().map(|node| materialcraft_eval::ports(&self.pkg, graph, node).1.into_iter().map(|port| port.id).collect()))
            .unwrap_or_default()
    }
}
