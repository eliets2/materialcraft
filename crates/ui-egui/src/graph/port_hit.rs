use egui::Pos2;
use materialcraft_engine::core::NodeId;
use materialcraft_engine::nodes::PortKind;

/// One port as drawn this frame
#[derive(Clone, Debug)]
pub struct PortHit {
    pub node: NodeId,
    pub output: bool,
    pub port: String,
    pub label: String,
    pub pos: Pos2,
    pub kind: PortKind,
}

pub(super) fn nearest(ports: &[PortHit], pointer: Pos2, radius: f32) -> Option<&PortHit> {
    nearest_by(ports, pointer, |port| (port.pos, radius))
}

pub(crate) fn nearest_by<T>(items: &[T], pointer: Pos2, hit_area: impl Fn(&T) -> (Pos2, f32)) -> Option<&T> {
    items
        .iter()
        .filter_map(|item| {
            let (position, radius) = hit_area(item);
            let distance = position.distance_sq(pointer);
            (distance <= radius * radius).then_some((item, distance))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(item, _)| item)
}
