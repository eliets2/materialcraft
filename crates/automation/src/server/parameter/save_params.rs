use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct SaveParams {
    /// Optional path relative to the write root, with an .sbs extension
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}
