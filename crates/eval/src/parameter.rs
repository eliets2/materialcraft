use materialcraft_core::{Node, Value};
use materialcraft_nodes::{NodeDefinition, clamp_params, resolve_params};
use std::collections::BTreeMap;

pub(crate) fn resolve_parameters(
    node: &Node,
    def: Option<&NodeDefinition>,
    vars: &BTreeMap<String, Value>,
    seed: u32,
    size: [u32; 2],
) -> Result<BTreeMap<String, Value>, String> {
    // Parameters: defaults, overrides, then bindings to exposed inputs
    let mut params = match def {
        Some(d) if node.def != "graph_instance" => resolve_params(d, &node.params),
        _ => node.params.clone(),
    };

    for (param, exposed) in &node.bindings {
        if let (Some(v), Some(cur)) = (vars.get(exposed), params.get(param))
            && let Some(c) = cur.coerce_json(&serde_json::to_value(v).unwrap_or_default())
        {
            params.insert(param.clone(), c);
        }
    }
    // Dynamic parameters: functions of the graph's inputs, evaluated once per cook
    for (param, f) in &node.functions {
        let Some(cur) = params.get(param) else { continue };

        match materialcraft_nodes::function::compile(f, seed) {
            Ok(prog) => {
                let context = materialcraft_nodes::function::EvaluationContext {
                    position: [0.5; 2],
                    size: size.map(|side| side as f32),
                    images: &[],
                    tile: true,
                    variables: vars,
                };

                let value = match prog.evaluate(&context, &mut materialcraft_nodes::function::EvaluationState::default()) {
                    Ok(value) => value,
                    Err(error) => return Err(format!("function of `{param}`: {error}")),
                };

                if let Some(c) = cur.coerce_json(&serde_json::to_value(value).unwrap_or_default()) {
                    params.insert(param.clone(), c);
                }
            }
            Err(e) => return Err(format!("function of `{param}`: {e}")),
        }
    }
    // Hard limits on whatever files, bindings and functions supplied (a value can't hang a cook)
    if let Some(d) = def
        && node.def != "graph_instance"
    {
        clamp_params(d, &mut params);
    }
    Ok(params)
}
