use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MaterialExportParams {
    /// Forward-slash path relative to the write root (`.mcmat` is appended if missing). The
    /// parent folder must exist
    pub path: String,
    /// Graph index (default: the open graph)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<u32>,
    /// Square size in pixels, rounded to a power of two from 16 to 8192 (default: the graph's size)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    /// Exposed input values to cook with and embed, by input id (see `session_inspect`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Value>,
}
