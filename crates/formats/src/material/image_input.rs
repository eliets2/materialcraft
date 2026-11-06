use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageInput {
    pub identifier: String,
    /// `gray` or `color`
    pub kind: String,
}
