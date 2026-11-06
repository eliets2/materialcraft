use serde::{Deserialize, Serialize};

/// The graph view's camera: the graph point at the view's top-left corner, and the zoom
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphCamera {
    pub pan: [f32; 2],
    pub zoom: f32,
}

impl GraphCamera {
    pub const ZOOM: (f32, f32) = (0.08, 4.0);

    /// The camera with a finite pan and a zoom inside [`Self::ZOOM`] (state set from outside can
    /// hold a zoom of 0 or NaN, which the view divides by).
    pub fn sanitized(self) -> GraphCamera {
        let d = GraphCamera::default();
        let pan = |v: f32, d: f32| if v.is_finite() { v.clamp(-1.0e9, 1.0e9) } else { d };
        let zoom = if self.zoom.is_finite() { self.zoom.clamp(Self::ZOOM.0, Self::ZOOM.1) } else { d.zoom };
        GraphCamera { pan: [pan(self.pan[0], d.pan[0]), pan(self.pan[1], d.pan[1])], zoom }
    }
}

impl Default for GraphCamera {
    fn default() -> Self {
        GraphCamera { pan: [-60.0, -60.0], zoom: 0.8 }
    }
}
