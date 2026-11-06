use serde::{Deserialize, Serialize};

/// A named bookmark on the canvas (Previous/Next pin)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    pub id: u64,
    pub name: String,
    pub pos: [f32; 2],
}
