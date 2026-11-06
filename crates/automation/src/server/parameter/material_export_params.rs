use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// Some MCP clients reject boolean sub-schemas
#[schemars(extend("additionalProperties" = {"not": {}}))]
pub struct MaterialExportParams {
    /// Forward-slash path relative to the write root (`.mcmat` is appended if missing). The
    /// parent folder must exist
    pub path: String,
    /// Graph index or full SBS path; defaults to the open graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<u32>,
    /// Square size in pixels, rounded to a power of two from 16 to 8192 (default: the graph's size)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    /// Exposed input values to cook with and embed, by input id (see `session_inspect`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Value>,
    /// Image files to read into the graph's image inputs, by input id. Paths are relative to the write
    /// root's read rules; an id must name an image input of the graph
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Value>,
}
}
