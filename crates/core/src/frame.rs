use serde::{Deserialize, Serialize};

/// A titled, colored rectangle behind a group of nodes
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub id: u64,
    pub title: String,
    #[serde(default = "default_title_visible")]
    pub title_visible: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// `[x, y, width, height]` in graph space
    pub rect: [f32; 4],
    /// Linear RGBA (alpha is the fill opacity)
    pub color: [f32; 4],
}

fn default_title_visible() -> bool {
    true
}

impl Frame {
    pub fn contains_position(&self, position: [f32; 2]) -> bool {
        let [x, y, width, height] = self.rect;
        position[0] >= x && position[1] >= y && position[0] <= x + width && position[1] <= y + height
    }

    pub fn contains_rect(&self, rect: [f32; 4]) -> bool {
        self.contains_position([rect[0], rect[1]]) && self.contains_position([rect[0] + rect[2], rect[1] + rect[3]])
    }
}
