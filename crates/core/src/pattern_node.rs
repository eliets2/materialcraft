use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{PatternNodeKind, Value};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternNode {
    pub id: u32,
    pub kind: PatternNodeKind,
    pub children: Vec<Option<u32>>,
    pub parameters: BTreeMap<String, Value>,
    pub position: [f32; 2],
}
