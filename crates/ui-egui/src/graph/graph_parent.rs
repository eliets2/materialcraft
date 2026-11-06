use materialcraft_engine::core::NodeId;

pub(crate) struct GraphParent {
    pub graph: usize,
    pub selection: Vec<NodeId>,
    pub camera: crate::state::GraphCamera,
}
