use egui::{Pos2, Rect, pos2, vec2};
use materialcraft_engine::core::{PatternNode, PatternNodeKind};

use super::{PatternDrag, PatternEditor};

pub(super) const NODE_SIZE: f32 = 96.0;
pub(super) const HEADER_HEIGHT: f32 = 18.0;

pub(super) fn bounds(node: &PatternNode, editor: &PatternEditor, snap: bool, selected: bool) -> Rect {
    let mut position = materialcraft_engine::graph_grid::snap_position(node.position, false);
    if let Some(PatternDrag::Nodes { delta, .. }) = editor.drag
        && selected
    {
        position = materialcraft_engine::graph_grid::translate(position, delta.into(), snap);
    }
    let position = pos2(position[0], position[1]);
    if node.kind == PatternNodeKind::Dot {
        Rect::from_center_size(position, vec2(20.0, 20.0))
    } else {
        Rect::from_min_size(position, vec2(NODE_SIZE, NODE_SIZE))
    }
}

pub(super) fn port(bounds: Rect, slot: Option<usize>, count: usize) -> Pos2 {
    match slot {
        Some(slot) => pos2(bounds.left() + bounds.width() * (slot as f32 + 0.5) / count.max(1) as f32, bounds.bottom()),
        None => bounds.center_top(),
    }
}

pub(super) fn port_label(kind: PatternNodeKind, slot: usize) -> &'static str {
    let labels: &[&str] = match kind {
        PatternNodeKind::Quadrant => &["Top left", "Top right", "Bottom left", "Bottom right"],
        PatternNodeKind::Switch => &["False", "True"],
        PatternNodeKind::Iterate => &["Once", "Repeat"],
        PatternNodeKind::Dot => &["Output"],
    };
    labels.get(slot).copied().unwrap_or("Child")
}

pub(super) fn wire(source: Pos2, destination: Pos2) -> [Pos2; 4] {
    crate::graph::wire_geometry::curve_points(source, destination, egui::Vec2::Y)
}
