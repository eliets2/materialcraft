use materialcraft_core::{Graph, Node, SizeMode};
use materialcraft_nodes::NodeDefinition;

/// Output size of `node` given its first connected input's size
pub fn node_size(graph: &Graph, def: Option<&NodeDefinition>, node: &Node, input: Option<(u32, u32)>, max: Option<u32>) -> (u32, u32) {
    let parent = graph.size_px();
    let (bw, bh) = match node.base.size_mode {
        SizeMode::Absolute => {
            let s = |l: i32| 1u32 << l.clamp(4, 13);
            (s(node.base.size[0]), s(node.base.size[1]))
        }
        mode => {
            let base = match (mode, input, def.is_some_and(|d| d.generator)) {
                (SizeMode::RelativeToInput, Some(i), false) => i,
                _ => (parent[0], parent[1]),
            };

            let sh = |v: u32, o: i32| {
                let v = v as f64 * 2f64.powi(o.clamp(-12, 12));
                v.round().clamp(16.0, 8192.0) as u32
            };
            (sh(base.0, node.base.size[0]), sh(base.1, node.base.size[1]))
        }
    };

    match max {
        Some(m) if bw.max(bh) > m => {
            let k = m as f64 / bw.max(bh) as f64;
            (((bw as f64 * k).round() as u32).max(1), ((bh as f64 * k).round() as u32).max(1))
        }
        _ => (bw, bh),
    }
}
