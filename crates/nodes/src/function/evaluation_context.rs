use materialcraft_core::Value;
use materialcraft_image::Image;
use std::collections::BTreeMap;

/// Inputs shared by one function evaluation
pub struct EvaluationContext<'a> {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub images: &'a [Option<Image>],
    pub tile: bool,
    pub variables: &'a BTreeMap<String, Value>,
}
