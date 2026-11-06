#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Displacement {
    /// How far white rises above black, in world units
    pub depth: f32,
    /// The height (0..1) that stays on the original surface
    pub level: f32,
    /// UV repeats of the height map
    pub tiling: f32,
    /// Subdivide while one more step stays within this many triangles
    pub max_triangles: usize,
    /// Shade with the displaced surface's normals instead of the original ones
    pub shade_relief: bool,
}
