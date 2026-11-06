use super::RunParams;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct BatchParams {
    /// Commands run in order; the batch stops at the first error and reports its index
    pub steps: Vec<RunParams>,
}
