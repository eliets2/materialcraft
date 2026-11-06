use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ConnectParams {
    /// Source node id
    pub from: u64,
    /// Source output port id (default: its first output)
    #[serde(default, rename = "fromPort", skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    /// Target node id
    pub to: u64,
    /// Target input port id (default: its first input)
    #[serde(default, rename = "toPort", skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
}
