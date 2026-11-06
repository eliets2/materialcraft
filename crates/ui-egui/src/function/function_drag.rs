use egui::Vec2;

pub(super) enum FunctionDrag {
    Node(u32, Vec2),
    Wire(u32),
    Box { start: egui::Pos2 },
    Pan(egui::PointerButton),
}
