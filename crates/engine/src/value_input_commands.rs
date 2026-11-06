use materialcraft_core::ValueType;
use serde_json::{Value as JsonValue, json};

use crate::commands::{CommandSpec, bad, has_graph, node_param, strp};
use crate::{Result, Session, cmd};

const COMMAND: &str = "graph.node.value_input";

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.node.value_input",
        "Edit value input",
        [],
        None,
        "{id, name, type? (Float, Float2, Float3, Float4, Integer, Integer2, Integer3, Integer4, Boolean, String), remove?}",
        has_graph,
        edit
    )]
}

fn edit(session: &mut Session, parameters: &JsonValue) -> Result<JsonValue> {
    let id = node_param(session, COMMAND, parameters, "id")?;
    let name = strp(parameters, "name").unwrap_or("").trim().to_string();
    if name.is_empty() || name.len() > 256 || name.starts_with('$') {
        return Err(bad(COMMAND, "use a nonempty input name up to 256 bytes, without a leading $"));
    }

    let remove = parameters.get("remove").and_then(JsonValue::as_bool).unwrap_or(false);
    let type_name = strp(parameters, "type").unwrap_or("Float");
    let value_type = ValueType::ALL
        .into_iter()
        .find(|value_type| value_type.label() == type_name)
        .ok_or_else(|| bad(COMMAND, format!("unknown value type `{type_name}`")))?;
    let node = session.graph()?.node(id)?;
    let definition = materialcraft_nodes::find(&node.def);
    if definition.is_some_and(|definition| definition.input_index(&name).is_some()) {
        return Err(bad(COMMAND, "an image input already uses that name"));
    }
    if !remove && node.input_values.len() >= 64 && !node.input_values.contains_key(&name) {
        return Err(bad(COMMAND, "a node supports up to 64 value inputs"));
    }

    session.edit("Edit value input", |session| {
        let graph = session.graph_mut()?;
        let node = graph.node_mut(id)?;
        if remove {
            node.input_values.remove(&name);
            graph.disconnect(id, &name);
        } else {
            node.input_values.insert(name, value_type);
        }

        Ok(json!(null))
    })
}
