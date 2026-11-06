use materialcraft_core::{NodeId, Value};
use materialcraft_image::Image;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

/// What to cook and how
pub struct Options<'a> {
    /// Cap on the larger side of every node's output (previews); `None` = full size
    pub max_size: Option<u32>,
    /// Cook only these nodes and what they depend on (`None` = the whole graph)
    pub only: Option<Vec<NodeId>>,
    /// Images for the graph's input nodes, by identifier (graph instances)
    pub inputs: BTreeMap<String, Image>,
    /// Values for the graph's exposed parameters (graph instances), over the graph's own
    pub overrides: BTreeMap<String, Value>,
    /// Decodes a package resource (bitmap) by name
    pub resource: &'a (dyn Fn(&str) -> Option<Image> + Sync),
    /// Set from another thread to stop between nodes
    pub cancel: Option<&'a AtomicBool>,
    pub depth: u32,
    /// Graphs being cooked around this one, outermost first (cycle detection)
    pub graph_stack: Vec<usize>,
}

impl<'a> Options<'a> {
    pub fn new(resource: &'a (dyn Fn(&str) -> Option<Image> + Sync)) -> Options<'a> {
        Options { max_size: None, only: None, inputs: BTreeMap::new(), overrides: BTreeMap::new(), resource, cancel: None, depth: 0, graph_stack: Vec::new() }
    }
}
