use serde::{Deserialize, Serialize};

/// One stop of a [`Gradient`]: position 0..1 and linear RGBA
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientKey {
    pub pos: f32,
    pub color: [f32; 4],
    #[serde(default = "midpoint")]
    pub midpoint: f32,
}

fn midpoint() -> f32 {
    0.5
}

impl GradientKey {
    pub fn new(pos: f32, color: [f32; 4]) -> Self {
        Self { pos, color, midpoint: 0.5 }
    }
}
