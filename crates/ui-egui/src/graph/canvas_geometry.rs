use super::{GraphView, PortHit, WireRef};
use egui::{Pos2, Rect};
use materialcraft_engine::core::NodeId;

#[derive(Default)]
pub(crate) struct CanvasGeometry {
    pub rects: Vec<(NodeId, Rect)>,
    pub ports: Vec<PortHit>,
    pub wires: Vec<(WireRef, [Pos2; 4])>,
}

impl CanvasGeometry {
    pub fn publish(self, view: &mut GraphView, bounds: Rect) {
        view.view = bounds;
        view.rects = self.rects;
        view.ports = self.ports;
        view.wires = self.wires;
    }
}
