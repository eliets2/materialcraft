use super::ATOMIC;
use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) fn definition() -> NodeDefinition {
    node("sharpen", "Sharpen", ATOMIC)
        .desc("Unsharp mask.")
        .atomic()
        .any_in("input", "Input")
        .float("intensity", "Intensity", 1.0, 0.0, 10.0)
        .any_out()
        .cook(sharpen)
}

fn sharpen(context: &CookContext) -> CookResult {
    let img = context.any_or(0, Kind::Gray, [0.0; 4]);
    let soft = ops::gaussian(&img, 1.0, context.tile);
    let k = context.f("intensity");
    Ok(vec![Image::generate(context.w, context.h, img.kind, |x, y| {
        let (a, b) = (img.get(x as i64, y as i64, false), soft.get(x as i64, y as i64, false));
        [a[0] + (a[0] - b[0]) * k, a[1] + (a[1] - b[1]) * k, a[2] + (a[2] - b[2]) * k, a[3]]
    })])
}
