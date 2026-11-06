use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RunParams {
    /// Command id from `command_list`, e.g. `graph.node.add`, `graph.connect`, `edit.undo`
    pub id: String,
    /// Command parameters (see the command's `params` doc). Omitted = defaults; never a dialog
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}
