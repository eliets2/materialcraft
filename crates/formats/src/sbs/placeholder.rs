use super::warnings::Warnings;
use materialcraft_core::{MISSING_NODE, Node, Package, Value, missing_ports};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn report_placeholders(pkg: &Package, warnings: &mut Warnings) {
    for graph in &pkg.graphs {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for node in graph.nodes.iter().filter(|n| n.def == MISSING_NODE) {
            let name = match node.params.get("sbs_name") {
                Some(Value::Text(s)) if !s.is_empty() => s.clone(),
                _ => node.label.clone(),
            };
            *counts.entry(name).or_default() += 1;
        }

        for (name, count) in counts {
            let times = if count > 1 { format!(" ×{count}") } else { String::new() };
            warnings.add(format!("graph `{}`: missing node `{name}`{times} is not in MaterialCraft's library yet; kept as a placeholder", graph.name));
        }
    }
}
pub(super) fn fill_placeholder(
    node: &mut Node,
    kind: &str,
    name: &str,
    package: &str,
    raw_params: Option<&str>,
    output_uids: &[(String, bool)],
    bridgings: Option<&[(String, String)]>,
) {
    node.label = name.to_string();
    node.params.insert("sbs_kind".into(), Value::Text(kind.into()));
    node.params.insert("sbs_name".into(), Value::Text(name.into()));
    if !package.is_empty() {
        node.params.insert("sbs_package".into(), Value::Text(package.into()));
    }
    if let Some(raw) = raw_params {
        node.params.insert("sbs_parameters".into(), Value::Text(raw.into()));
    }

    let mut seen = BTreeSet::new();
    let outputs: Vec<String> = placeholder_outputs(output_uids, bridgings)
        .into_iter()
        .filter(|(_, port)| seen.insert(port.clone()))
        .map(|(uid, port)| if output_uids.iter().any(|(u, gray)| *u == uid && *gray) { format!("{port}:gray") } else { port })
        .collect();
    node.params.insert("outputs".into(), Value::Text(outputs.join(",")));
    node.params.insert("inputs".into(), Value::Text(String::new()));
}
pub(super) fn placeholder_outputs(output_uids: &[(String, bool)], bridgings: Option<&[(String, String)]>) -> Vec<(String, String)> {
    let bridged = |uid: &str| bridgings.and_then(|b| b.iter().find(|(u, _)| u == uid)).map(|(_, identifier)| identifier.clone());
    let mut outputs: Vec<(String, String)> = output_uids
        .iter()
        .enumerate()
        .map(|(i, (uid, _))| (uid.clone(), bridged(uid).unwrap_or_else(|| if i == 0 { "output".to_string() } else { format!("output_{}", i + 1) })))
        .collect();

    for (uid, identifier) in bridgings.unwrap_or_default() {
        if !outputs.iter().any(|(u, _)| u == uid) {
            outputs.push((uid.clone(), identifier.clone()));
        }
    }
    outputs
}
pub(super) fn add_placeholder_input(node: &mut Node, port: &str) {
    if port.is_empty() || port.contains(',') {
        return;
    }

    let (mut inputs, _) = missing_ports(node);
    if !inputs.iter().any(|p| p == port) {
        inputs.push(port.to_string());
        node.params.insert("inputs".into(), Value::Text(inputs.join(",")));
    }
}
