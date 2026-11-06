pub(super) struct PatternStamp {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
    pub pattern: i32,
    pub variation: f32,
    pub blend: i32,
    pub filtering: i32,
    pub premultiplied: bool,
}
