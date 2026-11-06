use materialcraft_core::{Graph, NodeId, Value, ValueType};
use std::collections::BTreeSet;

use crate::find;

pub fn output_value_type(graph: &Graph, mut id: NodeId) -> Result<Option<ValueType>, String> {
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(id) {
            return Err("cycle while resolving a value connection".into());
        }

        let node = graph.node(id).map_err(|error| error.to_string())?;
        match node.def.as_str() {
            "dot" | "output" => match graph.input_of(id, "input") {
                Some(connection) => id = connection.from,
                None => return Ok(None),
            },
            "input_value" => return Ok(Some(node.params.get("default").and_then(ValueType::of).unwrap_or(ValueType::Float))),
            "value_processor" => {
                let default = find("value_processor").and_then(|definition| definition.param("function"));
                let function = node.params.get("function").or_else(|| default.map(|parameter| &parameter.default));
                let Some(Value::Function(function)) = function else {
                    return Err("missing value function".into());
                };

                let types = crate::function::node_types(function)?;
                let code = function.output.and_then(|id| types.get(&id).copied()).unwrap_or(256);
                return ValueType::from_code(code).map(Some).ok_or_else(|| format!("unsupported value type {code}"));
            }
            _ => return Ok(None),
        }
    }
}
