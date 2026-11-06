use materialcraft_core::Value;

pub(super) struct InputBridge {
    pub identifier: String,
    pub type_code: u32,
    pub usage: u32,
    pub default: Value,
}
