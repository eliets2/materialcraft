//! Edit menu: undo/redo, clipboard, duplicate, delete and selection

use std::collections::BTreeMap;

use serde_json::{Value as J, json};

use materialcraft_core::{Connection, NodeId};

use crate::Clip;
use crate::commands::{CommandSpec, always, bad, has_graph, has_selection, ids_param, point};
use crate::{Result, Session, cmd};

fn can_undo(s: &Session) -> std::result::Result<(), String> {
    if s.can_undo() { Ok(()) } else { Err("nothing to undo".into()) }
}

fn can_redo(s: &Session) -> std::result::Result<(), String> {
    if s.can_redo() { Ok(()) } else { Err("nothing to redo".into()) }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("edit.undo", "Undo", ["Edit"], Some("Ctrl+Z"), "{}", can_undo, |s, _| Ok(json!({"undone": s.undo()}))),
        cmd!("edit.redo", "Redo", ["Edit"], Some("Ctrl+Y"), "{}", can_redo, |s, _| Ok(json!({"redone": s.redo()}))),
        cmd!("edit.copy", "Copy selection", [], Some("Ctrl+C"), "{ids?} copies nodes (and the wires between them)", has_selection, copy),
        cmd!("edit.cut", "Cut selection", [], Some("Ctrl+X"), "{ids?}", has_selection, cut),
        cmd!("edit.paste", "Paste", [], Some("Ctrl+V"), "{at? [x, y], clip? (JSON from edit.copy)} pastes nodes, selected", has_clip_or_param, paste),
        cmd!(
            "edit.duplicate",
            "Duplicate selection",
            [],
            Some("Ctrl+D"),
            "{ids?} duplicates nodes with their inner wires and their inputs",
            has_selection,
            duplicate
        ),
        cmd!(
            "edit.pasteWithoutLinks",
            "Paste without link",
            [],
            Some("Ctrl+Shift+V"),
            "{at?, snapToGrid?} pastes without incoming links",
            has_clip_or_param,
            |session, parameters| {
                let mut parameters = parameters.as_object().cloned().unwrap_or_default();
                parameters.insert("withoutLinks".into(), J::Bool(true));
                paste(session, &J::Object(parameters))
            }
        ),
        cmd!(
            "edit.duplicateWithoutLinks",
            "Duplicate selection without link",
            [],
            Some("Ctrl+Shift+D"),
            "{ids?, snapToGrid?} duplicates without incoming links",
            has_selection,
            |session, parameters| {
                let mut parameters = parameters.as_object().cloned().unwrap_or_default();
                parameters.insert("withoutLinks".into(), J::Bool(true));
                duplicate(session, &J::Object(parameters))
            }
        ),
        cmd!("edit.delete", "Delete selection", [], Some("Delete"), "{ids?}", has_deletable_selection, delete),
        cmd!(
            "edit.deleteRelink",
            "Delete and relink",
            [],
            Some("Backspace"),
            "{ids?} deletes nodes and joins what fed their first input to what they fed",
            has_deletable_selection,
            delete_relink
        ),
        cmd!("edit.selectAll", "Select all", [], Some("Ctrl+A"), "{}", has_graph, |s, _| {
            s.selection = s.graph()?.nodes.iter().map(|n| n.id).collect();
            Ok(json!({"selected": s.selection.len()}))
        }),
        cmd!("edit.selectNone", "Deselect", [], None, "{}", always, |s, _| {
            s.selection.clear();
            s.annotation_selection = None;
            Ok(J::Null)
        }),
    ]
}

fn has_clip_or_param(s: &Session) -> std::result::Result<(), String> {
    // `clip` may come with the params; the check happens in `paste`
    has_graph(s)
}

fn clip_of(s: &Session, ids: &[NodeId]) -> Result<Clip> {
    let g = s.graph()?;
    Ok(Clip {
        nodes: g.nodes.iter().filter(|n| ids.contains(&n.id)).cloned().collect(),
        connections: g.connections.iter().filter(|c| ids.contains(&c.from) && ids.contains(&c.to)).cloned().collect(),
        inputs: g.connections.iter().filter(|c| !ids.contains(&c.from) && ids.contains(&c.to)).cloned().collect(),
        source_graph: Some(g.name.clone()),
    })
}

fn copy(s: &mut Session, p: &J) -> Result<J> {
    let ids = ids_param(s, "edit.copy", p)?;
    let clip = clip_of(s, &ids)?;
    let out = serde_json::to_value(&clip).unwrap_or(J::Null);
    s.clipboard = Some(clip);
    Ok(json!({"copied": ids.len(), "clip": out}))
}

fn cut(s: &mut Session, p: &J) -> Result<J> {
    let r = copy(s, p)?;
    delete(s, p)?;
    Ok(r)
}

/// Insert copied nodes and their wires with fresh ids
fn insert_clip(s: &mut Session, clip: &Clip, at: Option<[f32; 2]>, offset: [f32; 2], outer: &[Connection], snap: bool) -> Result<Vec<NodeId>> {
    let g = s.graph_mut()?;
    let min = clip.nodes.iter().fold([f32::MAX, f32::MAX], |m, n| [m[0].min(n.pos[0]), m[1].min(n.pos[1])]);
    let shift = match at {
        Some(a) if min[0] < f32::MAX => [a[0] - min[0], a[1] - min[1]],
        _ => offset,
    };

    let mut map: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    for n in &clip.nodes {
        let id = NodeId(g.alloc_id());
        map.insert(n.id, id);
        let mut c = n.clone();
        c.id = id;
        c.pos = crate::graph_grid::translate(n.pos, shift, snap);
        g.nodes.push(c);
    }
    for c in &clip.connections {
        if let (Some(a), Some(b)) = (map.get(&c.from), map.get(&c.to)) {
            let _ = g.connect(*a, &c.from_port, *b, &c.to_port);
        }
    }
    for c in outer {
        if let Some(b) = map.get(&c.to)
            && g.node(c.from).is_ok()
        {
            let _ = g.connect(c.from, &c.from_port, *b, &c.to_port);
        }
    }
    Ok(map.into_values().collect())
}

fn paste(s: &mut Session, p: &J) -> Result<J> {
    let clip = match p.get("clip") {
        Some(c) => serde_json::from_value::<Clip>(c.clone()).map_err(|e| bad("edit.paste", format!("bad clip: {e}")))?,
        None => s.clipboard.clone().ok_or_else(|| bad("edit.paste", "the clipboard is empty"))?,
    };

    if clip.nodes.len() > 10_000 {
        return Err(bad("edit.paste", "too many nodes"));
    }
    let at = point(p, "at");
    let keep_inputs = !crate::commands::boolp(p, "withoutLinks").unwrap_or(false) && clip.source_graph.as_deref() == Some(s.graph()?.name.as_str());
    let snap = crate::commands::boolp(p, "snapToGrid").unwrap_or(false);

    s.edit("Paste", |s| {
        let ids = insert_clip(s, &clip, at, [40.0, 40.0], if keep_inputs { &clip.inputs } else { &[] }, snap)?;
        s.selection = ids.clone();
        Ok(json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}))
    })
}

fn duplicate(s: &mut Session, p: &J) -> Result<J> {
    let ids = ids_param(s, "edit.duplicate", p)?;
    let clip = clip_of(s, &ids)?;
    let keep = !matches!(p.get("withoutLinks"), Some(J::Bool(true)));
    let snap = crate::commands::boolp(p, "snapToGrid").unwrap_or(false);
    s.edit("Duplicate", |s| {
        let new = insert_clip(s, &clip, None, [40.0, 40.0], if keep { &clip.inputs } else { &[] }, snap)?;
        s.selection = new.clone();
        Ok(json!({"ids": new.iter().map(|i| i.0).collect::<Vec<_>>()}))
    })
}

fn has_deletable_selection(session: &Session) -> std::result::Result<(), String> {
    if session.annotation_selection.is_some() { has_graph(session) } else { has_selection(session) }
}

fn delete_annotation(session: &mut Session) -> Result<J> {
    let selection = session.annotation_selection;
    session.edit("Delete annotation", |session| {
        let graph = session.graph_mut()?;
        match selection {
            Some(crate::AnnotationSelection::Frame(id)) => graph.frames.retain(|frame| frame.id != id),
            Some(crate::AnnotationSelection::Comment(id)) => graph.comments.retain(|comment| comment.id != id),
            None => {}
        }
        session.annotation_selection = None;
        Ok(json!({"deleted": 1}))
    })
}

fn delete(s: &mut Session, p: &J) -> Result<J> {
    if s.annotation_selection.is_some() && p.get("ids").is_none() {
        return delete_annotation(s);
    }

    let ids = ids_param(s, "edit.delete", p)?;
    s.edit("Delete", |s| {
        s.graph_mut()?.remove_nodes(&ids);
        s.selection.retain(|i| !ids.contains(i));
        if s.view2d.node.is_some_and(|n| ids.contains(&n)) {
            s.view2d.node = None;
        }
        Ok(json!({"deleted": ids.len()}))
    })
}

fn delete_relink(s: &mut Session, p: &J) -> Result<J> {
    if s.annotation_selection.is_some() && p.get("ids").is_none() {
        return delete_annotation(s);
    }

    let ids = ids_param(s, "edit.deleteRelink", p)?;
    s.edit("Delete and relink", |s| {
        let ports_by_id: Vec<Vec<String>> = ids.iter().map(|id| s.input_ports(*id)).collect();
        let id_set: std::collections::BTreeSet<NodeId> = ids.iter().cloned().collect();
        let (sources, outs) = {
            let graph = s.graph()?;
            let mut sources: BTreeMap<NodeId, Option<(NodeId, String)>> = BTreeMap::new();
            for (id, ports) in ids.iter().zip(&ports_by_id) {
                // Port order comes from the node definition, not the order its wires were created.
                let node = graph.node(*id).ok();
                let port = ports.iter().find(|port| !node.is_some_and(|node| node.input_values.contains_key(*port)));
                let entry = port.and_then(|port| graph.input_of(*id, port)).map(|c| (c.from, c.from_port.clone()));
                sources.insert(*id, entry);
            }
            let outs: BTreeMap<NodeId, Vec<(NodeId, String)>> =
                graph.connections.iter().filter(|c| id_set.contains(&c.from)).map(|c| (c.from, (c.to, c.to_port.clone()))).fold(
                    BTreeMap::new(),
                    |mut map, (from, to)| {
                        map.entry(from).or_default().push(to);
                        map
                    },
                );
            (sources, outs)
        };
        // Only deleted nodes feeding a surviving target need a resolved source.
        let needed: std::collections::BTreeSet<NodeId> =
            outs.iter().filter(|(_, tos)| tos.iter().any(|(to, _)| !id_set.contains(to))).map(|(from, _)| *from).collect();
        let mut resolved: BTreeMap<NodeId, Option<(NodeId, String)>> = BTreeMap::new();
        for id in &needed {
            if resolved.contains_key(id) {
                continue;
            }
            let mut path: Vec<NodeId> = Vec::new();
            let mut seen: std::collections::BTreeSet<NodeId> = std::collections::BTreeSet::new();
            let mut current = sources.get(id).cloned().flatten();
            let terminal: Option<(NodeId, String)> = loop {
                match current {
                    None => break None,
                    Some((from, port)) if !id_set.contains(&from) => break Some((from, port)),
                    Some((from, _)) => {
                        if let Some(r) = resolved.get(&from) {
                            break r.clone();
                        }
                        if !seen.insert(from) {
                            break None;
                        }
                        path.push(from);
                        current = sources.get(&from).cloned().flatten();
                    }
                }
            };
            resolved.insert(*id, terminal.clone());
            for node in path {
                resolved.insert(node, terminal.clone());
            }
        }
        let graph = s.graph_mut()?;
        graph.remove_nodes(&ids);
        // One pass per surviving target: it gets the declared source of
        // its upstream deleted chain. A deleted intermediate is only ever
        // used as a waypoint in that walk, never as a fan-out itself.
        for from in outs.keys() {
            for (to, to_port) in &outs[from] {
                if id_set.contains(to) {
                    continue;
                }
                if let Some((source, port)) = resolved.get(from).cloned().flatten() {
                    // Only connect the deleted chain's declared source; do not
                    // create a second fan-out from an intermediate deleted node.
                    let _ = graph.connect(source, &port, *to, to_port);
                }
            }
        }
        s.selection.retain(|i| !id_set.contains(i));
        Ok(json!({"deleted": ids.len()}))
    })
}
