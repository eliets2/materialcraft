use egui::{Pos2, Rect};
use materialcraft_engine::{core::NodeId, nodes::PortKind};

pub(super) struct NodeLayout {
    pub(super) id: NodeId,
    pub(super) rect: Rect,
    pub(super) ins: Vec<(String, String, PortKind, Pos2)>,
    pub(super) outs: Vec<(String, String, PortKind, Pos2)>,
}
