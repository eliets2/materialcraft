use crate::commands::{CommandSpec, bad, has_graph, strp};
use crate::selection_target::SelectionTarget;
use crate::{Result, Session, cmd};
use materialcraft_core::Value;
use serde_json::Value as Json;

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.align",
        "Align nodes",
        [],
        None,
        "{mode: left|right|top|bottom|hcenter|vcenter|hdistribute|vdistribute, ids?: required for Function/FX-Map, pattern?: owner node id, function?: FunctionGraphTarget, graph?: owning graph index}",
        has_graph,
        align
    )]
}

fn align(session: &mut Session, parameters: &Json) -> Result<Json> {
    let mode = strp(parameters, "mode").unwrap_or("left");
    let ids: Vec<u64> = parameters
        .get("ids")
        .and_then(Json::as_array)
        .map(|ids| ids.iter().filter_map(Json::as_u64).collect())
        .unwrap_or_else(|| session.selection.iter().map(|id| id.0).collect());
    let target = SelectionTarget::resolve(session, parameters)?;
    if target.is_some() && parameters.get("ids").and_then(Json::as_array).is_none() {
        return Err(bad("graph.align", "ids are required for a Function Graph or FX-Map"));
    }
    match target {
        Some(SelectionTarget::Function(target)) => {
            let mut graph = target.current(session)?.clone();
            align_selected(graph.nodes.iter_mut().map(|node| (u64::from(node.id), &mut node.pos)).collect(), &ids, mode)?;
            crate::graph_validation::function(&graph).map_err(|error| bad("graph.align", error))?;
            session.edit("Align nodes", |session| target.replace(session, graph))?;
        }
        Some(SelectionTarget::Pattern(owner)) => {
            let mut graph = SelectionTarget::pattern(session, owner)?.clone();
            align_selected(graph.nodes.iter_mut().map(|node| (u64::from(node.id), &mut node.position)).collect(), &ids, mode)?;
            crate::graph_validation::pattern(&graph).map_err(|error| bad("graph.align", error))?;
            session.edit("Align nodes", |session| {
                session.graph_mut()?.node_mut(owner)?.params.insert("pattern".into(), Value::PatternGraph(Box::new(graph)));
                Ok(())
            })?;
        }
        None => {
            session.edit("Align nodes", |session| {
                align_selected(session.graph_mut()?.nodes.iter_mut().map(|node| (node.id.0, &mut node.pos)).collect(), &ids, mode)
            })?;
        }
    }
    Ok(Json::Null)
}

fn align_selected(mut positions: Vec<(u64, &mut [f32; 2])>, ids: &[u64], mode: &str) -> Result<()> {
    let present: std::collections::BTreeSet<_> = positions.iter().map(|(id, _)| *id).collect();
    if let Some(id) = ids.iter().find(|id| !present.contains(id)) {
        return Err(bad("graph.align", format!("no node {id}")));
    }
    let selected: std::collections::BTreeSet<_> = ids.iter().copied().collect();
    positions.retain(|(id, _)| selected.contains(id));
    align_positions(positions.into_iter().map(|(_, position)| position).collect(), mode)
}

fn align_positions(mut positions: Vec<&mut [f32; 2]>, mode: &str) -> Result<()> {
    let axis = match mode {
        "left" | "right" | "hcenter" | "hdistribute" => 0,
        "top" | "bottom" | "vcenter" | "vdistribute" => 1,
        other => return Err(bad("graph.align", format!("unknown mode `{other}`"))),
    };

    if positions.len() < 2 {
        return Ok(());
    }

    let minimum = positions.iter().map(|position| position[axis]).fold(f32::MAX, f32::min);
    let maximum = positions.iter().map(|position| position[axis]).fold(f32::MIN, f32::max);
    if mode.ends_with("distribute") {
        positions.sort_by(|left, right| left[axis].total_cmp(&right[axis]));
        let step = (maximum - minimum) / (positions.len() - 1) as f32;
        for (index, position) in positions.iter_mut().enumerate() {
            position[axis] = minimum + step * index as f32;
        }
    } else {
        let value = match mode {
            "left" | "top" => minimum,
            "right" | "bottom" => maximum,
            _ => (minimum + maximum) * 0.5,
        };
        for position in positions {
            position[axis] = value;
        }
    }
    Ok(())
}
