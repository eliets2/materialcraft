use serde::{Deserialize, Serialize};

use crate::CurvePoint;

/// A monotone tone curve through sorted control points (linear when it has two points)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub points: Vec<CurvePoint>,
}

impl Default for Curve {
    fn default() -> Self {
        Curve { points: vec![CurvePoint::new(0.0, 0.0), CurvePoint::new(1.0, 1.0)] }
    }
}

impl Curve {
    pub fn eval(&self, x: f32) -> f32 {
        let Some(first) = self.points.first() else { return x };
        if x <= first.x {
            return first.y;
        }
        let Some(index) = self.points.windows(2).position(|points| x <= points[1].x) else {
            return self.points.last().map_or(x, |point| point.y);
        };

        let Some([start, control_left, control_right, end]) = self.segment(index) else { return x };
        let bezier = |axis: usize, t: f32| {
            let inverse = 1.0 - t;
            inverse.powi(3) * start[axis] + 3.0 * inverse.powi(2) * t * control_left[axis] + 3.0 * inverse * t * t * control_right[axis] + t.powi(3) * end[axis]
        };

        let (mut low, mut high) = (0.0, 1.0);
        for _ in 0..24 {
            let middle = (low + high) * 0.5;
            if bezier(0, middle) < x {
                low = middle;
            } else {
                high = middle;
            }
        }
        bezier(1, (low + high) * 0.5)
    }

    pub fn segment(&self, index: usize) -> Option<[[f32; 2]; 4]> {
        let left = self.points.get(index)?;
        let right = self.points.get(index.checked_add(1)?)?;
        let span = (right.x - left.x).max(1e-6);
        let slope = |index: usize| {
            let left = self.points.get(index.saturating_sub(1));
            let right = self.points.get((index + 1).min(self.points.len().saturating_sub(1)));
            match (left, right) {
                (Some(left), Some(right)) if (right.x - left.x).abs() > 1e-6 => (right.y - left.y) / (right.x - left.x),
                _ => 0.0,
            }
        };

        let secant = (right.y - left.y) / span;
        let mut slopes = [slope(index), slope(index + 1)];
        if secant == 0.0 {
            slopes = [0.0; 2];
        } else {
            for slope in &mut slopes {
                if *slope * secant < 0.0 {
                    *slope = 0.0;
                }
            }
            let length_squared = (slopes[0] / secant).powi(2) + (slopes[1] / secant).powi(2);
            if length_squared > 9.0 {
                slopes = slopes.map(|slope| slope * 3.0 / length_squared.sqrt());
            }
        }

        let left_handle = left.right.unwrap_or([left.x + span / 3.0, left.y + slopes[0] * span / 3.0]);
        let right_handle = right.left.unwrap_or([right.x - span / 3.0, right.y - slopes[1] * span / 3.0]);
        Some([[left.x, left.y], left_handle, right_handle, [right.x, right.y]])
    }
}
