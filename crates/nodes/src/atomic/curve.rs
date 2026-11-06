use super::ATOMIC;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::Kind;

pub(super) fn definition() -> NodeDefinition {
    node("curve", "Curve", ATOMIC).desc("Remaps values through a tone curve.").atomic().any_in("input", "Input").curve("curve", "Curve").any_out().cook(curve)
}

fn curve(context: &CookContext) -> CookResult {
    let img = context.any_or(0, Kind::Gray, [0.0; 4]);
    let c = context.curve("curve");
    // A 1024-entry table: the curve is smooth, so linear lookups are exact enough
    let lut: Vec<f32> = (0..=1024).map(|i| c.eval(i as f32 / 1024.0)).collect();
    let f = |v: f32| {
        let t = v.clamp(0.0, 1.0) * 1024.0;
        let i = t as usize;
        let a = lut.get(i).copied().unwrap_or(0.0);
        let b = lut.get(i + 1).copied().unwrap_or(a);
        a + (b - a) * (t - i as f32)
    };
    Ok(vec![img.map(img.kind, |p| [f(p[0]), f(p[1]), f(p[2]), p[3]])])
}
