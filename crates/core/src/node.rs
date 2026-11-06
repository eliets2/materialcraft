use serde::{Deserialize, Serialize};

use crate::Value;
use crate::{BaseParameters, NodeId};
use std::collections::BTreeMap;

/// A node instance: a catalog definition (`def`) with parameter overrides
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: NodeId,
    /// Catalog id, e.g. `blend`, `pixel_processor`, `output`, `input_color`, `graph_instance`
    pub def: String,
    /// User label; empty = the definition's label
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Graph-space position of the node's top-left corner
    pub pos: [f32; 2],
    /// Values that differ from the catalog defaults
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value>,
    /// Parameters driven by a graph input (param id → exposed input id)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub functions: BTreeMap<String, crate::FunctionGraph>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub input_values: BTreeMap<String, crate::ValueType>,
    #[serde(default)]
    pub base: BaseParameters,
    /// A disabled node passes its first input through
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
}

impl Node {
    pub fn new(id: NodeId, def: &str, pos: [f32; 2]) -> Node {
        Node {
            id,
            def: def.into(),
            label: String::new(),
            pos,
            params: BTreeMap::new(),
            bindings: BTreeMap::new(),
            functions: BTreeMap::new(),
            input_values: BTreeMap::new(),
            base: BaseParameters::default(),
            disabled: false,
        }
    }
}
