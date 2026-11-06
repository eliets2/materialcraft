use materialcraft_core::Value;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct PatternVisit {
    pub node: u32,
    pub center: [f32; 2],
    pub extent: f32,
    pub depth: u32,
    pub number: u32,
    pub seed: u32,
    pub variables: BTreeMap<String, Value>,
}
