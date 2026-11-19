use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A graph given as its index or as its full SBS path
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum GraphSelector {
    Index(u32),
    Path(String),
}

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
    pub graph: Option<GraphSelector>,
    /// Square size in pixels, rounded to a power of two from 16 to 8192 (default: the graph's size)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    /// Exposed input values to cook with and embed, by input id (see `session_inspect`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Value>,
    /// Image files to read into the graph's image inputs, by input id. Paths are relative to the granted
    /// read root; an id must name an image input of the graph
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_accepts_an_index_or_an_sbs_path() {
        let by_path: MaterialExportParams = serde_json::from_value(serde_json::json!({"path": "m", "graph": "Materials/Oak"})).unwrap();
        assert!(matches!(by_path.graph, Some(GraphSelector::Path(ref p)) if p == "Materials/Oak"));
        let by_index: MaterialExportParams = serde_json::from_value(serde_json::json!({"path": "m", "graph": 2})).unwrap();
        assert!(matches!(by_index.graph, Some(GraphSelector::Index(2))));
    }
}
