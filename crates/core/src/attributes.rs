use serde::{Deserialize, Serialize};

/// Descriptive metadata ("Attributes")
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Attributes {
    pub category: String,
    pub description: String,
    pub author: String,
    pub tags: String,
}
