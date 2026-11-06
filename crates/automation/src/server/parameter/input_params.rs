use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct InputParams {
    /// Input events in window points, applied in order on the next frames
    pub events: Vec<Value>,
}
