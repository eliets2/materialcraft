use super::{Program, catalog, instruction::Instruction};
use materialcraft_core::FunctionGraph;
use std::collections::{BTreeMap, BTreeSet};

pub fn compile(graph: &FunctionGraph, seed: u32) -> Result<Program, String> {
    if graph.nodes.len() > 4096 {
        return Err("function exceeds the 4096-node execution limit".into());
    }

    let output = graph.output.ok_or("function has no result node")?;
    let mut nodes = BTreeMap::new();
    for node in &graph.nodes {
        if nodes.insert(node.id, node).is_some() {
            return Err(format!("duplicate function node {}", node.id));
        }
    }

    let mut pending = vec![(output, false)];
    let mut active = BTreeSet::new();
    let mut indices = BTreeMap::new();
    let mut instructions: Vec<Instruction> = Vec::new();
    while let Some((id, visited)) = pending.pop() {
        if indices.contains_key(&id) {
            continue;
        }

        let node = nodes.get(&id).ok_or_else(|| format!("missing function node {id}"))?;
        let definition = catalog::definition(&node.op).ok_or_else(|| format!("unknown function `{}`", node.op))?;
        if node.inputs.len() != definition.inputs.len() || node.inputs.iter().any(Option::is_none) {
            return Err(format!("{} ({id}) requires connected inputs: {}", definition.label, definition.inputs.join(", ")));
        }

        if !visited {
            if !active.insert(id) {
                return Err(format!("function cycle at node {id}"));
            }

            pending.push((id, true));
            pending.extend(node.inputs.iter().rev().flatten().map(|input| (*input, false)));
            continue;
        }

        active.remove(&id);
        let inputs: Vec<usize> =
            node.inputs.iter().flatten().map(|input| indices.get(input).copied().ok_or("unresolved function input".to_string())).collect::<Result<_, _>>()?;
        let value = node.value.clone().or_else(|| catalog::default_value(&node.op));
        let input_type = |index: usize| inputs.get(index).and_then(|index| instructions.get(*index)).map_or(256, |instruction| instruction.type_code);
        let inferred = super::inferred_type::inferred_type(&node.op, value.as_ref(), input_type);
        let type_code = if node.type_code == 0 { inferred } else { node.type_code };
        let pure = !matches!(node.op.as_str(), "set" | "sequence" | "while" | "rand" | "isdefined")
            && !node.op.starts_with("get_")
            && inputs.iter().all(|index| instructions.get(*index).is_some_and(|instruction| instruction.pure));
        indices.insert(id, instructions.len());
        instructions.push(Instruction { id, definition, inputs, value, type_code, pure });
    }

    let output = indices.get(&output).copied().ok_or("missing result instruction")?;
    Ok(Program { instructions, output, seed })
}
