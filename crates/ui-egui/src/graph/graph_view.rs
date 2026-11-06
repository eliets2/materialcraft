use super::{GraphDrag, NodeSearch, PortHit, WireRef};
use egui::{Pos2, Rect};
use materialcraft_engine::core::NodeId;

pub struct GraphView {
    pub(crate) inspected_node: Option<NodeId>,
    pub(crate) keyboard_was_focused: bool,
    pub(super) thumbnails_cleared: bool,
    pub(super) drag: Option<GraphDrag>,
    pub(super) search: Option<NodeSearch>,
    /// Node rectangles on screen last frame (for `ui.inspect` and scripted input)
    pub rects: Vec<(NodeId, Rect)>,
    pub ports: Vec<PortHit>,
    pub frames: Vec<(u64, Rect)>,
    pub comments: Vec<(u64, Rect)>,
    /// The canvas rectangle last frame
    pub view: Rect,
    /// Graph point of the last right-click (context menu actions add things there)
    pub(super) menu_at: Pos2,
    /// The graph the camera last framed
    pub(super) framed_graph: Option<usize>,
    /// The selected wires, in the order they were picked (Delete removes them, X swaps them)
    pub selected_wires: Vec<WireRef>,
    /// The wire the context menu is for (right-clicked on a wire)
    pub(super) menu_wire: Option<WireRef>,
    /// Wires on screen last frame: the wire and its bezier control points
    pub wires: Vec<(WireRef, [Pos2; 4])>,
}

impl GraphView {
    pub(crate) fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }
}

impl Default for GraphView {
    fn default() -> Self {
        GraphView {
            inspected_node: None,
            keyboard_was_focused: false,
            thumbnails_cleared: false,
            drag: None,
            search: None,
            rects: Vec::new(),
            ports: Vec::new(),
            frames: Vec::new(),
            comments: Vec::new(),
            view: Rect::NOTHING,
            menu_at: Pos2::ZERO,
            framed_graph: None,
            selected_wires: Vec::new(),
            menu_wire: None,
            wires: Vec::new(),
        }
    }
}
