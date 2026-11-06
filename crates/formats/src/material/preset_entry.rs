use materialcraft_core::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresetEntry {
    pub name: String,
    pub values: BTreeMap<String, Value>,
}
