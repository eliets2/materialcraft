use serde::{Deserialize, Serialize};

/// Plain text on the graph canvas
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub text: String,
    pub pos: [f32; 2],
    #[serde(default = "comment_size")]
    pub size: [f32; 2],
}

fn comment_size() -> [f32; 2] {
    [180.0, 70.0]
}
