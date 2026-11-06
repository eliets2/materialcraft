use crate::{BridgeClient, Headless};
use std::sync::{Arc, Mutex};

/// Where tools run
pub enum Backend {
    /// An in-process session
    Headless(Arc<Mutex<Headless>>),
    /// The running desktop app, over its control channel
    Bridge(Arc<BridgeClient>),
}
