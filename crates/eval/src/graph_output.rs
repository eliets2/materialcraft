use crate::{Cooked, identifier};
use materialcraft_core::{ComponentUsage, Graph, Node, NodeId, Value};

pub fn output_usage(node: &Node) -> &'static str {
    let index = match node.params.get("usage") {
        None => 0,
        Some(Value::Enum(index)) => *index,
        Some(value) => value.as_i32().map_or(ComponentUsage::ANY_INDEX, |index| index.max(0) as u32),
    };
    ComponentUsage::from_index(index).id
}
use materialcraft_image::Image;

/// The graph's output nodes with their usage id and image, in node order
pub fn outputs<'c>(graph: &Graph, cooked: &'c Cooked) -> Vec<(NodeId, &'static str, String, Option<&'c Image>)> {
    graph.nodes.iter().filter(|n| n.def == "output").map(|n| (n.id, output_usage(n), identifier(n), cooked.image(n.id, 0))).collect()
}
