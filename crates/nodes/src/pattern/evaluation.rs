use materialcraft_core::{PatternNodeKind, Value};
use std::collections::BTreeMap;

use super::{catalog, pattern_stamp::PatternStamp, pattern_visit::PatternVisit, raster};
use crate::function::{EvaluationContext, EvaluationState, compile};
use crate::{CookContext, CookResult};

const MAX_VISITS: usize = 65_536;

pub(crate) fn cook(context: &CookContext) -> CookResult {
    let Some(Value::PatternGraph(graph)) = context.params.get("pattern") else {
        return Err("missing pattern graph".into());
    };

    graph.validate()?;
    let nodes: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut functions = BTreeMap::new();
    for node in &graph.nodes {
        for (name, value) in &node.parameters {
            if let Value::Function(function) = value {
                functions.insert((node.id, name.as_str()), compile(function, context.seed)?);
            }
        }
    }

    let mut pending = Vec::new();
    if let Some(root) = graph.root {
        pending.push(PatternVisit { node: root, center: [0.5; 2], extent: 1.0, depth: 0, number: 0, seed: context.seed, variables: context.vars.clone() });
    }

    let mut stamps = Vec::new();
    let mut state = EvaluationState::default();
    let mut remaining = MAX_VISITS;
    while let Some(mut visit) = pending.pop() {
        remaining = remaining.checked_sub(1).ok_or("pattern graph exceeded its execution budget")?;
        let node = nodes.get(&visit.node).ok_or("missing pattern node")?;
        if node.kind == PatternNodeKind::Dot {
            if let Some(Some(child)) = node.children.first() {
                visit.node = *child;
                pending.push(visit);
            }
            continue;
        }

        let definition = catalog::definition(node.kind).ok_or("missing pattern definition")?;
        if node.parameters.get("randominherited").and_then(Value::as_bool) == Some(false) {
            visit.seed = node.parameters.get("randomseed").and_then(Value::as_i32).unwrap_or(0) as u32;
        }
        visit.variables.insert("$randomseed".into(), Value::Int(visit.seed as i32));
        visit.variables.insert("$depth".into(), Value::Float(visit.depth as f32));
        visit.variables.insert("$depthpow2".into(), Value::Float(visit.extent));
        visit.variables.insert("$number".into(), Value::Float(visit.number as f32));
        visit.variables.insert("$time".into(), Value::Float(0.0));

        let mut parameters = BTreeMap::new();
        for parameter in &definition.params {
            let value = match functions.get(&(node.id, parameter.id)) {
                Some(program) => {
                    let evaluation = EvaluationContext {
                        position: visit.center,
                        size: [context.w as f32, context.h as f32],
                        images: context.inputs.get(1..).unwrap_or(&[]),
                        tile: context.tile,
                        variables: &visit.variables,
                    };

                    let value = program.evaluate(&evaluation, &mut state)?;
                    visit.variables.clone_from(state.variables());
                    value
                }
                None => node.parameters.get(parameter.id).unwrap_or(&parameter.default).clone(),
            };

            parameters.insert(parameter.id, value);
        }

        let scalar = |name: &str| parameters.get(name).and_then(Value::as_f32).unwrap_or(0.0);
        let integer = |name: &str| parameters.get(name).and_then(Value::as_i32).unwrap_or(0);
        let vector = |name: &str| parameters.get(name).and_then(Value::as_vec4).map(|value| [value[0], value[1]]).unwrap_or([0.0; 2]);
        if parameters.get("randominherited").and_then(Value::as_bool) == Some(false) {
            visit.seed = integer("randomseed") as u32;
        }

        let mut children = Vec::new();
        match node.kind {
            PatternNodeKind::Dot => {}
            PatternNodeKind::Quadrant => {
                let offset = vector("branchoffset");
                visit.center = [visit.center[0] + offset[0] * visit.extent, visit.center[1] + offset[1] * visit.extent];
                let pattern = integer("patterntype");
                let pattern = if pattern == 1 { integer("imageindex").max(1).saturating_add(255) } else { pattern };
                if pattern != 0 {
                    let offset = vector("frameoffset");
                    let color = match parameters.get("opacity") {
                        Some(Value::Float(value)) => [*value, *value, *value, 1.0],
                        Some(value) => value.as_vec4().ok_or("pattern color must be a float or float4")?,
                        None => [1.0; 4],
                    };

                    stamps.push(PatternStamp {
                        center: [visit.center[0] + offset[0] * visit.extent, visit.center[1] + offset[1] * visit.extent],
                        size: vector("patternsize").map(|size| size * visit.extent),
                        rotation: scalar("patternrotation"),
                        color,
                        pattern,
                        variation: scalar("patternsuppl"),
                        blend: integer("blendingmode"),
                        filtering: integer("imagefiltering"),
                        premultiplied: parameters.get("imagepremul").and_then(Value::as_bool).unwrap_or(false),
                    });
                }

                for (slot, child) in node.children.iter().enumerate() {
                    if let Some(child) = child {
                        let mut child_visit = visit.clone();
                        child_visit.node = *child;
                        child_visit.depth = visit.depth.saturating_add(1);
                        child_visit.extent *= 0.5;
                        child_visit.center = [
                            visit.center[0] + (if slot & 1 == 0 { -0.25 } else { 0.25 }) * visit.extent,
                            visit.center[1] + (if slot & 2 == 0 { -0.25 } else { 0.25 }) * visit.extent,
                        ];
                        children.push(child_visit);
                    }
                }
            }
            PatternNodeKind::Switch => {
                let slot = usize::from(parameters.get("switch").and_then(Value::as_bool).unwrap_or(false));
                if let Some(Some(child)) = node.children.get(slot) {
                    visit.node = *child;
                    children.push(visit);
                }
            }
            PatternNodeKind::Iterate => {
                let count = usize::try_from(integer("numberadded")).map_err(|_| "iteration count cannot be negative")?;
                if count > remaining || count > 10_000 {
                    return Err("pattern iteration count exceeds the execution budget".into());
                }
                if let Some(Some(child)) = node.children.first() {
                    let mut first = visit.clone();
                    first.node = *child;
                    first.number = 0;
                    children.push(first);
                }
                if let Some(Some(child)) = node.children.get(1) {
                    for number in 0..count {
                        let mut repeated = visit.clone();
                        repeated.node = *child;
                        repeated.number = number as u32;
                        children.push(repeated);
                    }
                }
            }
        }

        if children.len().saturating_add(pending.len()) > remaining {
            return Err("pattern graph exceeded its execution budget".into());
        }
        pending.extend(children.into_iter().rev());
    }

    Ok(vec![raster::render(context, &stamps)?])
}
