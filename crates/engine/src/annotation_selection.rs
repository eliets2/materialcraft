use materialcraft_core::Graph;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum AnnotationSelection {
    Frame(u64),
    Comment(u64),
}

impl AnnotationSelection {
    pub fn exists(self, graph: &Graph) -> bool {
        match self {
            Self::Frame(id) => graph.frames.iter().any(|frame| frame.id == id),
            Self::Comment(id) => graph.comments.iter().any(|comment| comment.id == id),
        }
    }
}
