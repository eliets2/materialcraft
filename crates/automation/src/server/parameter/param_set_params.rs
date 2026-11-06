use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ParamSetParams {
    /// Node id
    pub id: u64,
    /// Parameter id (see `library_list`)
    pub param: String,
    /// The value: a number, `[x, y]`, `"#rrggbb"`, an enum index, a curve `[[x, y], …]` or a gradient `[[pos, color], …]`
    pub value: Value,
}
