use materialcraft_core::{FunctionGraph, PatternGraph, Value};

fn function_structure(graph: &FunctionGraph) -> Result<(), String> {
    materialcraft_nodes::function::node_types(graph)?;
    if graph.nodes.iter().any(|node| node.pos.iter().any(|coordinate| !coordinate.is_finite())) {
        return Err("function node positions must be finite".into());
    }
    Ok(())
}

pub(crate) fn pattern(graph: &PatternGraph) -> Result<(), String> {
    pattern_structure(graph)?;
    values(graph.nodes.iter().flat_map(|node| node.parameters.values()))
}

pub(crate) fn function(graph: &FunctionGraph) -> Result<(), String> {
    function_structure(graph)?;
    values(graph.nodes.iter().filter_map(|node| node.value.as_ref()))
}

pub(crate) fn value(value: &Value) -> Result<(), String> {
    values(std::iter::once(value))
}

fn values<'a>(values: impl Iterator<Item = &'a Value>) -> Result<(), String> {
    // Nested graph values are checked without recursive traversal
    let mut pending: Vec<_> = values.collect();
    while let Some(value) = pending.pop() {
        match value {
            Value::Function(graph) => {
                function_structure(graph)?;
                pending.extend(graph.nodes.iter().filter_map(|node| node.value.as_ref()));
            }
            Value::PatternGraph(graph) => {
                pattern_structure(graph)?;
                pending.extend(graph.nodes.iter().flat_map(|node| node.parameters.values()));
            }
            _ => {}
        }
    }
    Ok(())
}

fn pattern_structure(graph: &PatternGraph) -> Result<(), String> {
    graph.validate()?;
    if graph.nodes.iter().any(|node| node.position.iter().any(|coordinate| !coordinate.is_finite())) {
        return Err("pattern node positions must be finite".into());
    }
    Ok(())
}
