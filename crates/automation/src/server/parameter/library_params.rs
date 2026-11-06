use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct LibraryParams {
    /// Only nodes whose id or label contains this text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Library category, such as "Atomic nodes" or "Graph items"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}
