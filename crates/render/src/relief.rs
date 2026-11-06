#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relief {
    /// How far white rises above black, in scene radii; 0 = off
    pub depth: f32,
    /// The height (0..1) that stays on the original surface: 0 = everything rises out of it
    pub level: f32,
}
