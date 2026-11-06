use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct FilterParams {
    /// Only entries whose id or label contains this text (case-insensitive)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}
