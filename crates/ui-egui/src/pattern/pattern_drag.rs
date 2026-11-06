use egui::{Pos2, Vec2};

pub(super) enum PatternDrag {
    Nodes { start: Pos2, delta: Vec2 },
    Wire { node: u32, slot: Option<usize> },
    Box { start: Pos2 },
    Pan(egui::PointerButton),
}
