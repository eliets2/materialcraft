use serde::{Deserialize, Serialize};

/// How a node's output size follows its surroundings ("Output size" base parameter)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SizeMode {
    /// The size of the main (first connected) input; the graph size when none is connected
    #[default]
    RelativeToInput,
    /// The graph ("parent") size
    RelativeToParent,
    /// `offset` is the absolute log2 size (e.g. 10 = 1024 px)
    Absolute,
}
