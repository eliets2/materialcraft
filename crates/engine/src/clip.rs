use materialcraft_core::{Connection, Node};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub inputs: Vec<Connection>,
    #[serde(skip)]
    pub source_graph: Option<String>,
}
