pub const GRID_SPACING: f32 = 32.0;

pub fn snap_position(position: [f32; 2], enabled: bool) -> [f32; 2] {
    position.map(|coordinate| {
        let coordinate = if coordinate.is_finite() { coordinate.clamp(-1e7, 1e7) } else { 0.0 };
        if enabled { (coordinate / GRID_SPACING).round() * GRID_SPACING } else { coordinate }
    })
}

/// Apply the same movement and snapping rule during a drag preview and its commit
pub fn translate(position: [f32; 2], offset: [f32; 2], snap: bool) -> [f32; 2] {
    snap_position([position[0] + offset[0], position[1] + offset[1]], snap)
}
