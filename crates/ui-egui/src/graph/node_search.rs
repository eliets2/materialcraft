use egui::Pos2;
use materialcraft_engine::core::NodeId;

/// The node search popup (Space or Tab)
#[derive(Clone, Debug)]
pub(super) struct NodeSearch {
    pub(super) at: Pos2,
    pub(super) text: String,
    pub(super) sel: usize,
    pub(super) connect: Option<(NodeId, String)>,
    pub(super) focus: bool,
}
