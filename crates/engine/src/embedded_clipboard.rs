use materialcraft_core::{FunctionNode, PatternNode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EmbeddedClipboard {
    Function { source: String, nodes: Vec<FunctionNode> },
    Pattern { source: String, nodes: Vec<PatternNode> },
}
