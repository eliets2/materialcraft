use crate::{Cache, MAX_DEPTH, Options, Port, cook, identifier, instance_graph};
use materialcraft_core::{Node, Package, Value};
use materialcraft_image::{Image, Kind};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Mutex,
};

/// Whether graph `from` reaches `target`, without recursive stack growth
pub(crate) fn instances_reach(pkg: &Package, from: usize, target: usize) -> bool {
    let mut pending = vec![from];
    let mut seen = HashSet::new();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        let Some(graph) = pkg.graphs.get(index) else { continue };

        for (si, _) in graph.nodes.iter().filter(|n| n.def == "graph_instance").filter_map(|n| instance_graph(pkg, n)) {
            if si == target {
                return true;
            }
            pending.push(si);
        }
    }
    false
}

pub(crate) fn cook_instance(
    pkg: &Package,
    gi: usize,
    node: &Node,
    in_ports: &[Port],
    out_ports: &[Port],
    inputs: &[Option<Image>],
    params: &BTreeMap<String, Value>,
    size: (u32, u32),
    opts: &Options,
    cache: &Mutex<Cache>,
) -> Result<Vec<Image>, String> {
    if opts.depth >= MAX_DEPTH {
        return Err("graph instances nest too deep (does a graph use itself?)".into());
    }
    let (sub_gi, sub) = instance_graph(pkg, node).ok_or("the instanced graph does not exist")?;
    if sub_gi == gi || opts.graph_stack.contains(&sub_gi) || instances_reach(pkg, sub_gi, gi) {
        return Err(format!("graph `{}` uses itself through a graph instance", sub.name));
    }
    let mut o = Options::new(opts.resource);
    o.max_size = opts.max_size;
    o.cancel = opts.cancel;
    o.depth = opts.depth + 1;
    o.graph_stack = opts.graph_stack.clone();
    o.graph_stack.push(gi);
    o.inputs = in_ports.iter().zip(inputs).filter_map(|(p, i)| Some((p.id.clone(), i.clone()?))).collect();
    o.overrides = params.iter().filter(|(k, _)| sub.exposed.iter().any(|e| &e.id == *k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let cooked = cook(pkg, sub_gi, &o, cache);
    if cooked.cancelled {
        return Err("cancelled".into());
    }
    Ok(out_ports
        .iter()
        .map(|p| {
            sub.nodes
                .iter()
                .find(|n| n.def == "output" && identifier(n) == p.id)
                .and_then(|n| cooked.image(n.id, 0))
                .map(|i| i.resize(size.0, size.1, true))
                .unwrap_or_else(|| Image::filled(size.0, size.1, Kind::Gray, [0.0; 4]))
        })
        .collect())
}
