use serde::{Deserialize, Serialize};

use crate::Value;
use std::collections::BTreeMap;

/// A named set of input values (graph presets)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub values: BTreeMap<String, Value>,
}
