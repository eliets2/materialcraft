use egui::Pos2;

pub(super) struct FunctionPort {
    pub node: u32,
    pub slot: Option<usize>,
    pub position: Pos2,
    pub radius: f32,
    pub label: &'static str,
}
