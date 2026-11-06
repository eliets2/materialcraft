use crate::NodeOutput;
use std::sync::Arc;

/// One node's cooked result
#[derive(Clone, Debug)]
pub struct NodeResult {
    /// One image per output port (empty when the cook failed)
    pub outputs: Arc<Vec<NodeOutput>>,
    pub error: Option<String>,
    /// Cook time in milliseconds (0 when served from the cache)
    pub ms: f32,
    pub key: u64,
    pub cached: bool,
}
