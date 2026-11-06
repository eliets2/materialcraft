use serde::{Deserialize, Serialize};

use crate::Value;

/// The kind of value an exposed graph input carries
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposedParameter {
    /// Identifier, unique in the graph
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub default: Value,
    #[serde(default)]
    pub min: f32,
    #[serde(default = "one")]
    pub max: f32,
    /// Labels when the value is an enum
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

fn one() -> f32 {
    1.0
}
