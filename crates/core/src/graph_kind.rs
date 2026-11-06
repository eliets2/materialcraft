use serde::{Deserialize, Serialize};

/// Whether a graph makes images or is a function
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphKind {
    #[default]
    Material,
    Function,
}
