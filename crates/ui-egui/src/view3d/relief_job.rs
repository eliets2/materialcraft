use materialcraft_render::Scene;
use std::sync::{Arc, mpsc::Receiver};

/// A displaced scene building on a worker thread (`None` arrives when nothing moves)
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) struct ReliefJob {
    pub(super) key: u64,
    pub(super) rx: Receiver<Option<Arc<Scene>>>,
}
