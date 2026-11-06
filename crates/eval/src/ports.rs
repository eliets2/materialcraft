use crate::Port;
use materialcraft_core::{Graph, Node, Package, Value};
use materialcraft_nodes::{PortKind, find};

/// The input and output ports of a node in its package
pub fn ports(package: &Package, graph: &Graph, node: &Node) -> (Vec<Port>, Vec<Port>) {
    ports_with_connections(package, node, graph.connections.iter().filter(|connection| connection.to == node.id))
}

/// Resolve ports using the node's incoming connections in document order
pub fn ports_with_connections<'a>(
    package: &Package,
    node: &Node,
    incoming: impl IntoIterator<Item = &'a materialcraft_core::Connection>,
) -> (Vec<Port>, Vec<Port>) {
    if node.def == "graph_instance" {
        let Some((_, graph)) = instance_graph(package, node) else {
            return (Vec::new(), Vec::new());
        };

        let graph_ports = |definition: &str| -> Vec<Port> {
            graph
                .nodes
                .iter()
                .filter(|node| node.def == definition || (definition == "input" && node.def.starts_with("input_")))
                .map(|node| {
                    let id = identifier(node);
                    let kind = match node.def.as_str() {
                        "input_grayscale" => PortKind::Gray,
                        "input_color" => PortKind::Color,
                        _ => PortKind::Any,
                    };

                    Port { label: id.clone(), id, kind }
                })
                .collect()
        };

        return (graph_ports("input"), graph_ports("output"));
    }

    if node.def == materialcraft_core::MISSING_NODE {
        let (inputs, outputs) = materialcraft_core::missing_ports(node);
        let inputs = inputs.into_iter().map(|id| Port { label: id.clone(), id, kind: PortKind::Any }).collect();
        let outputs =
            outputs.into_iter().map(|(id, grayscale)| Port { label: id.clone(), id, kind: if grayscale { PortKind::Gray } else { PortKind::Color } }).collect();

        return (inputs, outputs);
    }

    let Some(definition) = find(&node.def) else {
        return (Vec::new(), Vec::new());
    };

    let catalog_port = |port: &materialcraft_nodes::PortDefinition| Port { id: port.id.into(), label: port.label.into(), kind: port.kind };
    let mut inputs: Vec<Port> = node
        .input_values
        .iter()
        .map(|(identifier, value_type)| Port {
            id: identifier.clone(),
            label: format!("{} ({})", crate::value_input_label(identifier), value_type.label()),
            kind: PortKind::Value,
        })
        .collect();
    inputs.extend(definition.inputs.iter().map(catalog_port));
    if let Some(prefix) = materialcraft_nodes::image_input_prefix(&node.def) {
        let count = incoming
            .into_iter()
            .filter_map(|connection| connection.to_port.strip_prefix(prefix)?.parse::<usize>().ok())
            .filter(|index| *index < materialcraft_nodes::MAX_IMAGE_INPUTS)
            .map(|index| index + 2)
            .max()
            .unwrap_or(1)
            .min(materialcraft_nodes::MAX_IMAGE_INPUTS);
        inputs.extend((1..count).map(|index| Port { id: format!("{prefix}{index}"), label: format!("Input image {index}"), kind: PortKind::Any }));
    }

    (inputs, definition.outputs.iter().map(catalog_port).collect())
}

/// The graph a graph-instance node refers to
pub fn instance_graph<'a>(pkg: &'a Package, node: &Node) -> Option<(usize, &'a Graph)> {
    match node.params.get("graph") {
        Some(Value::Text(name)) => pkg.graph_by_name(name),
        _ => None,
    }
}

/// An input or output node's identifier (its `identifier` parameter, default "input"/"output")
pub fn identifier(n: &Node) -> String {
    match n.params.get("identifier") {
        Some(Value::Text(s)) if !s.is_empty() => s.clone(),
        _ => if n.def == "output" { "output" } else { "input" }.into(),
    }
}
