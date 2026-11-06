use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct NewGraphParams {
    /// Template id: empty, pbr (six material outputs), atomics
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Graph size in pixels, a power of two from 16 to 8192 (default 1024)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
}
