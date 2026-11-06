use crate::AnnotationSelection;
use materialcraft_core::{NodeId, Package};

#[derive(Clone)]
pub(crate) struct Snapshot {
    pub(crate) pkg: Package,
    pub(crate) graph: usize,
    pub(crate) selection: Vec<NodeId>,
    pub(crate) annotation_selection: Option<AnnotationSelection>,
    pub(crate) material_preview: crate::MaterialPreview,
    pub(crate) label: String,
    pub(crate) modified: bool,
    pub(crate) dirty: bool,
    pub(crate) cook_epoch: u64,
}
