use serde::{Deserialize, Serialize};

use crate::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionNode {
    pub id: u32,
    // Zero requests type inference
    #[serde(default)]
    pub type_code: u32,
    pub op: String,
    // Slots follow the native operation's connector order
    #[serde(default)]
    pub inputs: Vec<Option<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default)]
    pub pos: [f32; 2],
}
