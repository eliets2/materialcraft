use materialcraft_core::{MaterialChannel, NodeId};
use serde_json::{Value, json};

use crate::commands::{CommandSpec, always, bad, boolp, has_graph, u64p};
use crate::{MaterialPreview, MaterialSource, Result, Session, cmd};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "view.3d.outputs",
            "View outputs in 3D view",
            ["Tools"],
            None,
            "{graph?: index or name, reset?: bool} assigns image outputs by usage; reset clears other assignments without moving the camera",
            has_graph,
            outputs
        ),
        cmd!(
            "view.3d.assign",
            "View output in 3D view",
            [],
            None,
            "{graph?: index or name, node?: selected node, port?: index or identifier, channel: baseColor|normal|roughness|metallic|height|ambientOcclusion|emissive|opacity, reset?: bool}",
            has_graph,
            assign
        ),
        cmd!("view.3d.clear", "Clear material assignments", [], None, "{} clears the material without resetting the scene or camera", always, |session, _| {
            session.material_preview = MaterialPreview::default();
            session.dirty = true;
            Ok(Value::Null)
        }),
    ]
}

fn graph_index(session: &Session, parameters: &Value, command: &str) -> Result<usize> {
    let graph = match parameters.get("graph") {
        None => session.graph,
        Some(Value::String(name)) => session.pkg.graph_by_name(name).map(|(index, _)| index).ok_or_else(|| bad(command, format!("no graph `{name}`")))?,
        Some(value) => value.as_u64().and_then(|index| usize::try_from(index).ok()).ok_or_else(|| bad(command, "graph must be an index or name"))?,
    };
    if session.pkg.graph(graph)?.function.is_some() {
        return Err(bad(command, "a Function Graph has no material image outputs"));
    }
    Ok(graph)
}

fn outputs(session: &mut Session, parameters: &Value) -> Result<Value> {
    let graph = graph_index(session, parameters, "view.3d.outputs")?;
    let material = MaterialPreview::for_graph(&session.pkg, graph);
    let sources = material.sources(&session.pkg);
    if sources.is_empty() {
        return Err(bad("view.3d.outputs", "no image outputs have supported material usages; assign an image to a channel with View in 3D view"));
    }
    if boolp(parameters, "reset") == Some(true) {
        session.material_preview = material;
    } else {
        session.material_preview.channels.extend(sources);
    }
    session.dirty = true;
    Ok(json!(session.material_preview))
}

fn assign(session: &mut Session, parameters: &Value) -> Result<Value> {
    const COMMAND: &str = "view.3d.assign";
    let graph = graph_index(session, parameters, COMMAND)?;
    let channel = parameters
        .get("channel")
        .and_then(Value::as_str)
        .and_then(MaterialChannel::from_id)
        .ok_or_else(|| bad(COMMAND, "choose a supported material channel"))?;
    let node = match parameters.get("node") {
        Some(_) => NodeId(u64p(parameters, "node").ok_or_else(|| bad(COMMAND, "node must be an ID"))?),
        None if graph == session.graph => *session.selection.last().ok_or_else(|| bad(COMMAND, "select a node or supply its ID"))?,
        None => return Err(bad(COMMAND, "node is required for another graph")),
    };
    let source_graph = session.pkg.graph(graph)?;
    let source_node = source_graph.node(node)?;
    let ports = if source_node.def == "output" {
        vec!["output".to_string()]
    } else {
        materialcraft_eval::ports(&session.pkg, source_graph, source_node).1.into_iter().map(|port| port.id).collect()
    };
    let port = match parameters.get("port") {
        None => ports.first(),
        Some(Value::String(name)) => ports.iter().find(|port| *port == name),
        Some(value) => value.as_u64().and_then(|index| usize::try_from(index).ok()).and_then(|index| ports.get(index)),
    }
    .cloned()
    .ok_or_else(|| bad(COMMAND, "no such image output port"))?;
    let source = MaterialSource { graph, node, port };
    if !source.is_image(&session.pkg) {
        return Err(bad(COMMAND, "the 3D view needs an image, not a scalar, vector or unresolved output"));
    }
    if boolp(parameters, "reset") == Some(true) {
        session.material_preview = MaterialPreview::default();
    }
    session.material_preview.channels.insert(channel, source);
    session.dirty = true;
    Ok(json!(session.material_preview))
}
