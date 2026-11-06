use crate::{EngineError, Result};
use materialcraft_core::{Graph, Value};

pub const TEMPLATES: &[(&str, &str)] = &[("empty", "Empty"), ("pbr", "Material outputs"), ("atomics", "Atomic sample")];

pub fn template(id: &str, name: &str, size_log2: u32) -> Result<Graph> {
    let mut graph = match id {
        "empty" | "pbr" => Graph::new(name),
        "atomics" => crate::atomic_sample()?.graphs.into_iter().next().ok_or_else(|| EngineError::Other("atomic sample has no graph".into()))?,
        _ => return Err(EngineError::Other(format!("unknown template `{id}`"))),
    };

    graph.name = name.to_string();
    graph.size = [size_log2, size_log2];
    if id == "pbr" {
        for (index, (identifier, color)) in [
            ("basecolor", [0.5, 0.5, 0.5, 1.0]),
            ("normal", [0.5, 0.5, 1.0, 1.0]),
            ("roughness", [0.5, 0.5, 0.5, 1.0]),
            ("metallic", [0.0, 0.0, 0.0, 1.0]),
            ("height", [0.0, 0.0, 0.0, 1.0]),
            ("ambientocclusion", [1.0; 4]),
        ]
        .into_iter()
        .enumerate()
        {
            let source = graph.add_node("uniform", [0.0, index as f32 * 180.0]);
            let node = graph.node_mut(source)?;
            node.params.insert("color_mode".into(), Value::Bool(index <= 1));
            node.params.insert("color".into(), Value::Color(color));
            let output = graph.add_node("output", [220.0, index as f32 * 180.0]);
            let node = graph.node_mut(output)?;
            node.params.insert("identifier".into(), Value::Text(identifier.into()));
            node.params.insert("usage".into(), Value::Enum(index as u32));
            graph.connect(source, "output", output, "input")?;
        }
    }
    Ok(graph)
}
