use materialcraft_nodes::PortKind;

pub(super) fn output_type(graph: &materialcraft_core::Graph, id: materialcraft_core::NodeId, port: &str) -> u32 {
    if let Ok(Some(kind)) = materialcraft_nodes::output_value_type(graph, id) {
        return kind as u32;
    }

    let mut pending = vec![(id, port.to_string())];
    let mut visited = std::collections::BTreeSet::new();
    while let Some((id, port)) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }

        let Ok(node) = graph.node(id) else { continue };
        let Some(definition) = materialcraft_nodes::find(&node.def) else { return 1 };
        if let Some(parameter) = definition.param("color_mode") {
            if node.params.get("color_mode").unwrap_or(&parameter.default).as_bool() == Some(true) {
                return 1;
            }
            continue;
        }

        match definition.outputs.iter().find(|output| output.id == port).map(|output| output.kind) {
            Some(PortKind::Color) => return 1,
            Some(PortKind::Gray) => continue,
            _ => {}
        }

        let sources = definition.inputs.iter().filter(|input| input.kind != PortKind::Gray).filter_map(|input| graph.input_of(id, input.id));
        for source in sources {
            pending.push((source.from, source.from_port.clone()));
            if node.def != "blend" {
                break;
            }
        }
    }
    2
}
