use materialcraft_core::Value;
use materialcraft_image::Image;
use std::collections::BTreeMap;

/// What to cook a portable material with, over the graph's own values
#[derive(Clone, Debug, Default)]
pub struct MaterialOptions {
    /// Square size in pixels (rounded to a power of two, 16..8192); `None` = the graph's size
    pub size: Option<u32>,
    /// Exposed input values (they also become the embedded package's values)
    pub values: BTreeMap<String, Value>,
    /// Images for the graph's input nodes, by identifier (mesh maps)
    pub inputs: BTreeMap<String, Image>,
}
