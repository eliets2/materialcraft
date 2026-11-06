use super::capsule_segment::CapsuleSegment;

pub(crate) struct CapsuleRaster {
    segments: Vec<CapsuleSegment>,
    radius: f32,
    grid_side: usize,
    cell_size: [f32; 2],
    bins: Vec<Vec<u32>>,
    image_size: [f32; 2],
    tile: bool,
}

impl CapsuleRaster {
    pub fn new(segments: Vec<CapsuleSegment>, radius: f32, w: u32, h: u32, tile: bool) -> CapsuleRaster {
        let radius = if radius.is_finite() { radius.max(0.0) } else { 0.0 };
        let reach = radius + 1.0;
        let grid_side = ((w.max(h) as f32 / (reach * 4.0).max(8.0)) as usize).clamp(1, 128);
        let cell_size = [w as f32 / grid_side as f32, h as f32 / grid_side as f32];
        let mut bins: Vec<Vec<u32>> = vec![Vec::new(); grid_side * grid_side];
        for (index, segment) in segments.iter().enumerate() {
            if !segment.start.iter().chain(&segment.end).all(|v| v.is_finite()) {
                continue;
            }
            let span = |start: f32, end: f32, cell: f32| {
                let first = ((start.min(end) - reach) / cell).floor() as i64;
                (first, (((start.max(end) + reach) / cell).floor() as i64).min(first + grid_side as i64 - 1))
            };

            let ((x0, x1), (y0, y1)) = (span(segment.start[0], segment.end[0], cell_size[0]), span(segment.start[1], segment.end[1], cell_size[1]));
            for gy in y0..=y1 {
                for gx in x0..=x1 {
                    if !tile && (gx < 0 || gy < 0 || gx >= grid_side as i64 || gy >= grid_side as i64) {
                        continue;
                    }
                    if let Some(b) = bins.get_mut(gy.rem_euclid(grid_side as i64) as usize * grid_side + gx.rem_euclid(grid_side as i64) as usize) {
                        b.push(index as u32);
                    }
                }
            }
        }
        CapsuleRaster { segments, radius, grid_side, cell_size, bins, image_size: [w as f32, h as f32], tile }
    }

    /// Coverage (0..1, the strongest segment's weight) at pixel coordinates `(x, y)`
    pub fn coverage(&self, x: f32, y: f32) -> f32 {
        let (gx, gy) = (((x / self.cell_size[0]) as usize).min(self.grid_side - 1), ((y / self.cell_size[1]) as usize).min(self.grid_side - 1));
        let mut best = 0.0f32;
        for &index in self.bins.get(gy * self.grid_side + gx).into_iter().flatten() {
            let Some(segment) = self.segments.get(index as usize) else { continue };
            let (mut px, mut py) = (x - segment.start[0], y - segment.start[1]);
            if self.tile {
                px -= (px / self.image_size[0]).round() * self.image_size[0];
                py -= (py / self.image_size[1]).round() * self.image_size[1];
            }
            let (dx, dy) = (segment.end[0] - segment.start[0], segment.end[1] - segment.start[1]);
            let t = ((px * dx + py * dy) / (dx * dx + dy * dy).max(1e-12)).clamp(0.0, 1.0);
            let d = ((px - dx * t).powi(2) + (py - dy * t).powi(2)).sqrt();
            best = best.max((self.radius - d + 0.5).clamp(0.0, 1.0) * segment.weight);
        }
        best
    }
}
