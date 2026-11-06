use super::EditableGraph;
use std::collections::{BTreeMap, BTreeSet};

pub fn enabled<G: EditableGraph>(selection: &[u32], clipboard: &[G::Node], command: &str) -> Option<bool> {
    Some(match command {
        "graph.select" | "edit.selectAll" | "edit.selectNone" => true,
        "edit.paste" | "edit.pasteWithoutLinks" => !clipboard.is_empty(),
        "edit.copy" | "edit.cut" | "edit.delete" | "edit.duplicate" | "edit.duplicateWithoutLinks" | "graph.selectUpstream" | "graph.selectDownstream" => {
            !selection.is_empty()
        }
        "edit.deleteRelink" => G::RELINK && !selection.is_empty(),
        "graph.node.disable" => false,
        _ => return None,
    })
}

pub fn run<G: EditableGraph>(
    graph: &mut G,
    selection: &mut Vec<u32>,
    clipboard: &mut Vec<G::Node>,
    command: &str,
    at: [f32; 2],
    snap: bool,
) -> Result<bool, String> {
    if read(graph, selection, clipboard, command) {
        return Ok(false);
    }
    match command {
        "edit.cut" => {
            *clipboard = selected::<G>(graph, selection);
            return Ok(remove(graph, selection, false));
        }
        "edit.delete" | "edit.deleteRelink" => return Ok(remove(graph, selection, G::RELINK && command == "edit.deleteRelink")),
        "edit.duplicate" | "edit.duplicateWithoutLinks" | "edit.paste" | "edit.pasteWithoutLinks" => {
            let duplicate = command.starts_with("edit.duplicate");
            let mut nodes = if duplicate { selected::<G>(graph, selection) } else { clipboard.clone() };
            graph.check_capacity(nodes.len())?;
            let anchor = nodes.first_mut().map_or([0.0; 2], |node| *G::position(node));
            let offset = if duplicate { [32.0; 2] } else { [at[0] - anchor[0], at[1] - anchor[1]] };
            let present: BTreeSet<_> = graph.nodes().iter().map(G::id).collect();
            let mut ids = BTreeMap::new();
            for mut node in nodes {
                let original = G::id(&node);
                let position = G::position(&mut node);
                *position = crate::graph_grid::translate(*position, offset, snap);
                ids.insert(original, graph.insert(node)?);
            }
            let inserted: BTreeSet<_> = ids.values().copied().collect();
            for node in graph.nodes_mut().iter_mut().filter(|node| inserted.contains(&G::id(node))) {
                for link in G::links_mut(node) {
                    *link = link.and_then(|id| ids.get(&id).copied().or_else(|| (!command.ends_with("WithoutLinks") && present.contains(&id)).then_some(id)));
                }
            }
            *selection = ids.values().copied().collect();
            return Ok(!ids.is_empty());
        }
        _ => {}
    }
    Ok(false)
}

fn selected<G: EditableGraph>(graph: &G, selection: &[u32]) -> Vec<G::Node> {
    let selection: BTreeSet<_> = selection.iter().copied().collect();
    graph.nodes().iter().filter(|node| selection.contains(&G::id(node))).cloned().collect()
}

pub fn remove<G: EditableGraph>(graph: &mut G, selection: &mut Vec<u32>, relink: bool) -> bool {
    let selection_set: BTreeSet<_> = selection.iter().copied().collect();
    let replacements: BTreeMap<_, _> = graph
        .nodes()
        .iter()
        .filter(|node| selection_set.contains(&G::id(node)))
        .map(|node| (G::id(node), if relink { G::links(node).first().copied().flatten() } else { None }))
        .collect();
    graph.nodes_mut().retain(|node| !selection_set.contains(&G::id(node)));
    for node in graph.nodes_mut() {
        for link in G::links_mut(node) {
            let mut seen = BTreeSet::new();
            while let Some(id) = *link {
                let Some(replacement) = replacements.get(&id) else { break };

                if !seen.insert(id) {
                    *link = None;
                    break;
                }
                *link = *replacement;
            }
        }
    }
    if graph.result().is_some_and(|id| selection_set.contains(&id)) {
        *graph.result() = None;
    }
    selection.clear();
    !replacements.is_empty()
}

/// Apply selection and copy actions without cloning or mutating the graph
pub fn read<G: EditableGraph>(graph: &G, selection: &mut Vec<u32>, clipboard: &mut Vec<G::Node>, command: &str) -> bool {
    match command {
        "edit.selectAll" => *selection = graph.nodes().iter().map(G::id).collect(),
        "edit.selectNone" => selection.clear(),
        "graph.select" => {}
        "edit.copy" => *clipboard = selected::<G>(graph, selection),
        "graph.selectUpstream" | "graph.selectDownstream" => {
            let follow_links = (command == "graph.selectUpstream") == G::LINKS_ARE_INPUTS;
            let mut neighbors: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
            for node in graph.nodes() {
                for link in G::links(node).iter().flatten() {
                    let (from, to) = if follow_links { (G::id(node), *link) } else { (*link, G::id(node)) };
                    neighbors.entry(from).or_default().push(to);
                }
            }
            let mut visited = BTreeSet::new();
            let mut pending = selection.clone();
            while let Some(id) = pending.pop() {
                if visited.insert(id)
                    && let Some(next) = neighbors.get(&id)
                {
                    pending.extend(next);
                }
            }
            *selection = visited.into_iter().collect();
        }
        _ => return false,
    }
    true
}
