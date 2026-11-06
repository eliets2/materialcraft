use serde::{Deserialize, Serialize};

use crate::GradientKey;

/// A color ramp (gradient map)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub keys: Vec<GradientKey>,
}

impl Default for Gradient {
    fn default() -> Self {
        Gradient { keys: vec![GradientKey::new(0.0, [0.0, 0.0, 0.0, 1.0]), GradientKey::new(1.0, [1.0, 1.0, 1.0, 1.0])] }
    }
}

impl Gradient {
    /// Linear interpolation between the sorted keys
    pub fn eval(&self, t: f32) -> [f32; 4] {
        let k = &self.keys;
        let Some(first) = k.first() else { return [t, t, t, 1.0] };
        if t <= first.pos {
            return first.color;
        }
        for w in k.windows(2) {
            if t <= w[1].pos {
                let span = (w[1].pos - w[0].pos).max(1e-6);
                let midpoint = w[0].midpoint.clamp(0.001, 0.999);
                let f = ((t - w[0].pos) / span).clamp(0.0, 1.0).powf(0.5_f32.ln() / midpoint.ln());
                let (a, b) = (w[0].color, w[1].color);
                return [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f, a[3] + (b[3] - a[3]) * f];
            }
        }
        k.last().map_or([t, t, t, 1.0], |l| l.color)
    }

    /// Keys sorted by position (call after editing)
    pub fn sort(&mut self) {
        self.keys.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    }
}
