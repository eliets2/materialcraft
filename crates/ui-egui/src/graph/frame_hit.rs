use egui::{Pos2, Rect};

pub(crate) struct FrameHit {
    pub id: u64,
    pub rect: Rect,
    pub title: Rect,
    pub grip: Rect,
}

impl FrameHit {
    pub fn contains(&self, position: Pos2) -> bool {
        self.rect.contains(position) || self.title.contains(position)
    }
}
