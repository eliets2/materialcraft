use std::sync::Weak;

pub(crate) struct InputFingerprint {
    pub buffer: Weak<Vec<f32>>,
    pub shape: (u32, u32, usize),
    pub key: u64,
}
