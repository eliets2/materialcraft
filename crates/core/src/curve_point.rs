use serde::{Deserialize, Serialize};

/// One control point of a [`Curve`]: position and value, both 0..1
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub x: f32,
    pub y: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<[f32; 2]>,
}

impl CurvePoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y, left: None, right: None }
    }
}
