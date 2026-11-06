use materialcraft_core::{ExposedParameter, Value};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    #[serde(flatten)]
    pub param: ExposedParameter,
    /// The value the maps were cooked with
    pub value: Value,
}
