use serde::{Deserialize, Serialize};

use crate::{OutputFormat, SizeMode};

/// Parameters every node has ("Base parameters")
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BaseParameters {
    pub size_mode: SizeMode,
    /// log2 offset per axis (relative modes) or log2 size (absolute)
    pub size: [i32; 2],
    pub format: OutputFormat,
    /// Added to the graph's random seed for this node
    pub seed: i32,
    /// Whether sampling wraps at the edges (horizontal, vertical)
    pub tiling: [bool; 2],
}

impl Default for BaseParameters {
    fn default() -> Self {
        BaseParameters { size_mode: SizeMode::RelativeToInput, size: [0, 0], format: OutputFormat::Inherit, seed: 0, tiling: [true, true] }
    }
}
