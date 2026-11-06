use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeAddParams {
    /// Node definition id from `library_list`, e.g. `uniform`, `blend`, `levels`, `normal`, `output`
    pub def: String,
    /// Graph position `[x, y]` (default: right of `connectFrom`, else the origin)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[f32; 2]>,
    /// Parameter values by id (see `library_list`): numbers, `[x, y]`, `"#rrggbb"`, enum indices
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// Wire `{node, port?}`'s output into the new node's first input
    #[serde(default, rename = "connectFrom", skip_serializing_if = "Option::is_none")]
    pub connect_from: Option<Value>,
}
