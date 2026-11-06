//! Targeted selection and clipboard commands for standalone and embedded graphs

use crate::{EmbeddedClipboard, Result, Session, commands::bad, graph_edit, selection_target::SelectionTarget};
use materialcraft_core::{FunctionGraph, NodeId, PatternGraph, Value};
use serde_json::{Value as Json, json};

pub fn supports(command: &str) -> bool {
    matches!(
        command,
        "graph.select"
            | "edit.selectAll"
            | "edit.selectNone"
            | "edit.copy"
            | "edit.cut"
            | "edit.paste"
            | "edit.pasteWithoutLinks"
            | "edit.delete"
            | "edit.deleteRelink"
            | "edit.duplicate"
            | "edit.duplicateWithoutLinks"
            | "graph.selectUpstream"
            | "graph.selectDownstream"
    )
}

fn selected(session: &Session, target: &SelectionTarget, parameters: &Json) -> Result<Vec<u32>> {
    if let Some(ids) = parameters.get("ids") {
        return serde_json::from_value(ids.clone()).map_err(|error| bad("graph selection", error.to_string()));
    }
    if target.standalone() {
        return session.selection.iter().map(|id| u32::try_from(id.0).map_err(|_| bad("graph selection", "invalid function node id"))).collect();
    }
    Ok(Vec::new())
}

pub fn enabled(session: &Session, command: &str, parameters: &Json) -> Option<bool> {
    if !handles(session, command, parameters) {
        return None;
    }
    let target = match SelectionTarget::resolve(session, parameters) {
        Ok(Some(target)) => target,
        Ok(None) => return None,
        Err(_) => return Some(false),
    };
    let selection = match selected(session, &target, parameters) {
        Ok(selection) => selection,
        Err(_) => return Some(false),
    };
    enabled_for(session, &target, &selection, command)
}

pub fn enabled_for(session: &Session, target: &SelectionTarget, selection: &[u32], command: &str) -> Option<bool> {
    if !supports(command) {
        return None;
    }
    match target {
        SelectionTarget::Function(target) => {
            if target.current(session).is_err() {
                return Some(false);
            }
            let nodes = match &session.embedded_clipboard {
                Some(EmbeddedClipboard::Function { nodes, .. }) => nodes.as_slice(),
                _ => &[],
            };
            graph_edit::enabled::<FunctionGraph>(selection, nodes, command)
        }
        SelectionTarget::Pattern(owner) => {
            if SelectionTarget::pattern(session, *owner).is_err() {
                return Some(false);
            }
            let nodes = match &session.embedded_clipboard {
                Some(EmbeddedClipboard::Pattern { nodes, .. }) => nodes.as_slice(),
                _ => &[],
            };
            graph_edit::enabled::<PatternGraph>(selection, nodes, command)
        }
    }
}

fn handles(session: &Session, command: &str, parameters: &Json) -> bool {
    supports(command)
        && !(parameters.get("function").is_none()
            && parameters.get("pattern").is_none()
            && session.annotation_selection.is_some()
            && matches!(command, "edit.delete" | "edit.deleteRelink"))
}

pub(crate) fn execute(session: &mut Session, command: &str, parameters: &Json) -> Option<Result<Json>> {
    if !handles(session, command, parameters) {
        return None;
    }
    match SelectionTarget::resolve(session, parameters) {
        Ok(Some(target)) => Some(run(session, command, parameters, target)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    }
}

fn run(session: &mut Session, command: &str, parameters: &Json, target: SelectionTarget) -> Result<Json> {
    let source = target.source(session)?;
    let mut selection = selected(session, &target, parameters)?;
    if command == "graph.select" && target.standalone() && parameters.get("add").and_then(Json::as_bool) == Some(true) {
        for id in session.selection.iter().filter_map(|id| u32::try_from(id.0).ok()) {
            if !selection.contains(&id) {
                selection.push(id);
            }
        }
    }
    let copied_count = selection.len();
    let at = crate::commands::point(parameters, "at").unwrap_or([0.0; 2]);
    let snap = parameters.get("snapToGrid").and_then(Json::as_bool).unwrap_or(false);
    let supplied: Option<EmbeddedClipboard> =
        parameters.get("clip").cloned().map(serde_json::from_value).transpose().map_err(|error| bad(command, error.to_string()))?;
    let clipboard = supplied.as_ref().or(session.embedded_clipboard.as_ref());
    let copying = matches!(command, "edit.copy" | "edit.cut");
    let updated_clipboard = match &target {
        SelectionTarget::Function(function) => {
            let (nodes, origin) = match clipboard {
                Some(EmbeddedClipboard::Function { nodes, source }) => (nodes.as_slice(), source.as_str()),
                _ => (&[][..], ""),
            };
            let (graph, copied) = apply(function.current(session)?, &mut selection, nodes, paste_command(command, origin == source), at, snap)?;
            if let Some(graph) = graph {
                crate::graph_validation::function(&graph).map_err(|error| bad(command, error))?;
                session.edit("Edit Function Graph", |session| function.replace(session, graph))?;
            }
            copying.then_some(EmbeddedClipboard::Function { source, nodes: copied })
        }
        SelectionTarget::Pattern(owner) => {
            let (nodes, origin) = match clipboard {
                Some(EmbeddedClipboard::Pattern { nodes, source }) => (nodes.as_slice(), source.as_str()),
                _ => (&[][..], ""),
            };
            let (graph, copied) = apply(SelectionTarget::pattern(session, *owner)?, &mut selection, nodes, paste_command(command, origin == source), at, snap)?;
            if let Some(graph) = graph {
                crate::graph_validation::pattern(&graph).map_err(|error| bad(command, error))?;
                session.edit("Edit FX-Map graph", |session| {
                    session.graph_mut()?.node_mut(*owner)?.params.insert("pattern".into(), Value::PatternGraph(Box::new(graph)));
                    Ok(())
                })?;
            }
            copying.then_some(EmbeddedClipboard::Pattern { source, nodes: copied })
        }
    };
    if target.standalone() {
        session.selection = selection.iter().map(|id| NodeId(u64::from(*id))).collect();
        session.annotation_selection = None;
    }
    let mut result = json!({"ids": selection, "selected": selection.len()});
    if copying {
        result["clip"] = json!(updated_clipboard);
        result["copied"] = json!(copied_count);
    }
    if let Some(clipboard) = updated_clipboard {
        session.embedded_clipboard = Some(clipboard);
    }
    Ok(result)
}

fn paste_command(command: &str, same_source: bool) -> &str {
    if command == "edit.paste" && !same_source { "edit.pasteWithoutLinks" } else { command }
}

fn apply<G: graph_edit::EditableGraph + Clone>(
    graph: &G,
    selection: &mut Vec<u32>,
    clipboard: &[G::Node],
    command: &str,
    at: [f32; 2],
    snap: bool,
) -> Result<(Option<G>, Vec<G::Node>)> {
    if !matches!(command, "edit.selectAll" | "edit.selectNone") {
        let present: std::collections::BTreeSet<_> = graph.nodes().iter().map(G::id).collect();
        if selection.iter().any(|id| !present.contains(id)) {
            return Err(bad(command, "selection contains a missing node"));
        }
    }
    if graph_edit::enabled::<G>(selection, clipboard, command) != Some(true) {
        return Err(bad(command, "the selection or clipboard does not support this action"));
    }
    let mut copied = Vec::new();
    if graph_edit::read(graph, selection, &mut copied, command) {
        return Ok((None, copied));
    }
    if command.starts_with("edit.paste") {
        copied = clipboard.to_vec();
    }
    let mut edited = graph.clone();
    let changed = graph_edit::run(&mut edited, selection, &mut copied, command, at, snap).map_err(|error| bad(command, error))?;
    Ok((changed.then_some(edited), copied))
}
