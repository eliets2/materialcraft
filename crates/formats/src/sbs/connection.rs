use super::warnings::Warnings;
use materialcraft_core::{Connection, Graph, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};

pub(super) fn connect_all(graph: &mut Graph, wires: Vec<Connection>, warnings: &mut Warnings) {
    let graph_name = graph.name.clone();
    let cycle_warning = || format!("graph `{graph_name}`: a wire that would make a cycle was dropped");

    let mut last_into_input: HashMap<(NodeId, String), usize> = HashMap::new();
    let mut kept: Vec<Option<Connection>> = Vec::with_capacity(wires.len());
    for wire in wires {
        if wire.from == wire.to {
            warnings.add(cycle_warning());
            continue;
        }
        if let Some(previous) = last_into_input.insert((wire.to, wire.to_port.clone()), kept.len())
            && let Some(slot) = kept.get_mut(previous)
        {
            *slot = None;
        }
        kept.push(Some(wire));
    }
    let wires: Vec<Connection> = kept.into_iter().flatten().collect();

    // Kahn's algorithm: nodes it can't order lie on or after a cycle
    let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
    let mut downstream: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for wire in &wires {
        *in_degree.entry(wire.to).or_default() += 1;
        downstream.entry(wire.from).or_default().push(wire.to);
    }
    let mut ready: VecDeque<NodeId> = graph.nodes.iter().map(|n| n.id).filter(|id| !in_degree.contains_key(id)).collect();
    let mut ordered: HashSet<NodeId> = HashSet::new();
    while let Some(id) = ready.pop_front() {
        ordered.insert(id);
        for next in downstream.get(&id).into_iter().flatten() {
            if let Some(degree) = in_degree.get_mut(next) {
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(*next);
                }
            }
        }
    }

    // Wires among unordered nodes go through the cycle-checked path, one at a time
    let (safe, risky): (Vec<Connection>, Vec<Connection>) = wires.into_iter().partition(|w| ordered.contains(&w.from) || ordered.contains(&w.to));
    graph.connections = safe;
    for wire in risky {
        if graph.connect(wire.from, &wire.from_port, wire.to, &wire.to_port).is_err() {
            warnings.add(cycle_warning());
        }
    }
}
