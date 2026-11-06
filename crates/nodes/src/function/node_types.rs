use materialcraft_core::FunctionGraph;
use std::collections::{BTreeMap, BTreeSet};

use super::{catalog, inferred_type::inferred_type};

pub fn node_types(graph: &FunctionGraph) -> Result<BTreeMap<u32, u32>, String> {
    if graph.nodes.len() > 4096 {
        return Err("function exceeds the 4096-node limit".into());
    }

    let nodes: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    if nodes.len() != graph.nodes.len() {
        return Err("duplicate function node ids".into());
    }

    let mut types = BTreeMap::new();
    let mut active = BTreeSet::new();
    for root in &graph.nodes {
        let mut pending = vec![(root.id, false)];
        while let Some((id, visited)) = pending.pop() {
            if types.contains_key(&id) {
                continue;
            }

            let node = nodes.get(&id).ok_or_else(|| format!("missing function node {id}"))?;
            let definition = catalog::definition(&node.op).ok_or_else(|| format!("unknown function `{}`", node.op))?;
            if node.inputs.len() > definition.inputs.len() {
                return Err(format!("{} has too many inputs", node.op));
            }
            if !visited {
                if !active.insert(id) {
                    return Err(format!("function cycle at node {id}"));
                }

                pending.push((id, true));
                pending.extend(node.inputs.iter().rev().flatten().map(|source| (*source, false)));
                continue;
            }

            active.remove(&id);
            let input_type = |slot| node.inputs.get(slot).copied().flatten().and_then(|id| types.get(&id).copied()).unwrap_or(256);
            let inferred = inferred_type(&node.op, node.value.as_ref(), input_type);
            types.insert(id, if node.type_code == 0 { inferred } else { node.type_code });
        }
    }

    if graph.output.is_some_and(|id| !types.contains_key(&id)) {
        return Err("function result references a missing node".into());
    }
    Ok(types)
}
