use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PathParams {
    /// Forward-slash path relative to the workspace root
    pub path: String,
}
