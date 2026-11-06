use super::WireRef;
use egui::{Pos2, Vec2};
use materialcraft_engine::core::NodeId;

#[derive(Clone, Debug)]
pub(super) enum GraphDrag {
    Nodes {
        delta: Vec2,
    },
    Box {
        start: Pos2,
    },
    Wire {
        from: (NodeId, String),
        detached: Option<(NodeId, String)>,
    },
    /// Pressed on a wire: a click selects it, a drag pulls a reroute dot out of it
    WirePress {
        wire: WireRef,
        start: Pos2,
    },
    Pan(egui::PointerButton),
    Frame {
        id: u64,
        delta: Vec2,
        resize: bool,
        contents: materialcraft_engine::FrameContents,
    },
    Comment {
        id: u64,
        delta: Vec2,
    },
}
