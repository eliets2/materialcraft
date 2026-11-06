use serde::{Deserialize, Serialize};

/// The 2D view's camera: image zoom (1 = one image pixel per point) and center offset
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageCamera {
    pub zoom: f32,
    pub offset: [f32; 2],
    /// Fit the image to the view (until the user zooms)
    pub fit: bool,
}

impl ImageCamera {
    pub const ZOOM: (f32, f32) = (0.02, 64.0);

    /// The camera with a finite offset and a zoom inside [`Self::ZOOM`].
    pub fn sanitized(self) -> ImageCamera {
        let off = |v: f32| if v.is_finite() { v.clamp(-1.0e9, 1.0e9) } else { 0.0 };
        let zoom = if self.zoom.is_finite() { self.zoom.clamp(Self::ZOOM.0, Self::ZOOM.1) } else { 1.0 };
        ImageCamera { zoom, offset: [off(self.offset[0]), off(self.offset[1])], fit: self.fit }
    }
}

impl Default for ImageCamera {
    fn default() -> Self {
        ImageCamera { zoom: 1.0, offset: [0.0, 0.0], fit: true }
    }
}
