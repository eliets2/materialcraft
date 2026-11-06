use materialcraft_core::NodeId;
use std::collections::HashMap;

pub(super) struct ReadNode {
    pub(super) id: NodeId,
    pub(super) index: usize,
    pub(super) inputs: HashMap<String, String>,
    pub(super) outputs: Vec<(String, String)>,
    pub(super) open_inputs: bool,
}
