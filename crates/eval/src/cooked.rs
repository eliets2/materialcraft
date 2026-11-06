use crate::NodeResult;
use materialcraft_core::NodeId;
use materialcraft_image::Image;
use std::collections::BTreeMap;

/// Results of a cook, by node
#[derive(Clone, Debug, Default)]
pub struct Cooked {
    pub nodes: BTreeMap<NodeId, NodeResult>,
    /// Total wall time of the cook in milliseconds
    pub ms: f32,
    pub cancelled: bool,
}

impl Cooked {
    /// Output `port` of node `id`
    pub fn image(&self, id: NodeId, port: usize) -> Option<&Image> {
        self.nodes.get(&id).and_then(|result| result.outputs.get(port)).and_then(crate::NodeOutput::as_image)
    }

    pub fn value(&self, id: NodeId, port: usize) -> Option<&materialcraft_core::Value> {
        self.nodes.get(&id).and_then(|result| result.outputs.get(port)).and_then(crate::NodeOutput::as_value)
    }
}
