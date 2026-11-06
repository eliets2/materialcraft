#[derive(Clone, Copy, Debug)]
pub(crate) struct CapsuleSegment {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub weight: f32,
}
